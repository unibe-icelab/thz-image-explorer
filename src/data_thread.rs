use std::f32::consts::PI;
use std::fs;
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, RwLock};
use std::time::Instant;

use eframe::egui::ColorImage;
use image::RgbaImage;
use ndarray::parallel::prelude::*;
use ndarray::{Array1, Array2, Axis};
use realfft::num_complex::Complex32;

use crate::config::{Config, ConfigContainer};
use crate::data::{DataPoint, ScannedImage};
use crate::io::{open_from_npz, open_json};
use crate::math_tools::{apply_fft_window, numpy_unwrap};
use crate::matrix_plot::SelectedPixel;
use crate::Print;

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

fn update_intensity_image(scan: &mut ScannedImage, img_lock: &Arc<RwLock<Array2<f32>>>) {
    if let Ok(mut write_guard) = img_lock.write() {
        *write_guard = scan.filtered_img.clone();
    }
}

fn update_waterfall_image(
    scan: &mut ScannedImage,
    pixel_x: usize,
    waterfall_lock: &Arc<RwLock<Array2<f32>>>,
    scaling_lock: &Arc<RwLock<u8>>,
) {
    if let Some(r2c) = &scan.r2c {
        if let Ok(mut write_guard) = waterfall_lock.write() {
            let mut spectrum = r2c.make_output_vec();
            let mut scaling = 1;
            if let Ok(s) = scaling_lock.read() {
                scaling = s.clone();
            }
            let pixel_x = pixel_x / scaling as usize;
            let mut in_data = scan
                .filtered_data
                .index_axis(Axis(0), pixel_x)
                .index_axis(Axis(0), 0)
                .to_vec();
            r2c.process(&mut in_data, &mut spectrum).unwrap();

            let m = scan.filtered_data.shape()[0];
            let n = spectrum.len();
            let mut img = Array2::zeros((n, m));
            let data = scan.filtered_data.index_axis(Axis(0), pixel_x);
            (data.axis_iter(Axis(0)), img.axis_iter_mut(Axis(1)))
                .into_par_iter()
                .for_each(|(line, mut img)| {
                    let mut spectrum = r2c.make_output_vec();
                    let mut in_data = line.to_vec();
                    r2c.process(&mut in_data, &mut spectrum).unwrap();
                    let amp: Vec<f32> = spectrum.iter().map(|s| s.norm()).collect();
                    img.assign(&Array1::from_vec(amp));
                });
            *write_guard = img;
        }
    }
}

fn filter_time_window(
    config: &ConfigContainer,
    scan: &mut ScannedImage,
    img_lock: &Arc<RwLock<Array2<f32>>>,
    waterfall_lock: &Arc<RwLock<Array2<f32>>>,
    scaling_lock: &Arc<RwLock<u8>>,
) {
    // calculate fft filter and calculate ifft
    println!("updating data");
    let start = Instant::now();
    let mut lower = scan
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
                    .for_each(
                        |(mut scaled_data, mut filtered_data, mut filtered_img)| {
                            *filtered_img.into_scalar() = filtered_data
                                .iter()
                                .skip(lower)
                                .take(upper - lower)
                                .map(|xi| xi * xi)
                                .sum::<f32>();
                        },
                    );
            },
        );
    update_intensity_image(scan, img_lock);
    update_waterfall_image(scan, config.selected_pixel[0], waterfall_lock, scaling_lock);
    println!("updated data. This took {:?}", start.elapsed());
}

fn filter(
    config: &ConfigContainer,
    scan: &mut ScannedImage,
    img_lock: &Arc<RwLock<Array2<f32>>>,
    waterfall_lock: &Arc<RwLock<Array2<f32>>>,
    scaling_lock: &Arc<RwLock<u8>>,
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
    if let Some(r2c) = &scan.r2c {
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
                                |(mut scaled_data, mut filtered_data, mut filtered_img)| {
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

    // for x in 0..scan.width {
    //     for y in 0..scan.height {
    //         // calculate the intensity by summing the squares
    //         let sig_squared_sum = scan
    //             .raw_data
    //             .index_axis(Axis(0), x)
    //             .index_axis(Axis(0), y)
    //             .mapv(|xi| xi * xi)
    //             .sum();
    //         scan.raw_img[[x, y]] = sig_squared_sum;
    //     }
    // }
    // update images
    update_intensity_image(scan, img_lock);
    update_waterfall_image(scan, config.selected_pixel[0], waterfall_lock, scaling_lock);
    println!("updated data. This took {:?}", start.elapsed());
}

fn load_from_npz(
    opened_directory_path: &PathBuf,
    data: &mut DataPoint,
    scan: &mut ScannedImage,
    config: &mut ConfigContainer,
    img_lock: &Arc<RwLock<Array2<f32>>>,
    waterfall_lock: &Arc<RwLock<Array2<f32>>>,
    scaling_lock: &Arc<RwLock<u8>>,
) {
    let width: usize;
    let height: usize;

    let mut json_path = opened_directory_path.clone();
    json_path.push("meta.json");

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

    *scan = ScannedImage::new(
        height,
        width,
        data.hk.x_range[0],
        data.hk.y_range[0],
        data.hk.dx,
        data.hk.dy,
    );

    let mut pulse_path = opened_directory_path.clone();
    pulse_path.push("data.npz");
    dbg!(&pulse_path);
    match open_from_npz(scan, &pulse_path) {
        Ok(_) => {
            update_intensity_image(scan, img_lock);
            update_waterfall_image(scan, config.selected_pixel[0], waterfall_lock, scaling_lock);
        }
        Err(err) => {
            println!("an error occurred while trying to read data.npz {err:?}");
        }
    }
    println!("opened npz");
}

#[derive(Clone, PartialEq)]
enum FileType {
    Npy,
    Npz,
}

pub fn main_thread(
    data_lock: Arc<RwLock<DataPoint>>,
    img_lock: Arc<RwLock<Array2<f32>>>,
    waterfall_lock: Arc<RwLock<Array2<f32>>>,
    print_lock: Arc<RwLock<Vec<Print>>>,
    config_rx: Receiver<Config>,
    load_rx: Receiver<PathBuf>,
    scaling_lock: Arc<RwLock<u8>>,
) {
    // reads data from mutex, samples and saves if needed
    let mut data = DataPoint::default();
    let mut scan = ScannedImage::default();
    let mut config = ConfigContainer::default();
    let mut pixel = SelectedPixel::default();

    loop {
        if let Ok(config_command) = config_rx.recv() {
            match config_command {
                Config::OpenFile(opened_folder_path) => {
                    let files = fs::read_dir(&opened_folder_path)
                        .unwrap()
                        .filter_map(Result::ok)
                        .filter_map(|entry| {
                            if let Some(extension) = entry.path().extension() {
                                match extension.to_str().unwrap() {
                                    "npy" => Some(FileType::Npy),
                                    "npz" => Some(FileType::Npz),
                                    _ => None,
                                }
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<FileType>>();
                    if files.contains(&FileType::Npy) {
                        println!("[OK] found npy binaries.");
                        println!("[ERR] npy no implemented...");
                        // load_from_npy(
                        //     &opened_folder_path,
                        //     &mut real_planner,
                        //     &mut data,
                        //     &mut scan,
                        //     &mut config,
                        //     &img_lock,
                        //     &waterfall_lock,
                        // );
                    } else if files.contains(&FileType::Npz) {
                        println!("[OK] found a npz binary.");
                        load_from_npz(
                            &opened_folder_path,
                            &mut data,
                            &mut scan,
                            &mut config,
                            &img_lock,
                            &waterfall_lock,
                            &scaling_lock,
                        );

                        if let Some(r2c) = &scan.r2c {
                            if let Ok(mut data) = data_lock.write() {
                                data.time = scan.time.to_vec();
                                data.frequencies = scan.frequencies.to_vec();
                                data.signal_1 = scan
                                    .raw_data
                                    .index_axis(Axis(0), 0)
                                    .index_axis(Axis(0), 0)
                                    .to_vec();
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
                    } else {
                        println!("[ERR] no binaries found!")
                    }
                    // read HK
                }
                Config::SetFFTWindowLow(low) => {
                    config.fft_window[0] = low;
                    filter(
                        &config,
                        &mut scan,
                        &img_lock,
                        &waterfall_lock,
                        &scaling_lock,
                    );
                }
                Config::SetFFTWindowHigh(high) => {
                    config.fft_window[1] = high;
                    filter(
                        &config,
                        &mut scan,
                        &img_lock,
                        &waterfall_lock,
                        &scaling_lock,
                    );
                }
                Config::SetFFTFilterLow(low) => {
                    config.fft_filter[0] = low;
                    filter(
                        &config,
                        &mut scan,
                        &img_lock,
                        &waterfall_lock,
                        &scaling_lock,
                    );
                }
                Config::SetFFTFilterHigh(high) => {
                    config.fft_filter[1] = high;
                    filter(
                        &config,
                        &mut scan,
                        &img_lock,
                        &waterfall_lock,
                        &scaling_lock,
                    );
                }
                Config::SetTimeWindow(time_window) => {
                    config.time_window = time_window;
                    filter_time_window(
                        &config,
                        &mut scan,
                        &img_lock,
                        &waterfall_lock,
                        &scaling_lock,
                    );
                }
                Config::SetFFTLogPlot(log_plot) => {
                    config.fft_log_plot = log_plot;
                    filter(
                        &config,
                        &mut scan,
                        &img_lock,
                        &waterfall_lock,
                        &scaling_lock,
                    );
                }
                Config::SetFFTNormalization(normalization) => {
                    config.normalize_fft = normalization;
                    filter(
                        &config,
                        &mut scan,
                        &img_lock,
                        &waterfall_lock,
                        &scaling_lock,
                    );
                }
                Config::SetFFTResolution(df) => {
                    config.fft_df = df;
                    filter(
                        &config,
                        &mut scan,
                        &img_lock,
                        &waterfall_lock,
                        &scaling_lock,
                    );
                }
                Config::SetDownScaling(scaling) => {
                    // TODO: rescale selected pixel
                    config.down_scaling = scaling;
                    scan.rescale(config.down_scaling);
                    scan.filtered_data = scan.scaled_data.clone();
                    scan.filtered_img = scan.scaled_img.clone();
                    filter(
                        &config,
                        &mut scan,
                        &img_lock,
                        &waterfall_lock,
                        &scaling_lock,
                    );
                }
                Config::SetSelectedPixel(pixel_location) => {
                    config.selected_pixel = pixel_location;
                    update_waterfall_image(
                        &mut scan,
                        config.selected_pixel[0],
                        &waterfall_lock,
                        &scaling_lock,
                    );
                    println!("new pixel: {:?}", config.selected_pixel);
                    // send HK?
                }
            }
            let mut scaling = 1;
            if let Ok(s) = scaling_lock.read() {
                scaling = s.clone();
            }
            if let Some(r2c) = &scan.r2c {
                if let Ok(mut data) = data_lock.write() {
                    data.time = scan.time.to_vec();
                    data.frequencies = scan.frequencies.to_vec();
                    data.signal_1 = scan
                        .filtered_data
                        .index_axis(Axis(0), config.selected_pixel[0] / scaling as usize)
                        .index_axis(Axis(0), config.selected_pixel[1] / scaling as usize)
                        .to_vec();
                    let mut in_data: Vec<f32> = data.signal_1.to_vec();
                    let mut spectrum = r2c.make_output_vec();
                    // Forward transform the input data
                    r2c.process(&mut in_data, &mut spectrum).unwrap();
                    let amp: Vec<f32> = spectrum.iter().map(|s| s.norm()).collect();
                    let phase: Vec<f32> = spectrum.iter().map(|s| s.arg()).collect();
                    data.signal_1_fft = amp;
                    data.phase_1_fft = numpy_unwrap(&phase, Some(2.0 * PI));
                    let d = data.clone();
                }
            }
        }
    }
}
