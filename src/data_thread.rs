//! This module deals with processing and visualizing scanned image data.
//! It includes methods for filtering, updating intensity images, saving images, and managing
//! FFT operations for signal processing.

use crate::config::{ConfigCommand, ConfigContainer, ThreadCommunication};
use crate::data_container::{DataPoint, ScannedImage, ScannedImageFilterData};
use crate::filters::filter::{Filter, FILTER_REGISTRY};
use crate::gui::matrix_plot::SelectedPixel;
use crate::io::{
    load_meta_data_of_thz_file, open_from_npz, open_from_thz, open_json, save_to_thz,
    update_meta_data_of_thz_file,
};
use crate::math_tools::{fft, ifft};
use bevy_egui::egui::ColorImage;
use csv::ReaderBuilder;
use dotthz::DotthzMetaData;
use image::RgbaImage;
use ndarray::parallel::prelude::*;
use ndarray::{Array3, Axis};
use realfft::RealFftPlanner;
use std::path::Path;
use std::time::Instant;

pub enum UpdateType {
    None,
    Filter(usize),
    Image,
    Plot,
}

/// Saves an image to a given file location.
///
/// The image is saved as PNG in the specified directory.
/// Currently, the function does not support saving large images.
///
/// # Arguments
/// * `img` - The `ColorImage` object to be saved.
/// * `file_path` - The directory path where the image will be saved.
#[allow(dead_code)]
fn save_image(img: &ColorImage, file_path: &Path) {
    let height = img.height();
    let width = img.width();
    let mut raw: Vec<u8> = vec![];
    for p in img.pixels.clone().iter() {
        raw.push(p.r());
        raw.push(p.g());
        raw.push(p.b());
        raw.push(p.a());
    }
    let img_to_save = RgbaImage::from_raw(width as u32, height as u32, raw)
        .expect("container should have the right size for the image dimensions");
    let mut image_path = file_path.to_path_buf();
    image_path.push("image.png");
    match img_to_save.save(image_path) {
        Ok(_) => {}
        Err(err) => {
            log::error!("error in saving image: {err:?}");
        }
    }
    //TODO: implement large image saving
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
    if let Ok(mut write_guard) = thread_communication.img_lock.write() {
        *write_guard = scan.img.clone();
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

/// Enum representing supported file types for reading data.
///
/// - `Npy`: Represents `.npy` files.
/// - `Npz`: Represents `.npz` files.
#[derive(Clone, PartialEq)]
#[allow(dead_code)]
enum FileType {
    Npy,
    Npz,
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
    let mut data = DataPoint::default();
    let mut config = ConfigContainer::default();
    let mut selected_pixel = SelectedPixel::default();
    let mut meta_data = DotthzMetaData::default();

    let mut reset_filters = false;

    let mut update = UpdateType::None;
    loop {
        if let Ok(config_command) = thread_communication.config_rx.recv() {
            match config_command {
                ConfigCommand::LoadMetaData(path) => {
                    if path.extension().unwrap() == "thz" {
                        match load_meta_data_of_thz_file(&path, &mut meta_data) {
                            Ok(_) => {
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
                    continue;
                }
                ConfigCommand::UpdateMetaData(mut path) => {
                    // THz Image Explorer always saves thz files
                    if let Some(os_path) = path.extension() {
                        if os_path != "thz" {
                            path.set_extension("thz");
                            // save full file, not just metadata, since the dotTHz file does not exist yet.
                            if let Ok(mut filter_data) =
                                thread_communication.filter_data_lock.write()
                            {
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
                ConfigCommand::OpenFile(selected_file_path) => {
                    update = UpdateType::Filter(1);
                    reset_filters = true;
                    if let Some(file_ending) = selected_file_path.extension() {
                        match file_ending.to_str().unwrap() {
                            "thz" => {
                                if let Ok(mut filter_data) =
                                    thread_communication.filter_data_lock.write()
                                {
                                    if let Some(mut input) = filter_data.first_mut() {
                                        match open_from_thz(
                                            &selected_file_path,
                                            &mut input,
                                            &mut meta_data,
                                        ) {
                                            Ok(_) => {
                                                log::info!("opened {:?}", selected_file_path);
                                                //update_intensity_image(&scan, &thread_communication);
                                            }
                                            Err(err) => {
                                                log::error!(
                                                    "failed opening {:?}: {:?}",
                                                    selected_file_path,
                                                    err
                                                )
                                            }
                                        };
                                        if let Ok(mut md) = thread_communication.md_lock.write() {
                                            *md = meta_data.clone();
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
                    if path.extension().unwrap() != "thz" {
                        path.set_extension("thz");
                    }

                    if let Ok(mut filter_data) = thread_communication.filter_data_lock.write() {
                        // note, we save the input data, not the filtered data
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
                    println!("new pixel: {:} {:}", selected_pixel.x, selected_pixel.y);
                    update = UpdateType::Plot;
                }
                ConfigCommand::UpdateFilters => {
                    println!("update filters");
                    update = UpdateType::Filter(1);
                }
                ConfigCommand::UpdateSelectedFilters(indices) => {
                    update = UpdateType::Filter(indices.into_iter().min().unwrap_or(1));
                }
            }

            if let Ok(pixel) = thread_communication.pixel_lock.read() {
                selected_pixel = pixel.clone();
            }

            match update {
                UpdateType::Filter(start_idx) => {
                    let start = Instant::now();

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

                                        let start = Instant::now();
                                        match filter_id.as_str() {
                                            "fft" => {
                                                println!("Performing FFT");
                                                println!("{} -> {}", input_index, output_index);
                                                filter_data[output_index] =
                                                    fft(&filter_data[input_index], &config);
                                            }
                                            "ifft" => {
                                                println!("Performing iFFT");
                                                println!("{} -> {}", input_index, output_index);
                                                filter_data[output_index] =
                                                    ifft(&filter_data[input_index], &config);
                                            }
                                            uuid => {
                                                if let Some((_, filter)) =
                                                    filters.iter_mut().find(|(id, _)| id == uuid)
                                                {
                                                    println!(
                                                        "Applying filter: {}",
                                                        filter.config().name
                                                    );
                                                    println!("{} -> {}", input_index, output_index);
                                                    if let Some(progress) = thread_communication
                                                        .progress_lock
                                                        .get_mut(uuid)
                                                    {
                                                        filter_data[output_index] = filter.filter(
                                                            &filter_data[input_index],
                                                            &mut thread_communication.gui_settings,
                                                            progress,
                                                            &thread_communication.abort_flag,
                                                        );
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

                                        println!(
                                            "finished applying filter {}. This took {:?}",
                                            i,
                                            start.elapsed()
                                        );
                                    }
                                }

                                println!("finished applying filters");
                                println!("updating intensity image...");

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
                        if let Some(mut filters_cloned) = filters_cloned {
                            for (uuid, filter) in filters_cloned.iter() {
                                if let Some((_, filter_from_registry)) =
                                    filters.filters.iter_mut().find(|(id, _)| *id == uuid)
                                {
                                    filter_from_registry.copy_static_fields_from(filter.as_ref());
                                    if reset_filters {
                                        if let Ok(mut filter_data) =
                                            thread_communication.filter_data_lock.write()
                                        {
                                            if let Ok(filter_chain) =
                                                thread_communication.filter_chain_lock.read()
                                            {
                                                if let Ok(filter_uuid_to_index) =
                                                    thread_communication
                                                        .filter_uuid_to_index_lock
                                                        .read()
                                                {
                                                    for (i, filter_id) in filter_chain
                                                        .iter()
                                                        .enumerate()
                                                        .skip(start_idx)
                                                    {
                                                        let input_index = *filter_uuid_to_index
                                                            .get(&filter_chain[i - 1])
                                                            .unwrap();
                                                        filter_from_registry.reset(
                                                            &filter_data[input_index].time,
                                                            filter_data[input_index].data.shape(),
                                                        )
                                                    }
                                                }
                                            }
                                        }
                                        reset_filters = false;
                                    }
                                } else {
                                    log::warn!("Filter with uuid {} not found in registry", uuid);
                                }
                            }
                        }
                    }
                    println!("updating plots...");

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
                                data.signal_1 = raw
                                    .data
                                    .index_axis(Axis(0), selected_pixel.x)
                                    .index_axis(Axis(0), selected_pixel.y)
                                    .to_vec();
                                // frequency domain
                                data.frequencies = raw.frequency.to_vec();
                                data.signal_1_fft = raw
                                    .amplitudes
                                    .index_axis(Axis(0), selected_pixel.x)
                                    .index_axis(Axis(0), selected_pixel.y)
                                    .to_vec();
                                data.phase_1_fft = raw
                                    .phases
                                    .index_axis(Axis(0), selected_pixel.x)
                                    .index_axis(Axis(0), selected_pixel.y)
                                    .to_vec();
                            }

                            // filtered trace
                            if let Some(filtered) = filter_data.last() {
                                data.filtered_time = filtered.time.to_vec();
                                data.filtered_signal_1 = filtered
                                    .data
                                    .index_axis(Axis(0), selected_pixel.x)
                                    .index_axis(Axis(0), selected_pixel.y)
                                    .to_vec();
                                // frequency domain
                                data.filtered_frequencies = filtered.frequency.to_vec();
                                data.filtered_signal_1_fft = filtered
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
                                data.avg_signal_1 = filtered
                                    .data
                                    .mean_axis(Axis(0))
                                    .expect("Axis 2 mean failed")
                                    .mean_axis(Axis(0))
                                    .expect("Axis 1 mean failed")
                                    .to_vec();
                                data.avg_signal_1_fft = filtered
                                    .amplitudes
                                    .mean_axis(Axis(0))
                                    .expect("Axis 2 mean failed")
                                    .mean_axis(Axis(0))
                                    .expect("Axis 1 mean failed")
                                    .to_vec();
                                data.avg_phase_fft = filtered
                                    .phases
                                    .mean_axis(Axis(0))
                                    .expect("Axis 2 mean failed")
                                    .mean_axis(Axis(0))
                                    .expect("Axis 1 mean failed")
                                    .to_vec();
                            }
                        }
                    }

                    println!(
                        "updated filters starting with {}. This took {:?}",
                        start_idx,
                        start.elapsed()
                    );
                }
                UpdateType::Image => {
                    // update intensity image
                    let start = Instant::now();
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
                    println!("updated image. This took {:?}", start.elapsed());
                }
                UpdateType::Plot => {
                    // add selected pixel and avg data to the data lock for the plot
                    let start = Instant::now();

                    if let Ok(mut data) = thread_communication.data_lock.write() {
                        if let Ok(filter_data) = thread_communication.filter_data_lock.read() {
                            // raw trace
                            // time domain
                            if let Some(raw) = filter_data.first() {
                                data.time = raw.time.to_vec();
                                data.signal_1 = raw
                                    .data
                                    .index_axis(Axis(0), selected_pixel.x)
                                    .index_axis(Axis(0), selected_pixel.y)
                                    .to_vec();
                                // frequency domain
                                data.frequencies = raw.frequency.to_vec();
                                data.signal_1_fft = raw
                                    .amplitudes
                                    .index_axis(Axis(0), selected_pixel.x)
                                    .index_axis(Axis(0), selected_pixel.y)
                                    .to_vec();
                                data.phase_1_fft = raw
                                    .phases
                                    .index_axis(Axis(0), selected_pixel.x)
                                    .index_axis(Axis(0), selected_pixel.y)
                                    .to_vec();
                            }

                            // filtered trace
                            if let Some(filtered) = filter_data.last() {
                                data.filtered_time = filtered.time.to_vec();
                                data.filtered_signal_1 = filtered
                                    .data
                                    .index_axis(Axis(0), selected_pixel.x)
                                    .index_axis(Axis(0), selected_pixel.y)
                                    .to_vec();
                                // frequency domain
                                data.filtered_frequencies = filtered.frequency.to_vec();
                                data.filtered_signal_1_fft = filtered
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
                                data.avg_signal_1 = filtered
                                    .data
                                    .mean_axis(Axis(0))
                                    .expect("Axis 2 mean failed")
                                    .mean_axis(Axis(0))
                                    .expect("Axis 1 mean failed")
                                    .to_vec();
                                data.avg_signal_1_fft = filtered
                                    .amplitudes
                                    .mean_axis(Axis(0))
                                    .expect("Axis 2 mean failed")
                                    .mean_axis(Axis(0))
                                    .expect("Axis 1 mean failed")
                                    .to_vec();
                                data.avg_phase_fft = filtered
                                    .phases
                                    .mean_axis(Axis(0))
                                    .expect("Axis 2 mean failed")
                                    .mean_axis(Axis(0))
                                    .expect("Axis 1 mean failed")
                                    .to_vec();
                            }
                        }
                    }
                    println!("updated plot. This took {:?}", start.elapsed());
                }
                UpdateType::None => {
                    // do nothing
                }
            }
        }
    }
}
