use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use eframe::egui::ColorImage;
use image::RgbaImage;
use ndarray::Array2;
use rayon::prelude::*;

use crate::data::DataContainer;
use crate::io::{open_conf, open_from_csv, open_from_npy, open_hk};
use crate::math_tools::{apply_filter, make_ifft};
use crate::matrix_plot::color_from_intensity;
use crate::{make_fft, print_to_console, save_to_csv, update_in_console, Print, SelectedPixel};

#[derive(Clone)]
pub struct ScannedImage {
    pub path: String,
    pub x_min: f64,
    pub dx: f64,
    pub y_min: f64,
    pub dy: f64,
    pub height: usize,
    pub width: usize,
    pub img: Vec<f64>,
    pub data: Vec<DataContainer>,
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
            img: vec![],
            data: vec![],
        };
    }
}

impl ScannedImage {
    pub fn new(
        height: usize,
        width: usize,
        x_min: f64,
        y_min: f64,
        dx: f64,
        dy: f64,
    ) -> ScannedImage {
        return ScannedImage {
            path: "test".to_string(),
            x_min,
            dx,
            y_min,
            dy,
            height,
            width,
            img: vec![0.0; width * height],
            data: vec![DataContainer::default(); width * height],
        };
    }
    pub fn get_data(&mut self, x: usize, y: usize) -> DataContainer {
        self.data[x * self.width + y].clone()
    }

    pub fn set_data(&mut self, x: usize, y: usize, data: DataContainer) {
        self.data[x * self.width + y] = data;
    }

    pub fn get_img(&mut self, x: usize, y: usize) -> f64 {
        self.img[x * self.width + y].clone()
    }

    pub fn set_img(&mut self, x: usize, y: usize, val: f64) {
        self.img[x * self.width + y] = val;
    }
}

impl ScannedImage {
    fn iter(
        &self,
    ) -> ScannedImageIter<impl Iterator<Item = &DataContainer>, impl Iterator<Item = &f64>> {
        ScannedImageIter {
            data: self.data.iter(),
            img: self.img.iter(),
        }
    }
    fn iter_mut(
        &mut self,
    ) -> ScannedImageIterMut<impl Iterator<Item = &mut DataContainer>, impl Iterator<Item = &mut f64>>
    {
        ScannedImageIterMut {
            data: self.data.iter_mut(),
            img: self.img.iter_mut(),
        }
    }
}

struct ScannedImageIter<D, I> {
    data: D,
    img: I,
}

impl<'r, D, I> Iterator for ScannedImageIter<D, I>
where
    D: Iterator<Item = &'r DataContainer>,
    I: Iterator<Item = &'r f64>,
{
    type Item = (&'r DataContainer, &'r f64);

    fn next(&mut self) -> Option<Self::Item> {
        match self.data.next() {
            Some(d) => match self.img.next() {
                Some(i) => Some((d, i)),
                None => None,
            },
            None => None,
        }
    }
}

struct ScannedImageIterMut<D, I> {
    data: D,
    img: I,
}

impl<'r, D, I> Iterator for ScannedImageIterMut<D, I>
where
    D: Iterator<Item = &'r mut DataContainer>,
    I: Iterator<Item = &'r mut f64>,
{
    type Item = (&'r mut DataContainer, &'r mut f64);

    fn next(&mut self) -> Option<Self::Item> {
        match self.data.next() {
            Some(d) => match self.img.next() {
                Some(i) => Some((d, i)),
                None => None,
            },
            None => None,
        }
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

pub fn main_thread(
    data_lock: Arc<RwLock<DataContainer>>,
    df_lock: Arc<RwLock<f64>>,
    log_mode_lock: Arc<RwLock<bool>>,
    normalize_fft_lock: Arc<RwLock<bool>>,
    fft_bounds_lock: Arc<RwLock<[f64; 2]>>,
    fft_filter_bounds_lock: Arc<RwLock<[f64; 2]>>,
    img_lock: Arc<RwLock<Array2<f64>>>,
    waterfall_lock: Arc<RwLock<Array2<f64>>>,
    pixel_lock: Arc<RwLock<SelectedPixel>>,
    print_lock: Arc<RwLock<Vec<Print>>>,
    save_rx: Receiver<PathBuf>,
    load_rx: Receiver<PathBuf>,
) {
    // reads data from mutex, samples and saves if needed
    let mut acquire = false;
    let mut file_path = PathBuf::new();
    let mut opened_file_path = PathBuf::new();
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
            Err(..) => (),
        }

        match load_rx.recv() {
            Ok(fp) => {
                opened_file_path = fp;
            }
            Err(..) => (),
        }

        let width: usize;
        let height: usize;
        let mut file_path = opened_file_path.clone();
        file_path.push("conf.csv");

        match open_conf(&mut data.hk, &file_path) {
            Ok((w, h)) => {
                width = w;
                height = h;
            }
            Err(err) => {
                println!("failed to open conf @ {file_path:?}... {err}");
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
                let mut pulse_path = opened_file_path.clone();
                pulse_path.push(format!("pixel_ID={:05}-{:05}.npy", x, y));
                let mut fft_path = opened_file_path.clone();
                fft_path.push(format!("pixel_ID={:05}-{:05}_spectrum.npy", x, y));
                //TODO: PathBuf!
                match open_from_npy(&mut data, &pulse_path, &fft_path) {
                    Ok(_) => {}
                    Err(_) => {
                        println!("failed to open files: {pulse_path:?} {fft_path:?}");
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
                        data = scan.get_data(x1, y1);
                    }
                }
                data.signal_1 = data
                    .signal_1
                    .iter()
                    .zip(data.ref_1.iter())
                    .map(|(s, r)| *s - *r)
                    .collect();

                let (frequencies_fft, signal_1_fft, phase_1_fft) = make_fft(
                    &data.time,
                    &data.signal_1,
                    normalize_fft,
                    &df,
                    &lower_bound,
                    &upper_bound,
                );
                let (_, ref_1_fft, ref_phase_1_fft) = make_fft(
                    &data.time,
                    &data.ref_1,
                    normalize_fft,
                    &df,
                    &lower_bound,
                    &upper_bound,
                );

                data.frequencies_fft = frequencies_fft;
                data.signal_1_fft = signal_1_fft;
                data.phase_1_fft = phase_1_fft;
                data.ref_1_fft = ref_1_fft;
                data.ref_phase_1_fft = ref_phase_1_fft;

                data.filtered_phase_1_fft = data.phase_1_fft.clone();
                data.filtered_signal_1_fft = data.signal_1_fft.clone();
                // calculate the intensity by summing the squares
                let sig_squared: Vec<f64> = data.signal_1.par_iter().map(|x| x.powi(2)).collect();
                scan.set_img(x, y, sig_squared.par_iter().sum());
                scan.set_data(x, y, data.clone());

                if let Ok(mut write_guard) = img_lock.write() {
                    let img = Array2::from_shape_fn((scan.width, scan.height), |(x, y)| {
                        scan.get_img(x, y)
                    });
                    *write_guard = img;
                }
            }
        }

        if let Ok(mut write_guard) = waterfall_lock.write() {
            let len = scan.data[0].signal_1_fft.len();
            let img = Array2::from_shape_fn((len, scan.height), |(x, y)| {
                scan.get_data(scan.width - 1, y)
                    .filtered_signal_1_fft
                    .clone()[x]
            });
            *write_guard = img;
        }
        let mut pixel = SelectedPixel::new();

        loop {
            let old_lb = lower_bound;
            let old_ub = upper_bound;
            if let Ok(read_guard) = fft_bounds_lock.read() {
                lower_bound = read_guard[0].clone();
                upper_bound = read_guard[1].clone();
            }
            let old_df = df;
            if let Ok(read_guard) = df_lock.read() {
                df = read_guard.clone();
            }
            let old_pixel = pixel.clone();
            if let Ok(read_guard) = pixel_lock.read() {
                pixel = read_guard.clone();
            }
            if pixel.x != old_pixel.x
                || pixel.y != old_pixel.y
                || old_df != df
                || old_lb != lower_bound
                || old_ub != upper_bound
            {
                data = scan.get_data(pixel.x as usize, pixel.y as usize);

                let (frequencies_fft, signal_1_fft, phase_1_fft) = make_fft(
                    &data.time,
                    &data.signal_1,
                    normalize_fft,
                    &df,
                    &lower_bound,
                    &upper_bound,
                );
                let (_, ref_1_fft, ref_phase_1_fft) = make_fft(
                    &data.time,
                    &data.ref_1,
                    normalize_fft,
                    &df,
                    &lower_bound,
                    &upper_bound,
                );

                data.frequencies_fft = frequencies_fft;
                data.signal_1_fft = signal_1_fft;
                data.phase_1_fft = phase_1_fft;
                data.ref_1_fft = ref_1_fft;
                data.ref_phase_1_fft = ref_phase_1_fft;

                // open hk file of selected pixel
                let mut hk_path = opened_file_path.clone();
                hk_path.push(format!("pixel_ID={:05}-{:05}_hk.csv", pixel.x, pixel.y));
                match open_hk(&mut data.hk, &hk_path) {
                    Ok((x, y)) => {}
                    Err(err) => {
                        println!("failed to open hk: {err}");
                    }
                }

                if let Ok(mut write_guard) = img_lock.write() {
                    let img = Array2::from_shape_fn((scan.width, scan.height), |(x, y)| {
                        scan.get_img(x, y)
                    });
                    *write_guard = img;
                }
                if let Ok(mut write_guard) = waterfall_lock.write() {
                    let len = scan.data[0].signal_1_fft.len();
                    let img = Array2::from_shape_fn((len, scan.height), |(x, y)| {
                        scan.get_data(pixel.x as usize, y)
                            .filtered_signal_1_fft
                            .clone()[x]
                    });
                    *write_guard = img;
                }

                if let Ok(mut write_guard) = data_lock.write() {
                    *write_guard = data.clone();
                }
            }

            let old_bounds = filter_bounds;
            if let Ok(read_guard) = fft_filter_bounds_lock.read() {
                filter_bounds = read_guard.clone();
            }
            if filter_bounds != old_bounds {
                //TODO: iterate over image pixels instead of x and y
                // >> implement iter for scan object
                // for (pixel_data, img_data) in scan.data.iter_mut().zip(scan.img.iter_mut()) {
                scan.data
                    .par_iter_mut()
                    .zip(scan.img.par_iter_mut())
                    .for_each(|(pixel_data, img_data)| {
                        //scan.iter_mut().for_each(|(pixel_data, img_data)| {
                        apply_filter(pixel_data, &filter_bounds);
                        pixel_data.filtered_signal_1 = make_ifft(
                            &pixel_data.frequencies_fft,
                            &pixel_data.filtered_signal_1_fft,
                            &pixel_data.filtered_phase_1_fft,
                        );
                        // calculate the intensity by summing the squares
                        let sig_squared: Vec<f64> = pixel_data
                            .filtered_signal_1
                            .par_iter()
                            .map(|x| x.powi(2))
                            .collect();
                        *img_data = sig_squared.par_iter().sum();
                    });
                if let Ok(mut write_guard) = img_lock.write() {
                    let img = Array2::from_shape_fn((scan.width, scan.height), |(x, y)| {
                        scan.get_img(x, y)
                    });
                    *write_guard = img;
                }
                if let Ok(mut write_guard) = waterfall_lock.write() {
                    let len = scan.data[0].signal_1_fft.len();
                    let img = Array2::from_shape_fn((len, scan.height), |(x, y)| {
                        scan.get_data(pixel.x as usize, y)
                            .filtered_signal_1_fft
                            .clone()[x]
                    });
                    *write_guard = img;
                }
                if let Ok(mut write_guard) = data_lock.write() {
                    *write_guard = scan.get_data(pixel.x as usize, pixel.y as usize);
                }
            }

            // TODO: check for new file

            std::thread::sleep(Duration::from_millis(10));
        }
    }
}
