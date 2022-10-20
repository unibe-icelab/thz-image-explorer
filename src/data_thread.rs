use std::f64::NEG_INFINITY;
use std::sync::{Arc, RwLock};
use std::sync::mpsc::Receiver;
use std::time::Duration;
use eframe::egui::{Color32, ColorImage};
use image::RgbaImage;
use ndarray::Array2;
use crate::{make_fft, Print, print_to_console, save_to_csv, SelectedPixel, update_in_console};
use crate::data::{DataContainer};
use crate::io::{open_from_csv, open_conf, open_hk};
use crate::math_tools::{apply_filter, make_ifft};
use crate::matrix_plot::color_from_intensity;
use rayon::prelude::*;

#[derive(Clone)]
pub struct ScannedImage {
    pub path: String,
    pub x_min: f64,
    pub dx: f64,
    pub y_min: f64,
    pub dy: f64,
    pub height: usize,
    pub width: usize,
    pub img: Array2<f64>,
    pub color_img: ColorImage,
    pub data: Vec<Vec<DataContainer>>,
    // do we need the ids?
}

impl Default for ScannedImage {
    fn default() -> Self {
        return ScannedImage {
            path: "".to_string(),
            x_min: 0.0,
            dx: 0.0,
            y_min: 0.0,
            dy: 0.0,
            height: 0,
            width: 0,
            img: Array2::from_shape_fn((1, 1), |(_, _)| {
                0.0
            }),
            color_img: ColorImage::new([1, 1], Color32::TRANSPARENT),
            data: vec![vec![]],
        };
    }
}

impl ScannedImage {
    pub fn new(height: usize, width: usize, x_min: f64, y_min: f64, dx: f64, dy: f64) -> ScannedImage {
        return ScannedImage {
            path: "test".to_string(),
            x_min,
            dx,
            y_min,
            dy,
            height,
            width,
            img: Array2::from_shape_fn((width, height), |(_, _)| {
                0.0
            }),
            color_img: ColorImage::new([width, height], Color32::TRANSPARENT),
            data: vec![vec![DataContainer::default(); width]; height],
        };
    }
}

fn save_image(img: &ColorImage, file_path: &String) {
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
    match img_to_save.save(format!("{}/image.png", file_path)) {
        Ok(_) => {}
        Err(err) => {
            println!("error in saving image: {err:?}");
        }
    }
    //TODO: implement large image saving
}

pub fn main_thread(data_lock: Arc<RwLock<DataContainer>>,
                   df_lock: Arc<RwLock<f64>>,
                   log_mode_lock: Arc<RwLock<bool>>,
                   normalize_fft_lock: Arc<RwLock<bool>>,
                   fft_bounds_lock: Arc<RwLock<[f64; 2]>>,
                   fft_filter_bounds_lock: Arc<RwLock<[f64; 2]>>,
                   img_lock: Arc<RwLock<Array2<f64>>>,
                   pixel_lock: Arc<RwLock<SelectedPixel>>,
                   print_lock: Arc<RwLock<Vec<Print>>>,
                   save_rx: Receiver<String>,
                   load_rx: Receiver<String>) {
    // reads data from mutex, samples and saves if needed
    let mut acquire = false;
    let mut file_path = "test.csv".to_string();
    let mut opened_file_path = "test.csv".to_string();
    let mut data = DataContainer::default();
    let mut df = 0.001;
    let mut filter_bounds = [1.0, 10.0];
    let mut lower_bound = 1.0;
    let mut upper_bound = 7.0;
    let mut normalize_fft = false;
    let mut log_mode = true;

    loop {
        if let Ok(read_guard) = df_lock.read() {
            df = *read_guard;
        }

        if let Ok(read_guard) = log_mode_lock.read() {
            log_mode = *read_guard;
        }

        if let Ok(read_guard) = normalize_fft_lock.read() {
            normalize_fft = *read_guard;
        }

        if let Ok(read_guard) = fft_bounds_lock.read() {
            lower_bound = (*read_guard)[0];
            upper_bound = (*read_guard)[1];
        }

        // TODO: add filter bounds

        match save_rx.recv_timeout(Duration::from_millis(10)) {
            Ok(fp) => {
                file_path = fp;
                acquire = true;
            }
            Err(..) => ()
        }

        match load_rx.recv() {
            Ok(fp) => {
                opened_file_path = fp;
            }
            Err(..) => ()
        }

        let width: usize;
        let height: usize;
        match open_conf(&mut data.hk, format!("{opened_file_path}/conf.csv")) {
            Ok((w, h)) => {
                width = w;
                height = h;
            }
            Err(err) => {
                println!("failed to open conf @ {opened_file_path}... {err}");
                width = 0;
                height = 0;
            }
        }

        let mut scan = ScannedImage::new(
            height,
            width,
            data.hk.x_range[0],
            data.hk.y_range[0],
            data.hk.dx,
            data.hk.dy,
        );
        for x in 0..width {
            for y in 0..height {
                let pulse_path = format!("{}/pixel_ID={:05}-{:05}.csv", opened_file_path, x, y);
                let fft_path = format!("{}/pixel_ID={:05}-{:05}_spectrum.csv", opened_file_path, x, y);
                match open_from_csv(&mut data, &pulse_path, &fft_path) {
                    Ok(_) => {}
                    Err(_) => {
                        println!("failed to open files: {pulse_path} {fft_path}");
                        data = scan.data[x - 1][y - 1].clone();
                    }
                }
                data.signal_1 = data.signal_1.iter().zip(data.ref_1.iter()).map(|(s, r)| *s - *r).collect();

                let (frequencies_fft, signal_1_fft, phase_1_fft) = make_fft(&data.time, &data.signal_1, normalize_fft, &df, &lower_bound, &upper_bound);
                let (_, ref_1_fft, ref_phase_1_fft) = make_fft(&data.time, &data.ref_1, normalize_fft, &df, &lower_bound, &upper_bound);

                data.frequencies_fft = frequencies_fft;
                data.signal_1_fft = signal_1_fft;
                data.phase_1_fft = phase_1_fft;
                data.ref_1_fft = ref_1_fft;
                data.ref_phase_1_fft = ref_phase_1_fft;

                data.filtered_phase_1_fft = data.phase_1_fft.clone();
                data.filtered_signal_1_fft = data.signal_1_fft.clone();
                scan.data[x][y] = data.clone();
                // calculate the intensity by summing the squares
                let sig_squared: Vec<f64> = data.signal_1.par_iter().map(|x| x.powi(2)).collect();
                scan.img[[y, x]] = sig_squared.par_iter().sum();
                let max = scan.img.iter().fold(NEG_INFINITY, |ai, &bi| ai.max(bi));
                scan.color_img[(y, x)] = color_from_intensity(scan.img[[y, x]], max, data.cut_off);

                if let Ok(mut write_guard) = img_lock.write() {
                    *write_guard = scan.img.clone();
                }
            }
        }

        let mut pixel = SelectedPixel::new();

        loop {
            if let Ok(read_guard) = pixel_lock.read() {
                if pixel.x != read_guard.x || pixel.y != read_guard.y {
                    pixel = read_guard.clone();

                    data = scan.data[pixel.x as usize][pixel.y as usize].clone();

                    let (frequencies_fft, signal_1_fft, phase_1_fft) = make_fft(&data.time, &data.signal_1, normalize_fft, &df, &lower_bound, &upper_bound);
                    let (_, ref_1_fft, ref_phase_1_fft) = make_fft(&data.time, &data.ref_1, normalize_fft, &df, &lower_bound, &upper_bound);

                    data.frequencies_fft = frequencies_fft;
                    data.signal_1_fft = signal_1_fft;
                    data.phase_1_fft = phase_1_fft;
                    data.ref_1_fft = ref_1_fft;
                    data.ref_phase_1_fft = ref_phase_1_fft;

                    // open hk file of selected pixel
                    let hk_path = format!("{}/pixel_ID={:05}-{:05}_hk.csv", opened_file_path, pixel.y, pixel.x);
                    match open_hk(&mut data.hk, hk_path) {
                        Ok((x, y)) => {}
                        Err(err) => {
                            println!("failed to open hk: {err}");
                        }
                    }

                    if let Ok(mut write_guard) = data_lock.write() {
                        *write_guard = data.clone();
                    }
                }
            }

            let old_bounds = filter_bounds;
            if let Ok(read_guard) = fft_filter_bounds_lock.read() {
                filter_bounds = read_guard.clone();
            }
            if filter_bounds != old_bounds {
                //TODO: iterate over image pixels instead of x and y
                // >> implement iter for scan object
                for x in 0..width {
                    for y in 0..height {
                        apply_filter(&mut scan.data[x][y], &filter_bounds);
                        scan.data[x][y].filtered_signal_1 = make_ifft(&scan.data[x][y].frequencies_fft, &scan.data[x][y].filtered_signal_1_fft, &scan.data[x][y].filtered_phase_1_fft);
                        // calculate the intensity by summing the squares
                        let sig_squared: Vec<f64> = scan.data[x][y].filtered_signal_1.par_iter().map(|x| x.powi(2)).collect();
                        scan.img[[y, x]] = sig_squared.par_iter().sum();
                        let max = scan.img.iter().fold(NEG_INFINITY, |ai, &bi| ai.max(bi));
                        scan.color_img[(y, x)] = color_from_intensity(scan.img[[y, x]], max, scan.data[x][y].cut_off);

                        if let Ok(mut write_guard) = img_lock.write() {
                            *write_guard = scan.img.clone();
                        }
                    }
                }
                if let Ok(mut write_guard) = data_lock.write() {
                    *write_guard = scan.data[pixel.x as usize][pixel.y as usize].clone();
                }
            }

            // TODO: check for new file

            std::thread::sleep(Duration::from_millis(10));
        }

        let (frequencies_fft, signal_1_fft, phase_1_fft) = make_fft(&data.time, &data.signal_1, normalize_fft, &df, &lower_bound, &upper_bound);
        let (_, ref_1_fft, ref_phase_1_fft) = make_fft(&data.time, &data.ref_1, normalize_fft, &df, &lower_bound, &upper_bound);

        data.frequencies_fft = frequencies_fft;
        data.signal_1_fft = signal_1_fft;
        data.phase_1_fft = phase_1_fft;
        data.ref_1_fft = ref_1_fft;
        data.ref_phase_1_fft = ref_phase_1_fft;


        if acquire == true {
            // save file
            let extension = "_spectrum.csv".to_string();
            let mut file_path_fft: String;
            if file_path.ends_with(".csv") {
                file_path_fft = file_path[..file_path.len() - 4].to_string();
                file_path_fft.push_str(&extension);
            } else {
                file_path_fft = file_path.to_string();
                file_path_fft.push_str(&extension);
            }
            let print_index_1 = print_to_console(&print_lock, Print::TASK(format!("saving pulse file to {:?} ...", file_path).to_string()));
            let print_index_2 = print_to_console(&print_lock, Print::TASK(format!("saving fft ile to {:?} ...", file_path_fft).to_string()));
            let save_result = save_to_csv(&data, &file_path, &file_path_fft);
            match save_result {
                Ok(_) => {
                    update_in_console(&print_lock, Print::OK(format!("saved pulse file to {:?} ", file_path).to_string()), print_index_1);
                    update_in_console(&print_lock, Print::OK(format!("saved fft file to {:?} ", file_path_fft).to_string()), print_index_2);
                }
                Err(e) => {
                    update_in_console(&print_lock, Print::ERROR(format!("failed to save file to {:?} and {:?}", file_path, file_path_fft).to_string()), print_index_1);
                    update_in_console(&print_lock, Print::ERROR(e.to_string()), print_index_2);
                }
            }
            acquire = false;
        }

        if let Ok(mut write_guard) = data_lock.write() {
            // if normal mode, otherwise write scanned data here
            *write_guard = data.clone();
        }

        std::thread::sleep(Duration::from_millis(10));
    }
}