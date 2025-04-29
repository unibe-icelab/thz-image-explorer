//! This module deals with processing and visualizing scanned image data.
//! It includes methods for filtering, updating intensity images, saving images, and managing
//! FFT operations for signal processing.

use crate::config::{ConfigCommand, ConfigContainer, ThreadCommunication};
use crate::data_container::{DataPoint, ScannedImage};
use crate::filters::filter::FILTER_REGISTRY;
use crate::gui::matrix_plot::SelectedPixel;
use crate::io::{
    load_meta_data_of_thz_file, open_from_npz, open_from_thz, open_json, save_to_thz,
    update_meta_data_of_thz_file,
};
use crate::math_tools::{
    apply_adapted_blackman_window, apply_blackman, apply_flat_top, apply_hamming, apply_hanning,
    apply_soft_bandpass, numpy_unwrap, FftWindowType,
};
use bevy_egui::egui::ColorImage;
use csv::ReaderBuilder;
use dotthz::DotthzMetaData;
use image::RgbaImage;
use ndarray::parallel::prelude::*;
use ndarray::{Array1, Axis};
use realfft::num_complex::Complex;
use realfft::num_traits::Zero;
use std::f32::consts::PI;
use std::path::Path;
use std::time::Instant;

/// Saves an image to a given file location.
///
/// The image is saved as PNG in the specified directory.
/// Currently, the function does not support saving large images.
///
/// # Arguments
/// * `img` - The `ColorImage` object to be saved.
/// * `file_path` - The directory path where the image will be saved.
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
fn update_intensity_image(scan: &ScannedImage, thread_communication: &ThreadCommunication) {
    if let Ok(mut write_guard) = thread_communication.img_lock.write() {
        *write_guard = scan.filtered_img.clone();
    }
    if let Ok(mut write_guard) = thread_communication.filtered_data_lock.write() {
        *write_guard = scan.filtered_data.clone();
    }
    if let Ok(mut write_guard) = thread_communication.filtered_time_lock.write() {
        *write_guard = scan.filtered_time.clone();
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

        let time_span = scan.filtered_time.last().unwrap() - scan.filtered_time.first().unwrap();
        let (instances, cube_width, cube_height, cube_depth) =
            crate::gui::threed_plot::instance_from_data(time_span, scan.filtered_data.clone(), thread_communication.gui_settings.opacity_threshold);
        write_guard.0 = instances;
        write_guard.1 = cube_width;
        write_guard.2 = cube_height;
        write_guard.3 = cube_depth;
    }
}

/// Filters the scan data using a time window defined in the configuration container.
///
/// This function calculates the filtered data based on the lower and upper time window bounds and updates the intensity image.
///
/// # Arguments
/// * `config` - The configuration container with filter parameters.
/// * `scan` - A mutable reference to the scanned image data.
/// * `img_lock` - A thread-safe lock for the intensity image array.
fn filter_time_window(
    config: &ConfigContainer,
    scan: &mut ScannedImage,
    thread_communication: &ThreadCommunication,
) {
    use ndarray::{s, Axis};
    use rayon::prelude::*;
    use std::time::Instant;

    println!("updating data");
    let start = Instant::now();

    let lower = scan
        .filtered_time
        .iter()
        .position(|t| *t == config.time_window[0].round())
        .unwrap_or(0);
    let upper = scan
        .filtered_time
        .iter()
        .position(|t| *t == config.time_window[1].round())
        .unwrap_or(scan.filtered_time.len()); // safer fallback

    let range = lower..upper;

    let filtered_iter = scan.filtered_data.axis_iter_mut(Axis(0));
    let img_iter = scan.filtered_img.axis_iter_mut(Axis(0));

    filtered_iter
        .zip(img_iter)
        .par_bridge() // parallelize outer loop only
        .for_each(|(filtered_data_col, mut filtered_img_col)| {
            for i in 0..filtered_data_col.len_of(Axis(0)) {
                let data_slice = filtered_data_col.index_axis(Axis(0), i);
                let sum_sq = data_slice
                    .slice(s![range.clone()])
                    .iter()
                    .map(|&xi| xi * xi)
                    .sum::<f32>();
                filtered_img_col[i] = sum_sq;
            }
        });

    update_intensity_image(scan, thread_communication);
    println!("updated time data. This took {:?}", start.elapsed());
}

/// Performs FFT-based filtering on a scan based on the configuration.
///
/// The function applies a specific FFT window and a bandpass filter to scale and filter the data.
/// It uses the FFT routines defined in the scan object.
///
/// # Arguments
/// * `config` - Configuration settings for the FFT and filtering process.
/// * `scan` - A mutable reference to the scanned image data.
/// * `img_lock` - A thread-safe lock for the intensity image array.
fn filter(
    config: &ConfigContainer,
    thread_communication: &mut ThreadCommunication,
    scan: &mut ScannedImage,
) {
    // calculate fft filter and calculate ifft
    let start = Instant::now();
    let lower = scan
        .filtered_time
        .iter()
        .position(|t| *t == config.time_window[0].round())
        .unwrap_or(0);
    let upper = scan
        .filtered_time
        .iter()
        .position(|t| *t == config.time_window[1].round())
        .unwrap_or(scan.filtered_time.len());
    if let Some(r2c) = &scan.filtered_r2c {
        // scan.filtered_img =
        //     Array2::zeros((scan.filtered_data.shape()[0], scan.filtered_data.shape()[1]));
        // scan.filtered_data = Array3::zeros((
        //     scan.filtered_data.shape()[0],
        //     scan.filtered_data.shape()[1],
        //     scan.filtered_data.shape()[2],
        // ));
        if let Some(c2r) = &scan.filtered_c2r {
            (
                scan.scaled_data.axis_iter_mut(Axis(0)),
                scan.filtered_data.axis_iter_mut(Axis(0)),
                scan.filtered_img.axis_iter_mut(Axis(0)),
            )
                .into_par_iter()
                .for_each(
                    |(
                        mut scaled_data_columns,
                        mut filtered_data_columns,
                        mut filtered_img_columns,
                    )| {
                        let mut output = c2r.make_output_vec();
                        let mut input = vec![0.0; scan.filtered_time.len()];
                        let mut spectrum = r2c.make_output_vec();
                        for ((scaled_data, mut filtered_data), filtered_img) in scaled_data_columns
                            .axis_iter_mut(Axis(0))
                            .zip(filtered_data_columns.axis_iter_mut(Axis(0)))
                            .zip(filtered_img_columns.axis_iter_mut(Axis(0)))
                        {
                            // filtered_data.assign(&filtered_data);

                            match config.fft_window_type {
                                FftWindowType::AdaptedBlackman => {
                                    apply_adapted_blackman_window(
                                        &mut filtered_data,
                                        &scan.filtered_time,
                                        &config.fft_window[0],
                                        &config.fft_window[1],
                                    );
                                }
                                FftWindowType::Blackman => {
                                    apply_blackman(&mut filtered_data, &scan.filtered_time)
                                }
                                FftWindowType::Hanning => {
                                    apply_hanning(&mut filtered_data, &scan.filtered_time)
                                }
                                FftWindowType::Hamming => {
                                    apply_hamming(&mut filtered_data, &scan.filtered_time)
                                }
                                FftWindowType::FlatTop => {
                                    apply_flat_top(&mut filtered_data, &scan.filtered_time)
                                }
                            }

                            // calculate fft
                            // Forward transform the input data
                            input.clone_from_slice(filtered_data.as_slice().unwrap());
                            r2c.process(&mut input, &mut spectrum).unwrap();

                            apply_soft_bandpass(
                                &scan.frequencies,
                                &mut spectrum,
                                (config.fft_filter[0], config.fft_filter[1]),
                                0.1, // smooth transition over 100 Hz
                            );

                            // Forward transform the input data
                            match c2r.process(&mut spectrum, &mut output) {
                                Ok(_) => {}
                                Err(err) => {
                                    log::error!("error in iFFT: {err:?}");
                                }
                            };
                            let length = output.len();
                            let output = output
                                .iter()
                                .map(|p| *p / length as f32)
                                .collect::<Vec<f32>>();
                            *filtered_img.into_scalar() = output
                                .iter()
                                .skip(lower)
                                .take(upper - lower)
                                .map(|xi| xi * xi)
                                .sum::<f32>();
                            filtered_data.assign(&Array1::from_vec(output));
                        }
                    },
                );
        };

        for filter in FILTER_REGISTRY.lock().unwrap().iter_mut() {
            println!("Filter found: {}", filter.config().name);
            // filter.filter(scan, &mut thread_communication.gui_settings)
        }
    };
    // update images
    println!("updated fft. This took {:?}", start.elapsed());
    update_intensity_image(scan, &thread_communication);
    println!("updated and synced data. This took {:?}", start.elapsed());
}

/// Enum representing supported file types for reading data.
///
/// - `Npy`: Represents `.npy` files.
/// - `Npz`: Represents `.npz` files.
#[derive(Clone, PartialEq)]
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
    let mut scan = ScannedImage::default();
    let mut config = ConfigContainer::default();
    let mut selected_pixel = SelectedPixel::default();
    let mut meta_data = DotthzMetaData::default();
    let mut hk_csv = None;
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
                            // dave full file, not just metadata, since the dotTHz file does not exist yet.
                            if let Ok(md) = thread_communication.md_lock.read() {
                                match save_to_thz(&path, &scan, &md) {
                                    Ok(_) => {
                                        log::info!("saved {:?}", path);
                                    }
                                    Err(err) => {
                                        log::error!("failed saving {:?}: {:?}", path, err)
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
                    if let Some(file_ending) = selected_file_path.extension() {
                        match file_ending.to_str().unwrap() {
                            "npz" => {
                                // check if meta.json exists in the same folder
                                let width: usize;
                                let height: usize;

                                let json_path = selected_file_path.with_file_name("meta.json");

                                match open_json(&mut data.hk, &json_path) {
                                    Ok((w, h)) => {
                                        width = w;
                                        height = h;
                                    }
                                    Err(err) => {
                                        log::error!("failed to open json @ {json_path:?}... {err}");
                                        width = 0;
                                        height = 0;
                                    }
                                }

                                scan = ScannedImage::new(
                                    height,
                                    width,
                                    data.hk.x_range[0],
                                    data.hk.y_range[0],
                                    data.hk.dx,
                                    data.hk.dy,
                                );

                                let hk_path = selected_file_path.with_file_name("hk.csv");

                                // read HK
                                match ReaderBuilder::new()
                                    .delimiter(b',')
                                    .has_headers(true)
                                    .from_path(hk_path)
                                {
                                    Ok(rdr) => {
                                        hk_csv = Some(rdr);
                                    }
                                    Err(e) => {
                                        hk_csv = None;
                                        log::warn!("failed reading HK: {}", e)
                                    }
                                }

                                let pulse_path = selected_file_path.with_file_name("data.npz");

                                dbg!(&pulse_path);
                                match open_from_npz(&mut scan, &pulse_path) {
                                    Ok(_) => {
                                        update_intensity_image(&scan, &thread_communication);
                                    }
                                    Err(err) => {
                                        log::error!("an error occurred while trying to read data.npz {err:?}");
                                    }
                                }
                                // if let Some(r2c) = &scan.r2c {
                                //     if let Ok(mut data) = data_lock.write() {
                                //         data.time = scan.time.to_vec();
                                //         data.frequencies = scan.frequencies.to_vec();
                                //         data.signal_1 = scan
                                //             .raw_data
                                //             .index_axis(Axis(0), 0)
                                //             .index_axis(Axis(0), 0)
                                //             .to_vec();
                                //         let mut in_data: Vec<f32> = data.signal_1.to_vec();
                                //         let mut spectrum = r2c.make_output_vec();
                                //         // Forward transform the input data
                                //         r2c.process(&mut in_data, &mut spectrum).unwrap();
                                //         let amp: Vec<f32> =
                                //             spectrum.iter().map(|s| s.norm()).collect();
                                //         let phase: Vec<f32> =
                                //             spectrum.iter().map(|s| s.arg()).collect();
                                //         data.signal_1_fft = amp;
                                //         data.phase_1_fft = numpy_unwrap(&phase, Some(2.0 * PI));
                                //     }
                                // }
                            }
                            "npy" => {
                                log::error!("file ending not supported");
                            }
                            "thz" => {
                                match open_from_thz(&selected_file_path, &mut scan, &mut meta_data)
                                {
                                    Ok(_) => {
                                        log::info!("opened {:?}", selected_file_path);
                                        update_intensity_image(&scan, &thread_communication);
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
                            _ => {
                                log::warn!("file not supported: {:?} \n Close the file to connect to a spectrometer or open another file.", selected_file_path);
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

                    if let Ok(md) = thread_communication.md_lock.read() {
                        match save_to_thz(&path, &scan, &md) {
                            Ok(_) => {
                                log::info!("saved {:?}", path);
                            }
                            Err(err) => {
                                log::error!("failed saving {:?}: {:?}", path, err)
                            }
                        }
                    }
                }
                ConfigCommand::SetFFTWindowLow(low) => {
                    config.fft_window[0] = low;
                    filter(&config, &mut thread_communication, &mut scan);
                }
                ConfigCommand::SetFFTWindowHigh(high) => {
                    config.fft_window[1] = high;
                    filter(&config, &mut thread_communication, &mut scan);
                }
                ConfigCommand::SetFFTFilterLow(low) => {
                    config.fft_filter[0] = low;
                    filter(&config, &mut thread_communication, &mut scan);
                }
                ConfigCommand::SetFFTFilterHigh(high) => {
                    config.fft_filter[1] = high;
                    filter(&config, &mut thread_communication, &mut scan);
                }
                ConfigCommand::SetTimeWindow(time_window) => {
                    config.time_window = time_window;
                    filter_time_window(&config, &mut scan, &thread_communication);
                }
                ConfigCommand::SetFFTLogPlot(log_plot) => {
                    config.fft_log_plot = log_plot;
                    filter(&config, &mut thread_communication, &mut scan);
                }
                ConfigCommand::SetFFTNormalization(normalization) => {
                    config.normalize_fft = normalization;
                    filter(&config, &mut thread_communication, &mut scan);
                }
                ConfigCommand::SetFFTResolution(df) => {
                    config.fft_df = df;
                    filter(&config, &mut thread_communication, &mut scan);
                }
                ConfigCommand::SetFftWindowType(wt) => {
                    config.fft_window_type = wt;
                    filter(&config, &mut thread_communication, &mut scan);
                }
                ConfigCommand::SetDownScaling => {
                    if let Ok(scaling) = thread_communication.scaling_lock.read() {
                        scan.scaling = *scaling as usize;
                        scan.rescale()
                    }
                    filter(&config, &mut thread_communication, &mut scan);
                    update_intensity_image(&scan, &thread_communication);
                }
                ConfigCommand::SetSelectedPixel(pixel) => {
                    selected_pixel = pixel.clone();
                    if let Ok(scaling) = thread_communication.scaling_lock.read() {
                        scan.scaling = *scaling as usize;
                        scan.rescale()
                    }
                    println!("new pixel: {:} {:}", selected_pixel.x, selected_pixel.y);
                    // send HK?
                }
                ConfigCommand::UpdateFilters => {
                    println!("update filters");
                    if let Ok(mut filters) = FILTER_REGISTRY.lock() {
                        for filter in filters.iter_mut() {
                            // call the filter functions
                            println!("update filter: {}", filter.config().name);
                            filter.filter(&mut scan, &mut thread_communication.gui_settings)
                        }
                    }
                    filter_time_window(&config, &mut scan, &thread_communication);
                    // update the intensity image
                    update_intensity_image(&scan, &thread_communication);
                }
            }

            if let Ok(pixel) = thread_communication.pixel_lock.read() {
                selected_pixel = pixel.clone();
            }

            if let Some(r2c) = &scan.r2c {
                if let Ok(mut data) = thread_communication.data_lock.write() {
                    data.time = scan.time.to_vec();
                    data.signal_1 = scan
                        .scaled_data
                        .index_axis(Axis(0), selected_pixel.x / scan.scaling)
                        .index_axis(Axis(0), selected_pixel.y / scan.scaling)
                        .to_vec();
                    if let Some(ref mut hk) = hk_csv {
                        for result in hk.records() {
                            let record = result.unwrap();
                            // Check if the "pixel" column matches the target pixel
                            if record.get(0)
                                == Some(
                                    format!(
                                        "{:05}-{:05}",
                                        selected_pixel.x / scan.scaling,
                                        selected_pixel.y / scan.scaling,
                                    )
                                    .as_str(),
                                )
                            {
                                if let Some(rh_value) = record.get(9) {
                                    data.hk.ambient_humidity =
                                        rh_value.parse::<f64>().unwrap_or(0.0);
                                }
                                if let Some(rh_value) = record.get(8) {
                                    data.hk.ambient_pressure =
                                        rh_value.parse::<f64>().unwrap_or(0.0);
                                }
                                if let Some(rh_value) = record.get(6) {
                                    data.hk.sample_temperature =
                                        rh_value.parse::<f64>().unwrap_or(0.0);
                                }
                                if let Some(rh_value) = record.get(5) {
                                    data.hk.ambient_temperature =
                                        rh_value.parse::<f64>().unwrap_or(0.0);
                                }
                            }
                        }
                    }
                    let mut in_data: Vec<f32> = data.signal_1.to_vec();
                    let mut spectrum = r2c.make_output_vec();
                    // Forward transform the input data
                    r2c.process(&mut in_data, &mut spectrum).unwrap();
                    let amp: Vec<f32> = spectrum.iter().map(|s| s.norm()).collect();
                    let phase: Vec<f32> = spectrum.iter().map(|s| s.arg()).collect();
                    data.signal_1_fft = amp;
                    data.phase_1_fft = numpy_unwrap(&phase, Some(2.0 * PI));
                }
            }
            if let Some(r2c) = &scan.filtered_r2c {
                if let Ok(mut data) = thread_communication.data_lock.write() {
                    data.filtered_time = scan.filtered_time.to_vec();
                    data.frequencies = scan.frequencies.to_vec();
                    data.filtered_frequencies = scan.filtered_frequencies.to_vec();
                    // get avg and filtered data
                    data.filtered_signal_1 = scan
                        .filtered_data
                        .index_axis(Axis(0), selected_pixel.x / scan.scaling)
                        .index_axis(Axis(0), selected_pixel.y / scan.scaling)
                        .to_vec();

                    let mut in_data: Vec<f32> = data.filtered_signal_1.to_vec();
                    let mut spectrum = r2c.make_output_vec();
                    // Forward transform the input data
                    r2c.process(&mut in_data, &mut spectrum).unwrap();
                    let amp: Vec<f32> = spectrum.iter().map(|s| s.norm()).collect();
                    let phase: Vec<f32> = spectrum.iter().map(|s| s.arg()).collect();
                    data.filtered_signal_1_fft = amp;
                    data.filtered_phase_fft = numpy_unwrap(&phase, Some(2.0 * PI));

                    data.avg_signal_1 = scan
                        .filtered_data
                        .mean_axis(Axis(0))
                        .expect("Axis 2 mean failed")
                        .mean_axis(Axis(0))
                        .expect("Axis 1 mean failed")
                        .to_vec();

                    data.avg_signal_1_fft = vec![0.0; data.signal_1_fft.len()];
                    data.avg_phase_fft = vec![0.0; data.phase_1_fft.len()];

                    // for x in 0..scan.filtered_data.shape()[0] {
                    //     for y in 0..scan.filtered_data.shape()[1] {
                    //         let mut in_data: Vec<f32> = scan
                    //             .filtered_data
                    //             .index_axis(Axis(0), x)
                    //             .index_axis(Axis(0), y)
                    //             .to_vec();
                    //         let mut spectrum = r2c.make_output_vec();
                    //         // Forward transform the input data
                    //         r2c.process(&mut in_data, &mut spectrum).unwrap();
                    //         let amp: Vec<f32> = spectrum.iter().map(|s| s.norm()).collect();
                    //         let phase: Vec<f32> = spectrum.iter().map(|s| s.arg()).collect();
                    //         data.avg_signal_1_fft = data
                    //             .avg_signal_1_fft
                    //             .iter()
                    //             .zip(amp.iter()) // Combine the two iterators
                    //             .map(|(a, b)| a + b) // Add the elements together
                    //             .collect();
                    //         data.filtered_phase_fft = data
                    //             .filtered_phase_fft
                    //             .iter()
                    //             .zip(numpy_unwrap(&phase, Some(2.0 * PI)).iter()) // Combine the two iterators
                    //             .map(|(a, b)| a + b) // Add the elements together
                    //             .collect();
                    //     }
                    // }
                    data.avg_signal_1_fft = data
                        .avg_signal_1_fft
                        .iter()
                        .map(|&i| {
                            i / (scan.filtered_data.shape()[0] * scan.filtered_data.shape()[1])
                                as f32
                        }) // Perform division
                        .collect();
                    data.avg_phase_fft = data
                        .avg_phase_fft
                        .iter()
                        .map(|&i| {
                            i / (scan.filtered_data.shape()[0] * scan.filtered_data.shape()[1])
                                as f32
                        }) // Perform division
                        .collect();
                }
            }
        }
    }
}
