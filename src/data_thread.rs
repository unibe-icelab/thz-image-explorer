use crate::config::{Config, ConfigContainer};
use crate::data::{DataPoint, ScannedImage};
use crate::io::{open_from_npz, open_from_thz, open_json};
use crate::math_tools::{apply_fft_window, numpy_unwrap};
use crate::matrix_plot::SelectedPixel;
use csv::ReaderBuilder;
use dotthz::DotthzMetaData;
use eframe::egui::ColorImage;
use image::RgbaImage;
use ndarray::parallel::prelude::*;
use ndarray::{Array1, Array2, Array3, Axis};
use realfft::num_complex::Complex32;
use std::f32::consts::PI;
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, RwLock};
use std::time::Instant;

fn save_image(img: &ColorImage, file_path: &PathBuf) {
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
    let mut image_path = file_path.clone();
    image_path.push("image.png");
    match img_to_save.save(image_path) {
        Ok(_) => {}
        Err(err) => {
            println!("error in saving image: {err:?}");
        }
    }
    //TODO: implement large image saving
}

fn update_intensity_image(scan: &ScannedImage, img_lock: &Arc<RwLock<Array2<f32>>>) {
    if let Ok(mut write_guard) = img_lock.write() {
        *write_guard = scan.filtered_img.clone();
    }
}

fn filter_time_window(
    config: &ConfigContainer,
    scan: &mut ScannedImage,
    img_lock: &Arc<RwLock<Array2<f32>>>,
) {
    // calculate fft filter and calculate ifft
    println!("updating data");
    let start = Instant::now();
    let lower = scan
        .time
        .iter()
        .position(|t| *t == config.time_window[0].round())
        .unwrap_or(0);
    let upper = scan
        .time
        .iter()
        .position(|t| *t == config.time_window[1].round())
        .unwrap_or(0);
    println!("lower: {}, upper: {}", lower, upper);

    (
        scan.scaled_data.axis_iter_mut(Axis(0)),
        scan.filtered_data.axis_iter_mut(Axis(0)),
        scan.filtered_img.axis_iter_mut(Axis(0)),
    )
        .into_par_iter()
        .for_each(
            |(mut scaled_data_columns, mut filtered_data_columns, mut filtered_img_columns)| {
                (
                    scaled_data_columns.axis_iter_mut(Axis(0)),
                    filtered_data_columns.axis_iter_mut(Axis(0)),
                    filtered_img_columns.axis_iter_mut(Axis(0)),
                )
                    .into_par_iter()
                    .for_each(|(_scaled_data, filtered_data, filtered_img)| {
                        *filtered_img.into_scalar() = filtered_data
                            .iter()
                            .skip(lower)
                            .take(upper - lower)
                            .map(|xi| xi * xi)
                            .sum::<f32>();
                    });
            },
        );
    update_intensity_image(scan, img_lock);
    println!("updated data. This took {:?}", start.elapsed());
}

fn filter(config: &ConfigContainer, scan: &mut ScannedImage, img_lock: &Arc<RwLock<Array2<f32>>>) {
    // calculate fft filter and calculate ifft
    println!("updating data");
    let start = Instant::now();
    let lower = scan
        .time
        .iter()
        .position(|t| *t == config.time_window[0].round())
        .unwrap_or(0);
    let upper = scan
        .time
        .iter()
        .position(|t| *t == config.time_window[1].round())
        .unwrap_or(scan.time.len());
    if let Some(r2c) = &scan.r2c {
        scan.filtered_img = Array2::zeros((scan.scaled_data.shape()[0], scan.scaled_data.shape()[1]));
        if let Some(c2r) = &scan.c2r {
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
                        (
                            scaled_data_columns.axis_iter_mut(Axis(0)),
                            filtered_data_columns.axis_iter_mut(Axis(0)),
                            filtered_img_columns.axis_iter_mut(Axis(0)),
                        )
                            .into_par_iter()
                            .for_each(
                                |(scaled_data, mut filtered_data, filtered_img)| {
                                    filtered_data.assign(&scaled_data);
                                    apply_fft_window(
                                        &mut filtered_data,
                                        &scan.time,
                                        &config.fft_window[0],
                                        &config.fft_window[1],
                                    );

                                    // calculate fft
                                    let mut spectrum = r2c.make_output_vec();
                                    // Forward transform the input data
                                    r2c.process(&mut filtered_data.to_vec(), &mut spectrum)
                                        .unwrap();

                                    // apply bandpass filter
                                    for (f, spectrum) in
                                        scan.frequencies.iter().zip(spectrum.iter_mut())
                                    {
                                        if (*f < config.fft_filter[0])
                                            || (*f > config.fft_filter[1])
                                        {
                                            *spectrum = Complex32::new(0.0, 0.0);
                                        }
                                    }

                                    let mut output = c2r.make_output_vec();

                                    // Forward transform the input data
                                    match c2r.process(&mut spectrum, &mut output) {
                                        Ok(_) => {}
                                        Err(err) => {
                                            println!("error in iFFT: {err:?}");
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
                                },
                            );
                    },
                );
        };
    };
    // update images
    update_intensity_image(scan, img_lock);
    println!("updated data. This took {:?}", start.elapsed());
}

#[derive(Clone, PartialEq)]
enum FileType {
    Npy,
    Npz,
}

pub fn main_thread(
    md_lock: Arc<RwLock<DotthzMetaData>>,
    data_lock: Arc<RwLock<DataPoint>>,
    img_lock: Arc<RwLock<Array2<f32>>>,
    config_rx: Receiver<Config>,
    scaling_lock: Arc<RwLock<u8>>,
    pixel_lock: Arc<RwLock<SelectedPixel>>,
) {
    // reads data from mutex, samples and saves if needed
    let mut data = DataPoint::default();
    let mut scan = ScannedImage::default();
    let mut config = ConfigContainer::default();
    let mut selected_pixel = SelectedPixel::default();
    let mut meta_data = DotthzMetaData::default();
    let mut hk_csv = None;
    loop {
        if let Ok(config_command) = config_rx.recv() {
            match config_command {
                Config::OpenFile(selected_file_path) => {
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
                                        println!("failed to open json @ {json_path:?}... {err}");
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
                                        update_intensity_image(&mut scan, &img_lock);
                                    }
                                    Err(err) => {
                                        println!("an error occurred while trying to read data.npz {err:?}");
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
                                println!("file ending not supported");
                            }
                            "thz" => {
                                match open_from_thz(&selected_file_path, &mut scan, &mut meta_data)
                                {
                                    Ok(_) => {
                                        log::info!("opened {:?}", selected_file_path);
                                        update_intensity_image(&mut scan, &img_lock);
                                    }
                                    Err(err) => {
                                        log::error!(
                                            "failed opening {:?}: {:?}",
                                            selected_file_path,
                                            err
                                        )
                                    }
                                };
                                if let Ok(mut md) = md_lock.write() {
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
                Config::SetFFTWindowLow(low) => {
                    config.fft_window[0] = low;
                    filter(&config, &mut scan, &img_lock);
                }
                Config::SetFFTWindowHigh(high) => {
                    config.fft_window[1] = high;
                    filter(&config, &mut scan, &img_lock);
                }
                Config::SetFFTFilterLow(low) => {
                    config.fft_filter[0] = low;
                    filter(&config, &mut scan, &img_lock);
                }
                Config::SetFFTFilterHigh(high) => {
                    config.fft_filter[1] = high;
                    filter(&config, &mut scan, &img_lock);
                }
                Config::SetTimeWindow(time_window) => {
                    config.time_window = time_window;
                    filter_time_window(&config, &mut scan, &img_lock);
                }
                Config::SetFFTLogPlot(log_plot) => {
                    config.fft_log_plot = log_plot;
                    filter(&config, &mut scan, &img_lock);
                }
                Config::SetFFTNormalization(normalization) => {
                    config.normalize_fft = normalization;
                    filter(&config, &mut scan, &img_lock);
                }
                Config::SetFFTResolution(df) => {
                    config.fft_df = df;
                    filter(&config, &mut scan, &img_lock);
                }
                Config::SetDownScaling => {
                    if let Ok(scaling) = scaling_lock.read() {
                        scan.scaling = *scaling as usize;
                        scan.rescale()
                    }
                    filter(&config, &mut scan, &img_lock);
                    update_intensity_image(&scan, &img_lock);
                }
                Config::SetSelectedPixel(pixel) => {
                    selected_pixel = pixel.clone();
                    if let Ok(scaling) = scaling_lock.read() {
                        scan.scaling = *scaling as usize;
                        scan.rescale()
                    }
                    println!("new pixel: {:} {:}", selected_pixel.x, selected_pixel.y);
                    // send HK?
                }
            }

            if let Ok(pixel) = pixel_lock.read() {
                selected_pixel = pixel.clone();
            }

            if let Some(r2c) = &scan.r2c {
                if let Ok(mut data) = data_lock.write() {
                    data.time = scan.time.to_vec();
                    data.frequencies = scan.frequencies.to_vec();
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
                    // let d = data.clone();
                }
            }
        }
    }
}
