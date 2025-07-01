//! This module deals with processing and visualizing scanned image data.
//! It includes methods for filtering, updating intensity images, saving images, and managing
//! FFT operations for signal processing.

use crate::config::{ConfigCommand, ConfigContainer, ThreadCommunication};
use crate::data_container::ScannedImageFilterData;
use crate::filters::filter::{Filter, FILTER_REGISTRY};
use crate::gui::matrix_plot::{SelectedPixel, ROI};
use crate::io::{
    load_meta_data_of_thz_file, open_pulse_from_thz, open_scan_from_thz, save_to_thz,
    update_meta_data_of_thz_file,
};
use crate::math_tools::{
    apply_adapted_blackman_window, apply_blackman, apply_flat_top, apply_hamming, apply_hanning,
    average_polygon_roi, calculate_optical_properties, fft, ifft, numpy_unwrap, FftWindowType,
};
use dotthz::DotthzMetaData;
use ndarray::parallel::prelude::*;
use ndarray::{Array1, Array2, Array3, Axis};
use realfft::RealFftPlanner;
use std::f32::consts::PI;
use std::sync::atomic::Ordering;
use std::time::Instant;

pub enum UpdateType {
    None,
    Filter(usize),
    #[allow(dead_code)]
    Image,
    Plot,
}

/// Updates the intensity image lock with the filtered image from the scan.
///
/// This function writes the filtered image into the shared data structure to reflect the updated intensity.
///
/// # Arguments
/// * `scan` - A mutable reference to the `ScannedImage`.
/// * `img_lock` - A thread-safe `RwLock` containing the 2D array for the intensity image.
fn update_intensity_image(
    scan: &ScannedImageFilterData,
    thread_communication: &ThreadCommunication,
) {
    if scan.img.shape()[0] == 0 || scan.img.shape()[1] == 0 {
        if let Ok(mut write_guard) = thread_communication.img_lock.write() {
            *write_guard = Array2::zeros((1, 1));
        }
    } else {
        if let Ok(mut write_guard) = thread_communication.img_lock.write() {
            *write_guard = scan.img.clone();
        }
    }
    // is this really required?
    if let Ok(mut write_guard) = thread_communication.filtered_data_lock.write() {
        *write_guard = scan.data.clone();
    }
    if let Ok(mut write_guard) = thread_communication.filtered_time_lock.write() {
        *write_guard = scan.time.clone();
    }
    if let Ok(mut write_guard) = thread_communication.voxel_plot_instances_lock.write() {
        // let lower = scan
        //     .filtered_time
        //     .iter()
        //     .position(|t| *t == config.time_window[0].round())
        //     .unwrap_or(0);
        // let upper = scan
        //     .filtered_time
        //     .iter()
        //     .position(|t| *t == config.time_window[1].round())
        //     .unwrap_or(scan.filtered_time.len()); // safer fallback

        if scan.time.is_empty() {
            log::warn!("scan time is empty, cannot update voxel plot instances");
            return;
        }

        // save because we checked above
        let time_span = scan.time.last().unwrap() - scan.time.first().unwrap();
        let (instances, cube_width, cube_height, cube_depth) =
            crate::gui::threed_plot::instance_from_data(
                time_span,
                scan.data.clone(),
                thread_communication.gui_settings.opacity_threshold,
            );
        write_guard.0 = instances;
        write_guard.1 = cube_width;
        write_guard.2 = cube_height;
        write_guard.3 = cube_depth;
    }
}

fn update_metadata_rois(md: &mut DotthzMetaData, input: &ScannedImageFilterData) {
    // Remove all old ROI {number} entries
    let mut keys_to_remove = vec![];
    for key in md.md.keys() {
        if key.starts_with("ROI ") && key[4..].chars().all(|c| c.is_digit(10)) {
            keys_to_remove.push(key.clone());
        }
    }
    for key in keys_to_remove {
        md.md.swap_remove(&key);
    }

    // Insert new ROI Labels and ROI {number} entries
    if !input.rois.is_empty() {
        let mut roi_labels = String::new();
        for (i, (label, coords)) in input.rois.iter().enumerate() {
            if i > 0 {
                roi_labels.push(',');
            }
            roi_labels.push_str(label);
            md.md.insert(
                format!("ROI {}", i),
                coords
                    .iter()
                    .map(|(x, y)| format!("[{},{}]", x, y))
                    .collect::<Vec<String>>()
                    .join(","),
            );
        }
        md.md.insert("ROI Labels".to_string(), roi_labels);
    } else {
        md.md.swap_remove("ROI Labels");
    }
}

/// Handles communication on the main thread.
///
/// This function processes incoming `ConfigCommand` instances and executes actions
/// like opening files, applying filters, and updating pixel selections.
///
/// # Arguments
/// * `thread_communication` - A channel-based communication structure between threads.
pub fn main_thread(mut thread_communication: ThreadCommunication) {
    // reads data from mutex, samples and saves if needed
    let mut config = ConfigContainer::default();
    let mut selected_pixel = SelectedPixel::default();
    let mut meta_data = DotthzMetaData::default();

    let mut reset_filters = false;

    let mut sample_roi = "".to_string();
    let mut reference_roi = "".to_string();

    let mut update = UpdateType::None;
    loop {
        if thread_communication.abort_flag.load(Ordering::Relaxed) {
            log::info!("Aborting main thread loop, clearing all config commands from the queue");
            while !thread_communication.config_rx.is_empty() {
                let r = thread_communication.config_rx.recv();
                dbg!(&r);
            }
            thread_communication
                .abort_flag
                .store(false, Ordering::Relaxed);
        }

        if let Ok(config_command) = thread_communication.config_rx.recv() {
            match config_command {
                ConfigCommand::LoadMetaData(path) => {
                    if let Some(os_path) = path.extension() {
                        if os_path != "thz" || os_path != "thzimg" || os_path != "thzswp" {
                            match load_meta_data_of_thz_file(&path, &mut meta_data) {
                                Ok(_) => {
                                    if let Ok(mut filter_data) =
                                        thread_communication.filter_data_lock.write()
                                    {
                                        if let Some(input) = filter_data.first_mut() {
                                            if let Ok(mut data) =
                                                thread_communication.data_lock.write()
                                            {
                                                data.rois.clear();
                                            }

                                            if let Some(labels) = meta_data.md.get("ROI Labels") {
                                                let roi_labels: Vec<&str> =
                                                    labels.split(',').collect();
                                                for (i, label) in roi_labels.iter().enumerate() {
                                                    if let Some(roi_data) =
                                                        meta_data.md.get(&format!("ROI {}", i))
                                                    {
                                                        // Ensure we are correctly extracting coordinates
                                                        let polygon = roi_data
                                                            .split("],") // Split by "]," to separate coordinate pairs
                                                            .filter_map(|point| {
                                                                let cleaned =
                                                                    point.trim_matches(|c| {
                                                                        c == '[' || c == ']'
                                                                    });
                                                                let values: Vec<f64> = cleaned
                                                                    .split(',')
                                                                    .filter_map(|v| {
                                                                        v.trim().parse::<f64>().ok()
                                                                    })
                                                                    .collect();

                                                                if values.len() == 2 {
                                                                    Some([values[0], values[1]])
                                                                } else {
                                                                    None
                                                                }
                                                            })
                                                            .collect::<Vec<[f64; 2]>>();

                                                        if !polygon.is_empty() {
                                                            if let Ok(mut data) =
                                                                thread_communication
                                                                    .data_lock
                                                                    .write()
                                                            {
                                                                data.rois.push(ROI {
                                                                    polygon: polygon.clone(),
                                                                    closed: true,
                                                                    name: label.to_string(),
                                                                });
                                                                input.rois.insert(
                                                                    label.to_string(),
                                                                    polygon
                                                                        .iter()
                                                                        .map(|v| {
                                                                            (
                                                                                v[0] as usize,
                                                                                v[1] as usize,
                                                                            )
                                                                        })
                                                                        .collect(),
                                                                );
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }

                                    if let Ok(mut md) = thread_communication.md_lock.write() {
                                        *md = meta_data.clone();
                                    }

                                    log::info!("loaded meta-data from {:?}", path);
                                }
                                Err(err) => {
                                    log::error!("failed loading meta-data {:?}: {:?}", path, err)
                                }
                            }
                        } else {
                            log::error!("failed loading meta-data {:?}: not a dotTHz file", path)
                        }
                    } else {
                        log::error!("failed to get extension for {:?}", path)
                    }
                    continue;
                }
                ConfigCommand::SaveROIs(mut path) => {
                    // THz Image Explorer always saves thz files
                    if let Some(os_path) = path.extension() {
                        if os_path != "thz" || os_path != "thzimg" || os_path != "thzswp" {
                            path.set_extension("thz");
                            // save full file, not just metadata, since the dotTHz file does not exist yet.
                            if let Ok(filter_data) = thread_communication.filter_data_lock.read() {
                                if let Some(input) = filter_data.first() {
                                    if let Ok(mut md) = thread_communication.md_lock.write() {
                                        // add ROIs to metadata

                                        if !input.rois.is_empty() {
                                            update_metadata_rois(&mut md, input);
                                        }

                                        match save_to_thz(&path, &input, &md) {
                                            Ok(_) => {
                                                log::info!("saved {:?}", path);
                                            }
                                            Err(err) => {
                                                log::error!("failed saving {:?}: {:?}", path, err)
                                            }
                                        }
                                    }
                                }
                            }
                        } else {
                            if let Ok(filter_data) = thread_communication.filter_data_lock.read() {
                                if let Ok(mut md) = thread_communication.md_lock.write() {
                                    if let Some(input) = filter_data.first() {
                                        // add ROIs to metadata
                                        if !input.rois.is_empty() {
                                            update_metadata_rois(&mut md, input);
                                        }
                                        match update_meta_data_of_thz_file(&path, &md) {
                                            Ok(_) => {
                                                log::info!("updated meta-data in {:?}", path);
                                            }
                                            Err(err) => {
                                                log::error!(
                                                    "failed updating meta-data {:?}: {:?}",
                                                    path,
                                                    err
                                                )
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    continue;
                }
                ConfigCommand::UpdateMetaData(mut path) => {
                    // THz Image Explorer always saves thz files
                    if let Some(os_path) = path.extension() {
                        if os_path != "thz" || os_path != "thzimg" || os_path != "thzswp" {
                            path.set_extension("thz");
                            // save full file, not just metadata, since the dotTHz file does not exist yet.
                            if let Ok(filter_data) = thread_communication.filter_data_lock.read() {
                                if let Some(input) = filter_data.first() {
                                    if let Ok(md) = thread_communication.md_lock.read() {
                                        match save_to_thz(&path, &input, &md) {
                                            Ok(_) => {
                                                log::info!("saved {:?}", path);
                                            }
                                            Err(err) => {
                                                log::error!("failed saving {:?}: {:?}", path, err)
                                            }
                                        }
                                    }
                                }
                            }
                        } else {
                            if let Ok(md) = thread_communication.md_lock.read() {
                                match update_meta_data_of_thz_file(&path, &md) {
                                    Ok(_) => {
                                        log::info!("updated meta-data in {:?}", path);
                                    }
                                    Err(err) => {
                                        log::error!(
                                            "failed updating meta-data {:?}: {:?}",
                                            path,
                                            err
                                        )
                                    }
                                }
                            }
                        }
                    }
                    continue;
                }
                ConfigCommand::OpenRef(selected_file_path) => {
                    let mut meta_data = DotthzMetaData::default();
                    if let Ok((time, mut reference)) =
                        open_pulse_from_thz(&selected_file_path, &mut meta_data)
                    {
                        update = UpdateType::Filter(1);
                        if let Ok(mut filter_data) = thread_communication.filter_data_lock.write() {
                            if let Some(input) = filter_data.first_mut() {
                                if input.time.is_empty() {
                                    // no data loaded yet, populate with 1 x 1 scan and 0s

                                    input.time = time.clone();
                                    input.data = Array3::zeros((1, 1, time.len()));

                                    let n = input.time.len();
                                    let rng = input.time[n - 1] - input.time[0];
                                    let mut real_planner = RealFftPlanner::<f32>::new();
                                    let r2c = real_planner.plan_fft_forward(n);
                                    let c2r = real_planner.plan_fft_inverse(n);
                                    let spectrum = r2c.make_output_vec();
                                    let freq =
                                        (0..spectrum.len()).map(|i| i as f32 / rng).collect();
                                    input.frequency = freq;

                                    input.r2c = Some(r2c);
                                    input.c2r = Some(c2r);

                                    input.phases = Array3::zeros((1, 1, input.frequency.len()));
                                    input.amplitudes = Array3::zeros((1, 1, input.frequency.len()));
                                    input.fft = Array3::zeros((1, 1, input.frequency.len()));
                                }
                                if input.time.len() != reference.len()
                                    || !time.is_empty() && (input.time[0] - time[0]).abs() > 1e-9
                                {
                                    log::warn!(
                                            "Time data from reference file does not match scan time data. \
                                                Resizing and aligning reference signal. Phase data might not match correctly."
                                        );

                                    if !input.time.is_empty()
                                        && !time.is_empty()
                                        && input.time.len() > 1
                                        && time.len() > 1
                                    {
                                        let target_len = input.time.len();
                                        let mut new_reference = Array1::zeros(target_len);

                                        let input_dt = input.time[1] - input.time[0];
                                        let ref_dt = time[1] - time[0];

                                        if (input_dt - ref_dt).abs() > 1e-9 {
                                            log::warn!("Time steps of scan and reference do not match. Alignment may be inaccurate.");
                                        }

                                        // Calculate the time offset and corresponding index shift
                                        let time_offset = input.time[0] - time[0];
                                        let index_offset = (time_offset / ref_dt).round() as isize;

                                        // Determine the source slice from the original reference
                                        let src_start = if index_offset > 0 {
                                            index_offset as usize
                                        } else {
                                            0
                                        };
                                        let src_end = reference.len();

                                        // Determine the destination slice in the new reference array
                                        let dst_start = if index_offset < 0 {
                                            (-index_offset) as usize
                                        } else {
                                            0
                                        };
                                        let dst_end = target_len;

                                        // Calculate the length of the overlapping region
                                        let copy_len = (src_end.saturating_sub(src_start))
                                            .min(dst_end.saturating_sub(dst_start));

                                        if copy_len > 0 {
                                            // Slice the source and destination arrays and copy the data
                                            let src_slice = reference.slice(ndarray::s![
                                                src_start..src_start + copy_len
                                            ]);
                                            let mut dst_slice =
                                                new_reference.slice_mut(ndarray::s![
                                                    dst_start..dst_start + copy_len
                                                ]);
                                            dst_slice.assign(&src_slice);
                                        }

                                        reference = new_reference;
                                    } else {
                                        // Fallback for empty or single-point time arrays
                                        log::warn!("Cannot align signals due to empty or invalid time data. Resizing naively.");
                                        let target_len = input.time.len();
                                        if target_len > reference.len() {
                                            let mut new_reference = Array1::zeros(target_len);
                                            new_reference
                                                .slice_mut(ndarray::s![..reference.len()])
                                                .assign(&reference);
                                            reference = new_reference;
                                        } else {
                                            reference = reference
                                                .slice(ndarray::s![..target_len])
                                                .to_owned();
                                        }
                                    }
                                }
                                if let Some(r2c) = &input.r2c {
                                    let mut input_vec = vec![0.0; input.time.len()];
                                    let mut spectrum = r2c.make_output_vec();

                                    let mut amplitudes = vec![0.0; time.len()];
                                    let mut phases = vec![0.0; time.len()];
                                    let mut fft = Array1::zeros(input.frequency.len());

                                    match config.fft_window_type {
                                        FftWindowType::AdaptedBlackman => {
                                            apply_adapted_blackman_window(
                                                &mut reference.view_mut(),
                                                &time,
                                                &config.fft_window[0],
                                                &config.fft_window[1],
                                            );
                                        }
                                        FftWindowType::Blackman => {
                                            apply_blackman(&mut reference.view_mut(), &time)
                                        }
                                        FftWindowType::Hanning => {
                                            apply_hanning(&mut reference.view_mut(), &time)
                                        }
                                        FftWindowType::Hamming => {
                                            apply_hamming(&mut reference.view_mut(), &time)
                                        }
                                        FftWindowType::FlatTop => {
                                            apply_flat_top(&mut reference.view_mut(), &time)
                                        }
                                    }

                                    // Forward transform the input data
                                    input_vec.clone_from_slice(reference.as_slice().unwrap());
                                    r2c.process(&mut input_vec, &mut spectrum).unwrap();

                                    // Assign spectrum to fft
                                    fft.assign(&Array1::from_vec(spectrum.clone()));

                                    // Assign amplitudes
                                    amplitudes
                                        .iter_mut()
                                        .zip(spectrum.iter())
                                        .for_each(|(a, s)| *a = s.norm());

                                    // Assign phases (unwrap)
                                    let phase: Vec<f32> =
                                        spectrum.iter().map(|s| s.arg()).collect();
                                    let unwrapped = numpy_unwrap(&phase, Some(2.0 * PI));
                                    phases
                                        .iter_mut()
                                        .zip(unwrapped.iter())
                                        .for_each(|(p, v)| *p = *v);

                                    if let Ok(mut data) = thread_communication.data_lock.write() {
                                        data.available_references
                                            .push("Reference File".to_string());
                                        data.available_samples.push("Reference File".to_string());

                                        data.roi_signal.insert(
                                            "Reference File".to_string(),
                                            reference.to_vec(),
                                        );
                                        data.roi_signal_fft.insert(
                                            "Reference File".to_string(),
                                            amplitudes.clone(),
                                        );
                                        data.roi_phase
                                            .insert("Reference File".to_string(), phases.clone());
                                    }
                                    input
                                        .roi_data
                                        .insert("Reference File".to_string(), reference);
                                    input.roi_signal_fft.insert(
                                        "Reference File".to_string(),
                                        Array1::from_vec(amplitudes),
                                    );
                                    input.roi_phase_fft.insert(
                                        "Reference File".to_string(),
                                        Array1::from_vec(phases),
                                    );
                                }
                            }
                        }
                    }
                }
                ConfigCommand::OpenFile(selected_file_path) => {
                    update = UpdateType::Filter(1);
                    reset_filters = true;
                    if let Some(file_ending) = selected_file_path.extension() {
                        match file_ending.to_str().unwrap() {
                            "thz" => {
                                if let Ok(mut filter_data) =
                                    thread_communication.filter_data_lock.write()
                                {
                                    if let Some(input) = filter_data.first_mut() {
                                        match open_scan_from_thz(
                                            &selected_file_path,
                                            input,
                                            &mut meta_data,
                                        ) {
                                            Ok(_) => {
                                                log::info!("opened {:?}", selected_file_path);
                                                // Copy the first entry into all others
                                            }
                                            Err(err) => {
                                                log::error!(
                                                    "failed opening {:?}: {:?}",
                                                    selected_file_path,
                                                    err
                                                )
                                            }
                                        };

                                        if let Ok(mut data) = thread_communication.data_lock.write()
                                        {
                                            data.rois.clear();
                                        }

                                        if let Some(labels) = meta_data.md.get("ROI Labels") {
                                            let roi_labels: Vec<&str> = labels.split(',').collect();
                                            for (i, label) in roi_labels.iter().enumerate() {
                                                if let Some(roi_data) =
                                                    meta_data.md.get(&format!("ROI {}", i))
                                                {
                                                    // Ensure we are correctly extracting coordinates
                                                    let polygon = roi_data
                                                        .split("],") // Split by "]," to separate coordinate pairs
                                                        .filter_map(|point| {
                                                            let cleaned = point.trim_matches(|c| {
                                                                c == '[' || c == ']'
                                                            });
                                                            let values: Vec<f64> = cleaned
                                                                .split(',')
                                                                .filter_map(|v| {
                                                                    v.trim().parse::<f64>().ok()
                                                                })
                                                                .collect();

                                                            if values.len() == 2 {
                                                                Some([values[0], values[1]])
                                                            } else {
                                                                None
                                                            }
                                                        })
                                                        .collect::<Vec<[f64; 2]>>();

                                                    if !polygon.is_empty() {
                                                        if let Ok(mut data) =
                                                            thread_communication.data_lock.write()
                                                        {
                                                            data.rois.push(ROI {
                                                                polygon: polygon.clone(),
                                                                closed: true,
                                                                name: label.to_string(),
                                                            });
                                                            input.rois.insert(
                                                                label.to_string(),
                                                                polygon
                                                                    .iter()
                                                                    .map(|v| {
                                                                        (
                                                                            v[0] as usize,
                                                                            v[1] as usize,
                                                                        )
                                                                    })
                                                                    .collect(),
                                                            );
                                                        }
                                                    }
                                                }
                                            }
                                        }

                                        if let Ok(mut md) = thread_communication.md_lock.write() {
                                            *md = meta_data.clone();
                                        }

                                        let first = input.clone();
                                        for entry in filter_data.iter_mut().skip(1) {
                                            *entry = first.clone();
                                        }
                                    }
                                }
                            }
                            _ => {
                                log::warn!(
                                    "file not supported: {:?} \n Open another file.",
                                    selected_file_path
                                );
                                continue;
                            }
                        }
                    }
                }
                ConfigCommand::SaveFile(mut path) => {
                    // THz Image Explorer always saves thz files
                    if let Some(os_path) = path.extension() {
                        if os_path != "thz" || os_path != "thzimg" || os_path != "thzswp" {
                            path.set_extension("thz");
                            path.set_extension("thz");
                        }
                    }

                    if let Ok(filter_data) = thread_communication.filter_data_lock.read() {
                        // note, we save the input data, not the filtered data
                        if let Some(input) = filter_data.first() {
                            if let Ok(mut md) = thread_communication.md_lock.write() {
                                if !input.rois.is_empty() {
                                    update_metadata_rois(&mut md, input);
                                }

                                match save_to_thz(&path, &input, &md) {
                                    Ok(_) => {
                                        log::info!("saved {:?}", path);
                                    }
                                    Err(err) => {
                                        log::error!("failed saving {:?}: {:?}", path, err)
                                    }
                                }
                            }
                        }
                    }
                }
                ConfigCommand::SetFFTWindowLow(low) => {
                    config.fft_window[0] = low;
                    update = UpdateType::Filter(thread_communication.fft_index);
                }
                ConfigCommand::SetFFTWindowHigh(high) => {
                    config.fft_window[1] = high;
                    update = UpdateType::Filter(thread_communication.fft_index);
                }
                ConfigCommand::SetFFTLogPlot(log_plot) => {
                    config.fft_log_plot = log_plot;
                    update = UpdateType::Plot;
                }
                ConfigCommand::SetFFTNormalization(normalization) => {
                    config.normalize_fft = normalization;
                    update = UpdateType::Plot;
                }
                ConfigCommand::SetAvgInFourierSpace(avg_in_fourier_space) => {
                    config.avg_in_fourier_space = avg_in_fourier_space;
                    update = UpdateType::Filter(thread_communication.fft_index);
                }
                ConfigCommand::SetFFTResolution(df) => {
                    config.fft_df = df;
                    update = UpdateType::Plot;
                }
                ConfigCommand::SetFftWindowType(wt) => {
                    config.fft_window_type = wt;
                    update = UpdateType::Filter(thread_communication.fft_index);
                }
                ConfigCommand::SetDownScaling => {
                    if let Ok(scaling) = thread_communication.scaling_lock.read() {
                        //scan.scaling = *scaling as usize;
                        // scan.rescale()
                    }
                    // TODO implement downscaling!
                    log::error!("scaling is not supported yet!");
                    update = UpdateType::Filter(1);
                }
                ConfigCommand::SetSelectedPixel(pixel) => {
                    selected_pixel = pixel.clone();
                    // if let Ok(scaling) = thread_communication.scaling_lock.read() {
                    //     scan.scaling = *scaling as usize;
                    //     scan.rescale()
                    // }

                    // TODO how do we update the traces in the filter panel??
                    update = UpdateType::Plot;
                }
                ConfigCommand::UpdateFilters => {
                    println!("update filters");
                    update = UpdateType::Filter(1);
                }
                ConfigCommand::UpdateFilter(uuid) => {
                    if let Ok(filter_uuid_to_index) =
                        thread_communication.filter_uuid_to_index_lock.read()
                    {
                        if let Some(&idx) = filter_uuid_to_index.get(&uuid) {
                            update = UpdateType::Filter(idx);
                        } else {
                            log::warn!("Filter uuid {} not found in filter_uuid_to_index", uuid);
                            update = UpdateType::None;
                        }
                    } else {
                        log::error!("Could not acquire filter_uuid_to_index_lock");
                        update = UpdateType::None;
                    }
                }
                ConfigCommand::UpdateMaterialCalculation => {
                    update = UpdateType::Plot;
                }
                ConfigCommand::AddROI(roi) => {
                    update = UpdateType::Filter(1);
                    if let Ok(mut data) = thread_communication.data_lock.write() {
                        data.rois.push(roi.clone());
                    }
                    if let Ok(mut filter_data) = thread_communication.filter_data_lock.write() {
                        if let Some(input) = filter_data.first_mut() {
                            input.rois.insert(
                                roi.name,
                                roi.polygon
                                    .iter()
                                    .map(|v| (v[0] as usize, v[1] as usize))
                                    .collect(),
                            );
                        }
                    }
                }
                ConfigCommand::UpdateROI(old_name, roi) => {
                    update = UpdateType::Filter(1);
                    if let Ok(mut data) = thread_communication.data_lock.write() {
                        data.rois.retain(|roi| roi.name != old_name);
                    }
                    if let Ok(mut filter_data) = thread_communication.filter_data_lock.write() {
                        if let Some(input) = filter_data.first_mut() {
                            input.rois.remove(&old_name);
                        }
                    }
                    if let Ok(mut data) = thread_communication.data_lock.write() {
                        data.rois.push(roi.clone());
                    }
                    if let Ok(mut filter_data) = thread_communication.filter_data_lock.write() {
                        if let Some(input) = filter_data.first_mut() {
                            input.rois.insert(
                                roi.name,
                                roi.polygon
                                    .iter()
                                    .map(|v| (v[0] as usize, v[1] as usize))
                                    .collect(),
                            );
                        }
                    }
                }
                ConfigCommand::DeleteROI(name) => {
                    update = UpdateType::Plot;
                    if let Ok(mut data) = thread_communication.data_lock.write() {
                        data.rois.retain(|roi| roi.name != name);
                    }
                    if let Ok(mut filter_data) = thread_communication.filter_data_lock.write() {
                        if let Some(input) = filter_data.first_mut() {
                            input.rois.remove(&name);
                        }
                    }
                }
                ConfigCommand::SetSample(name) => {
                    update = UpdateType::Plot;
                    sample_roi = name;
                }
                ConfigCommand::SetReference(name) => {
                    update = UpdateType::Plot;
                    reference_roi = name;
                }
            }

            if let Ok(pixel) = thread_communication.pixel_lock.read() {
                selected_pixel = pixel.clone();
            }

            match update {
                UpdateType::Filter(start_idx) => {
                    // updating back the static fields
                    // Get the filter chain and uuid->index map
                    if reset_filters {
                        if let Ok(mut filters) = FILTER_REGISTRY.lock() {
                            if let Ok(filter_chain) = thread_communication.filter_chain_lock.read()
                            {
                                if let Ok(filter_uuid_to_index) =
                                    thread_communication.filter_uuid_to_index_lock.read()
                                {
                                    if let Ok(filter_data) =
                                        thread_communication.filter_data_lock.read()
                                    {
                                        for (i, uuid) in filter_chain.iter().enumerate() {
                                            let input_index = if i == 0 {
                                                0
                                            } else {
                                                *filter_uuid_to_index
                                                    .get(&filter_chain[i - 1])
                                                    .unwrap()
                                            };
                                            if let Some((_, filter)) = filters
                                                .filters
                                                .iter_mut()
                                                .find(|(id, _)| *id == uuid)
                                            {
                                                filter.reset(
                                                    &filter_data[input_index].time,
                                                    filter_data[input_index].data.shape(),
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    reset_filters = false;

                    let mut filters_cloned: Option<Vec<(String, Box<dyn Filter>)>> = None;

                    // we need to clone the filters out of the filter_registry, otherwise this
                    // would block the gui thread (because it is also required to update the filters)

                    if let Ok(filters) = FILTER_REGISTRY.lock() {
                        // Clone (uuid, filter) pairs
                        filters_cloned = Some(
                            filters
                                .filters
                                .iter()
                                .map(|(uuid, filter)| (uuid.clone(), filter.clone()))
                                .collect::<Vec<(String, Box<dyn Filter>)>>(),
                        );
                    }

                    let mut run_deconvolution = true;

                    if let Some(ref mut filters) = filters_cloned {
                        if let Ok(filter_chain) = thread_communication.filter_chain_lock.read() {
                            if let Ok(mut filter_data) =
                                thread_communication.filter_data_lock.write()
                            {
                                if let Ok(filter_uuid_to_index) =
                                    thread_communication.filter_uuid_to_index_lock.read()
                                {
                                    for (i, filter_id) in
                                        filter_chain.iter().enumerate().skip(start_idx)
                                    {
                                        let output_index =
                                            *filter_uuid_to_index.get(filter_id).unwrap();
                                        let input_index = *filter_uuid_to_index
                                            .get(&filter_chain[i - 1])
                                            .unwrap();

                                        if filter_data[input_index].time.is_empty() {
                                            log::warn!(
                                                "Input data for filter {} is empty, skipping filter application",
                                                filter_id
                                            );
                                            continue;
                                        }

                                        let start = Instant::now();
                                        match filter_id.as_str() {
                                            "fft" => {
                                                // println!("Performing FFT");
                                                // println!("{} -> {}", input_index, output_index);
                                                filter_data[output_index] =
                                                    fft(&filter_data[input_index], &config);
                                            }
                                            "ifft" => {
                                                // println!("Performing iFFT");
                                                // println!("{} -> {}", input_index, output_index);
                                                filter_data[output_index] =
                                                    ifft(&filter_data[input_index], &config);
                                            }
                                            uuid => {
                                                if let Some((_, filter)) =
                                                    filters.iter_mut().find(|(id, _)| id == uuid)
                                                {
                                                    // println!(
                                                    //     "Applying filter: {}",
                                                    //     filter.config().name
                                                    // );
                                                    // println!("{} -> {}", input_index, output_index);
                                                    let active = if let Ok(actives) =
                                                        thread_communication
                                                            .filters_active_lock
                                                            .read()
                                                    {
                                                        if let Some(active) = actives.get(uuid) {
                                                            *active
                                                        } else {
                                                            false
                                                        }
                                                    } else {
                                                        false
                                                    };

                                                    let deconvolution = filter
                                                        .config()
                                                        .name
                                                        .contains("Deconvolution");

                                                    if !deconvolution {
                                                        // If we update a different filter, lets not update the deconvolution filter
                                                        run_deconvolution = false;
                                                    }
                                                    if active
                                                        && !(deconvolution && !run_deconvolution)
                                                    {
                                                        if let Some(progress) = thread_communication
                                                            .progress_lock
                                                            .get_mut(uuid)
                                                        {
                                                            filter_data[output_index] = filter
                                                                .filter(
                                                                    &filter_data[input_index],
                                                                    &mut thread_communication
                                                                        .gui_settings,
                                                                    progress,
                                                                    &thread_communication
                                                                        .abort_flag,
                                                                );
                                                        }

                                                        if let Ok(mut computation_time) =
                                                            thread_communication
                                                                .filter_computation_time_lock
                                                                .write()
                                                        {
                                                            match filter_id.as_str() {
                                                                "fft" => {}
                                                                "ifft" => {}
                                                                uuid => {
                                                                    computation_time.insert(
                                                                        uuid.to_string(),
                                                                        start.elapsed(),
                                                                    );
                                                                }
                                                            }
                                                        }
                                                    } else {
                                                        filter_data[output_index] =
                                                            filter_data[input_index].clone();
                                                    }
                                                }
                                            }
                                        }

                                        // check if we need to update the fft planner due to dimension changes in time or frequency domain
                                        if filter_data[input_index].time.len()
                                            != filter_data[output_index].time.len()
                                        {
                                            let n = filter_data[output_index].time.len();
                                            let rng = filter_data[output_index].time[n - 1]
                                                - filter_data[output_index].time[0];
                                            let mut real_planner = RealFftPlanner::<f32>::new();
                                            let r2c = real_planner.plan_fft_forward(n);
                                            let c2r = real_planner.plan_fft_inverse(n);
                                            let spectrum = r2c.make_output_vec();
                                            let freq = (0..spectrum.len())
                                                .map(|i| i as f32 / rng)
                                                .collect();
                                            filter_data[output_index].frequency = freq;

                                            filter_data[output_index].r2c = Some(r2c);
                                            filter_data[output_index].c2r = Some(c2r);

                                            filter_data[output_index].phases = Array3::zeros((
                                                filter_data[output_index].width,
                                                filter_data[output_index].height,
                                                filter_data[output_index].frequency.len(),
                                            ));
                                            filter_data[output_index].amplitudes = Array3::zeros((
                                                filter_data[output_index].width,
                                                filter_data[output_index].height,
                                                filter_data[output_index].frequency.len(),
                                            ));
                                            filter_data[output_index].fft = Array3::zeros((
                                                filter_data[output_index].width,
                                                filter_data[output_index].height,
                                                filter_data[output_index].frequency.len(),
                                            ));
                                        }
                                    }
                                }

                                // update intensity image
                                if let Some(filtered) = filter_data.last_mut() {
                                    (
                                        filtered.data.axis_iter(Axis(0)),
                                        filtered.img.axis_iter_mut(Axis(0)),
                                    )
                                        .into_par_iter()
                                        .for_each(
                                            |(data_columns, mut img_columns)| {
                                                (
                                                    data_columns.axis_iter(Axis(0)),
                                                    img_columns.axis_iter_mut(Axis(0)),
                                                )
                                                    .into_par_iter()
                                                    .for_each(|(data, img)| {
                                                        *img.into_scalar() = data
                                                            .iter()
                                                            .map(|xi| xi * xi)
                                                            .sum::<f32>();
                                                    });
                                            },
                                        );

                                    update_intensity_image(&filtered, &thread_communication);
                                }
                            }
                        }
                    }

                    // updating back the static fields
                    if let Ok(mut filters) = FILTER_REGISTRY.lock() {
                        if let Some(filters_cloned) = filters_cloned {
                            for (uuid, filter) in filters_cloned.iter() {
                                if let Some((_, filter_from_registry)) =
                                    filters.filters.iter_mut().find(|(id, _)| *id == uuid)
                                {
                                    filter_from_registry.copy_static_fields_from(filter.as_ref());
                                } else {
                                    log::warn!("Filter with uuid {} not found in registry", uuid);
                                }
                            }
                        }
                    }

                    // add selected pixel and avg data to the data lock for the plot
                    if let Ok(mut data) = thread_communication.data_lock.write() {
                        if let Ok(filter_data) = thread_communication.filter_data_lock.read() {
                            // raw trace
                            // time domain
                            if let Some(raw) = filter_data.first() {
                                if raw.data.dim().0 <= selected_pixel.x
                                    || raw.data.dim().1 <= selected_pixel.y
                                {
                                    log::warn!(
                                        "selected pixel ({}, {}) is out of bounds for raw data with shape {:?}",
                                        selected_pixel.x,
                                        selected_pixel.y,
                                        raw.data.shape()
                                        );
                                    continue;
                                }
                                data.time = raw.time.to_vec();
                                data.signal = raw
                                    .data
                                    .index_axis(Axis(0), selected_pixel.x)
                                    .index_axis(Axis(0), selected_pixel.y)
                                    .to_vec();
                            }

                            // raw trace
                            // frequency domain
                            if let Some(raw) =
                                filter_data.iter().nth(thread_communication.fft_index + 1)
                            {
                                // frequency domain
                                data.frequencies = raw.frequency.to_vec();
                                data.signal_fft = raw
                                    .amplitudes
                                    .index_axis(Axis(0), selected_pixel.x)
                                    .index_axis(Axis(0), selected_pixel.y)
                                    .to_vec();
                                data.phase_fft = raw
                                    .phases
                                    .index_axis(Axis(0), selected_pixel.x)
                                    .index_axis(Axis(0), selected_pixel.y)
                                    .to_vec();
                            }

                            // filtered trace
                            if let Some(filtered) = filter_data.last() {
                                data.filtered_time = filtered.time.to_vec();
                                data.filtered_signal = filtered
                                    .data
                                    .index_axis(Axis(0), selected_pixel.x)
                                    .index_axis(Axis(0), selected_pixel.y)
                                    .to_vec();
                                // frequency domain
                                data.filtered_frequencies = filtered.frequency.to_vec();
                                data.filtered_signal_fft = filtered
                                    .amplitudes
                                    .index_axis(Axis(0), selected_pixel.x)
                                    .index_axis(Axis(0), selected_pixel.y)
                                    .to_vec();
                                data.filtered_phase_fft = filtered
                                    .phases
                                    .index_axis(Axis(0), selected_pixel.x)
                                    .index_axis(Axis(0), selected_pixel.y)
                                    .to_vec();

                                // averaged
                                if !config.avg_in_fourier_space {
                                    data.avg_signal = filtered
                                        .data
                                        .mean_axis(Axis(0))
                                        .expect("Axis 2 mean failed")
                                        .mean_axis(Axis(0))
                                        .expect("Axis 1 mean failed")
                                        .to_vec();
                                } else {
                                    data.avg_signal = filtered.avg_data.to_vec();
                                }
                                data.avg_signal_fft = filtered.avg_signal_fft.to_vec();
                                data.avg_phase_fft = filtered.avg_phase_fft.to_vec();

                                // Update ROIs data using average_polygon_roi
                                if !filtered.rois.is_empty() {
                                    // Clear existing ROI data
                                    // data.roi_signal.clear();
                                    // data.roi_signal_fft.clear();
                                    // data.roi_phase.clear();

                                    // Process each ROI
                                    for (roi_name, polygon) in &filtered.rois {
                                        // Time domain ROI averaging
                                        let roi_signal =
                                            average_polygon_roi(&filtered.data, polygon);
                                        data.roi_signal
                                            .insert(roi_name.clone(), roi_signal.to_vec());

                                        // Frequency domain ROI averaging (amplitudes)
                                        let roi_signal_fft =
                                            average_polygon_roi(&filtered.amplitudes, polygon);
                                        data.roi_signal_fft
                                            .insert(roi_name.clone(), roi_signal_fft.to_vec());

                                        // Frequency domain ROI averaging (phases)
                                        let roi_phase =
                                            average_polygon_roi(&filtered.phases, polygon);
                                        data.roi_phase.insert(roi_name.clone(), roi_phase.to_vec());
                                    }

                                    if config.avg_in_fourier_space {
                                        for (roi_name, roi_array) in &filtered.roi_data {
                                            data.roi_signal
                                                .insert(roi_name.clone(), roi_array.to_vec());
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if let Ok(filter_data) = thread_communication.filter_data_lock.read() {
                        if let Some(filtered) = filter_data.last() {
                            // Get sample thickness from GUI settings (in meters)
                            // TODO!
                            let sample_thickness = 1.0 / 1e3; // explorer.data.sample_thickness;

                            // Get ROI data
                            if let (Some(reference_amplitude), Some(reference_phase)) = (
                                filtered.roi_signal_fft.get(&reference_roi),
                                filtered.roi_phase_fft.get(&reference_roi),
                            ) {
                                if &sample_roi == "Selected Pixel" {
                                    let amplitudes_x =
                                        filtered.amplitudes.index_axis(Axis(0), selected_pixel.x);
                                    let sample_amplitude =
                                        amplitudes_x.index_axis(Axis(0), selected_pixel.y);

                                    let phases_x =
                                        filtered.phases.index_axis(Axis(0), selected_pixel.x);
                                    let sample_phase =
                                        phases_x.index_axis(Axis(0), selected_pixel.y);

                                    let (refractive_index, absorption_coeff, extinction_coeff) =
                                        calculate_optical_properties(
                                            sample_amplitude,
                                            sample_phase,
                                            reference_amplitude.view(),
                                            reference_phase.view(),
                                            filtered.frequency.view(),
                                            sample_thickness,
                                        );

                                    // Store the calculated data
                                    if let Ok(mut data) = thread_communication.data_lock.write() {
                                        data.refractive_index = refractive_index.to_vec();
                                        data.absorption_coefficient = absorption_coeff.to_vec();
                                        data.extinction_coefficient = extinction_coeff.to_vec();
                                    }
                                } else if let (Some(sample_amplitude), Some(sample_phase)) = (
                                    filtered.roi_signal_fft.get(&sample_roi),
                                    &filtered.roi_phase_fft.get(&sample_roi),
                                ) {
                                    let (refractive_index, absorption_coeff, extinction_coeff) =
                                        calculate_optical_properties(
                                            sample_amplitude.view(),
                                            sample_phase.view(),
                                            reference_amplitude.view(),
                                            reference_phase.view(),
                                            filtered.frequency.view(),
                                            sample_thickness,
                                        );

                                    // Store the calculated data
                                    if let Ok(mut data) = thread_communication.data_lock.write() {
                                        data.refractive_index = refractive_index.to_vec();
                                        data.absorption_coefficient = absorption_coeff.to_vec();
                                        data.extinction_coefficient = extinction_coeff.to_vec();
                                    }
                                };
                            }
                        } else {
                            log::warn!("No filtered data available for material calculation");
                        }
                    }
                    // TODO: request repaint
                }
                UpdateType::Image => {
                    // update intensity image
                    if let Ok(mut filter_data) = thread_communication.filter_data_lock.write() {
                        if let Some(filtered) = filter_data.last_mut() {
                            (
                                filtered.data.axis_iter(Axis(0)),
                                filtered.img.axis_iter_mut(Axis(0)),
                            )
                                .into_par_iter()
                                .for_each(
                                    |(data_columns, mut img_columns)| {
                                        (
                                            data_columns.axis_iter(Axis(0)),
                                            img_columns.axis_iter_mut(Axis(0)),
                                        )
                                            .into_par_iter()
                                            .for_each(
                                                |(data, img)| {
                                                    *img.into_scalar() =
                                                        data.iter().map(|xi| xi * xi).sum::<f32>();
                                                },
                                            );
                                    },
                                );

                            update_intensity_image(&filtered, &thread_communication);
                        }
                    }
                    // TODO: request repaint
                }
                UpdateType::Plot => {
                    // add selected pixel and avg data to the data lock for the plot

                    if let Ok(mut data) = thread_communication.data_lock.write() {
                        if let Ok(filter_data) = thread_communication.filter_data_lock.read() {
                            // raw trace
                            // time domain
                            if let Some(raw) = filter_data.first() {
                                if raw.data.dim().0 <= selected_pixel.x
                                    || raw.data.dim().1 <= selected_pixel.y
                                {
                                    log::warn!(
                                        "selected pixel ({}, {}) is out of bounds for raw data with shape {:?}",
                                        selected_pixel.x,
                                        selected_pixel.y,
                                        raw.data.shape()
                                        );
                                    continue;
                                }

                                data.time = raw.time.to_vec();
                                data.signal = raw
                                    .data
                                    .index_axis(Axis(0), selected_pixel.x)
                                    .index_axis(Axis(0), selected_pixel.y)
                                    .to_vec();
                            }

                            // raw trace
                            // frequency domain
                            if let Some(raw) =
                                filter_data.iter().nth(thread_communication.fft_index + 1)
                            {
                                // frequency domain
                                data.frequencies = raw.frequency.to_vec();
                                data.signal_fft = raw
                                    .amplitudes
                                    .index_axis(Axis(0), selected_pixel.x)
                                    .index_axis(Axis(0), selected_pixel.y)
                                    .to_vec();
                                data.phase_fft = raw
                                    .phases
                                    .index_axis(Axis(0), selected_pixel.x)
                                    .index_axis(Axis(0), selected_pixel.y)
                                    .to_vec();
                            }

                            // filtered trace
                            if let Some(filtered) = filter_data.last() {
                                data.filtered_time = filtered.time.to_vec();
                                data.filtered_signal = filtered
                                    .data
                                    .index_axis(Axis(0), selected_pixel.x)
                                    .index_axis(Axis(0), selected_pixel.y)
                                    .to_vec();
                                // frequency domain
                                data.filtered_frequencies = filtered.frequency.to_vec();
                                data.filtered_signal_fft = filtered
                                    .amplitudes
                                    .index_axis(Axis(0), selected_pixel.x)
                                    .index_axis(Axis(0), selected_pixel.y)
                                    .to_vec();
                                data.filtered_phase_fft = filtered
                                    .phases
                                    .index_axis(Axis(0), selected_pixel.x)
                                    .index_axis(Axis(0), selected_pixel.y)
                                    .to_vec();

                                // averaged
                                if !config.avg_in_fourier_space {
                                    data.avg_signal = filtered
                                        .data
                                        .mean_axis(Axis(0))
                                        .expect("Axis 2 mean failed")
                                        .mean_axis(Axis(0))
                                        .expect("Axis 1 mean failed")
                                        .to_vec();
                                } else {
                                    data.avg_signal = filtered.avg_data.to_vec();
                                }
                                data.avg_signal_fft = filtered.avg_signal_fft.to_vec();
                                data.avg_phase_fft = filtered.avg_phase_fft.to_vec();
                            }
                        }
                    }

                    if let Ok(filter_data) = thread_communication.filter_data_lock.read() {
                        if let Some(filtered) = filter_data.last() {
                            // Get sample thickness from GUI settings (in meters)
                            // TODO
                            let sample_thickness = 1.0 / 1e3; // explorer.data.sample_thickness;

                            // Get ROI data
                            if let (Some(reference_amplitude), Some(reference_phase)) = (
                                filtered.roi_signal_fft.get(&reference_roi),
                                filtered.roi_phase_fft.get(&reference_roi),
                            ) {
                                if &sample_roi == "Selected Pixel" {
                                    let amplitudes_x =
                                        filtered.amplitudes.index_axis(Axis(0), selected_pixel.x);
                                    let sample_amplitude =
                                        amplitudes_x.index_axis(Axis(0), selected_pixel.y);

                                    let phases_x =
                                        filtered.phases.index_axis(Axis(0), selected_pixel.x);
                                    let sample_phase =
                                        phases_x.index_axis(Axis(0), selected_pixel.y);

                                    let (refractive_index, absorption_coeff, extinction_coeff) =
                                        calculate_optical_properties(
                                            sample_amplitude,
                                            sample_phase,
                                            reference_amplitude.view(),
                                            reference_phase.view(),
                                            filtered.frequency.view(),
                                            sample_thickness,
                                        );

                                    // Store the calculated data
                                    if let Ok(mut data) = thread_communication.data_lock.write() {
                                        data.refractive_index = refractive_index.to_vec();
                                        data.absorption_coefficient = absorption_coeff.to_vec();
                                        data.extinction_coefficient = extinction_coeff.to_vec();
                                    }
                                } else if let (Some(sample_amplitude), Some(sample_phase)) = (
                                    filtered.roi_signal_fft.get(&sample_roi),
                                    &filtered.roi_phase_fft.get(&sample_roi),
                                ) {
                                    let (refractive_index, absorption_coeff, extinction_coeff) =
                                        calculate_optical_properties(
                                            sample_amplitude.view(),
                                            sample_phase.view(),
                                            reference_amplitude.view(),
                                            reference_phase.view(),
                                            filtered.frequency.view(),
                                            sample_thickness,
                                        );

                                    // Store the calculated data
                                    if let Ok(mut data) = thread_communication.data_lock.write() {
                                        data.refractive_index = refractive_index.to_vec();
                                        data.absorption_coefficient = absorption_coeff.to_vec();
                                        data.extinction_coefficient = extinction_coeff.to_vec();
                                    }
                                };
                            }
                        } else {
                            log::warn!("No filtered data available for material calculation");
                        }
                    }
                    // println!("updated plot.This took {: ?}", start.elapsed());
                    // TODO: request repaint
                }
                UpdateType::None => {
                    // do nothing
                }
            }
        }
    }
}
