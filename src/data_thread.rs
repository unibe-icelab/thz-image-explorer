use std::error::Error;
use std::fs;
use std::fs::File;
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use eframe::egui::ColorImage;
use image::RgbaImage;
use ndarray::Array2;
use ndarray_npy::NpzReader;
use rayon::prelude::*;

use crate::config::{Config, ConfigContainer};
use crate::data::{DataPoint, ScannedImage};
use crate::io::{open_conf, open_from_npy, open_from_npz, open_hk, open_json};
use crate::math_tools::{apply_filter, make_ifft};
use crate::{make_fft, print_to_console, save_to_csv, update_in_console, Print, SelectedPixel};

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
        let img = Array2::from_shape_fn((scan.width, scan.height), |(x, y)| scan.get_img(x, y));
        *write_guard = img;
    }
}

fn update_waterfall_image(
    scan: &mut ScannedImage,
    pixel_x: usize,
    waterfall_lock: &Arc<RwLock<Array2<f32>>>,
) {
    if let Ok(mut write_guard) = waterfall_lock.write() {
        let len = scan.data[0].signal_1_fft.len();
        let img = Array2::from_shape_fn((len, scan.height), |(x, y)| {
            scan.get_data(pixel_x, y).filtered_signal_1_fft.clone()[x]
        });
        *write_guard = img;
    }
}

fn update(
    config: &ConfigContainer,
    scan: &mut ScannedImage,
    img_lock: &Arc<RwLock<Array2<f32>>>,
    waterfall_lock: &Arc<RwLock<Array2<f32>>>,
) {
    // calculate fft filter and calculate ifft
    scan.data
        .iter_mut()
        .zip(scan.img.iter_mut())
        .for_each(|(pixel_data, img_data)| {
            // calculate fft
            (
                pixel_data.frequencies_fft,
                pixel_data.signal_1_fft,
                pixel_data.phase_1_fft,
            ) = make_fft(
                &pixel_data.time,
                &pixel_data.signal_1,
                config.normalize_fft,
                &config.fft_df,
                &config.fft_window[0],
                &config.fft_window[1],
            );
            (_, pixel_data.ref_1_fft, pixel_data.ref_phase_1_fft) = make_fft(
                &pixel_data.time,
                &pixel_data.ref_1,
                config.normalize_fft,
                &config.fft_df,
                &config.fft_window[0],
                &config.fft_window[1],
            );
            apply_filter(pixel_data, &config.fft_filter);
            pixel_data.filtered_signal_1 = make_ifft(
                &pixel_data.frequencies_fft,
                &pixel_data.filtered_signal_1_fft,
                &pixel_data.filtered_phase_1_fft,
            );
            // calculate the intensity by summing the squares
            let sig_squared: Vec<f32> = pixel_data
                .filtered_signal_1
                .iter()
                .map(|x| x.powi(2))
                .collect();
            *img_data = sig_squared.iter().sum();
        });
    // update images
    update_intensity_image(scan, img_lock);
    update_waterfall_image(scan, config.selected_pixel[0], waterfall_lock);
}

fn load_from_npy(
    opened_directory_path: &PathBuf,
    data: &mut DataPoint,
    scan: &mut ScannedImage,
    config: &mut ConfigContainer,
    img_lock: &Arc<RwLock<Array2<f32>>>,
    waterfall_lock: &Arc<RwLock<Array2<f32>>>,
) {
    let width: usize;
    let height: usize;

    let mut conf_path = opened_directory_path.clone();
    conf_path.push("conf.csv");

    match open_conf(&mut data.hk, &conf_path) {
        Ok((w, h)) => {
            width = w;
            height = h;
        }
        Err(err) => {
            println!("failed to open conf @ {conf_path:?}... {err}");
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

    for x in 0..width {
        for y in 0..height {
            let mut pulse_path = opened_directory_path.clone();
            pulse_path.push(format!("pixel_ID={:05}-{:05}.npy", x, y));
            let mut fft_path = opened_directory_path.clone();
            fft_path.push(format!("pixel_ID={:05}-{:05}_spectrum.npy", x, y));

            if let Err(_) = open_from_npy(data, &pulse_path, &fft_path) {
                println!("failed to open files: {pulse_path:?} {fft_path:?}");
                // just take a previous value... maybe replace this at some point?
                let x1;
                let y1;
                if x == 0 {
                    x1 = 0;
                } else {
                    x1 = x - 1;
                }
                if y == 0 {
                    y1 = 0;
                } else {
                    y1 = y - 1;
                }
                *data = scan.get_data(x1, y1);
            }

            // subtract reference from signal
            // TODO: we need to remove the bias/offset!
            data.signal_1 = data
                .signal_1
                .iter()
                .zip(data.ref_1.iter())
                .map(|(s, r)| *s - *r)
                .collect();

            (data.frequencies_fft, data.signal_1_fft, data.phase_1_fft) = make_fft(
                &data.time,
                &data.signal_1,
                config.normalize_fft,
                &config.fft_df,
                &config.fft_window[0],
                &config.fft_window[1],
            );
            (_, data.ref_1_fft, data.ref_phase_1_fft) = make_fft(
                &data.time,
                &data.ref_1,
                config.normalize_fft,
                &config.fft_df,
                &config.fft_window[0],
                &config.fft_window[1],
            );

            data.filtered_phase_1_fft = data.phase_1_fft.clone();
            data.filtered_signal_1_fft = data.signal_1_fft.clone();

            scan.set_data(x, y, data.clone());

            // calculate the intensity by summing the squares
            let sig_squared: Vec<f32> = data.signal_1.iter().map(|x| x.powi(2)).collect();
            scan.set_img(x, y, sig_squared.iter().sum());

            update_intensity_image(scan, img_lock);
            update_waterfall_image(scan, config.selected_pixel[0], waterfall_lock);
        }
    }
}

fn load_from_npz(
    opened_directory_path: &PathBuf,
    data: &mut DataPoint,
    scan: &mut ScannedImage,
    config: &mut ConfigContainer,
    img_lock: &Arc<RwLock<Array2<f32>>>,
    waterfall_lock: &Arc<RwLock<Array2<f32>>>,
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
            update_waterfall_image(scan, config.selected_pixel[0], waterfall_lock);
        }
        Err(err) => {
            println!("an error occurred while trying to read data.npz {err:?}");
        }
    }
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
) {
    // reads data from mutex, samples and saves if needed
    let mut data = DataPoint::default();
    let mut scan = ScannedImage::default();
    let mut config = ConfigContainer::default();
    let mut pixel = SelectedPixel::default();

    loop {
        if let Ok(opened_folder_path) = load_rx.recv_timeout(Duration::from_millis(10)) {
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
                load_from_npy(
                    &opened_folder_path,
                    &mut data,
                    &mut scan,
                    &mut config,
                    &img_lock,
                    &waterfall_lock,
                );
            } else if files.contains(&FileType::Npz) {
                println!("[OK] found a npz binary.");
                load_from_npz(
                    &opened_folder_path,
                    &mut data,
                    &mut scan,
                    &mut config,
                    &img_lock,
                    &waterfall_lock,
                )
            } else {
                println!("no binaries found.. CSV has to be implemented!")
            }
        }
        if let Ok(config_command) = config_rx.recv_timeout(Duration::from_millis(10)) {
            match config_command {
                Config::SetFFTWindowLow(low) => {
                    config.fft_window[0] = low;
                }
                Config::SetFFTWindowHigh(high) => {
                    config.fft_window[1] = high;
                }
                Config::SetFFTFilterLow(low) => {
                    config.fft_filter[0] = low;
                }
                Config::SetFFTFilterHigh(high) => {
                    config.fft_filter[1] = high;
                }
                Config::SetTimeWindowLow(low) => {
                    config.time_window[0] = low;
                }
                Config::SetTimeWindowHigh(high) => {
                    config.time_window[1] = high;
                }
                Config::SetFFTLogPlot(log_plot) => {
                    config.fft_log_plot = log_plot;
                }
                Config::SetFFTNormalization(normalization) => {
                    config.normalize_fft = normalization;
                }
                Config::SetFFTResolution(df) => {
                    config.fft_df = df;
                }
                Config::SetSelectedPixel(pixel_location) => {
                    config.selected_pixel = pixel_location;
                    if let Ok(mut write_guard) = data_lock.write() {
                        *write_guard = scan.get_data(pixel_location[0], pixel_location[1]);
                    }
                }
            }
            update(&config, &mut scan, &img_lock, &waterfall_lock);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}
