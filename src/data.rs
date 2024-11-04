use std::ops::AddAssign;
use std::sync::Arc;

use ndarray::{Array1, Array2, Array3, Axis};
use realfft::{ComplexToReal, RealToComplex};
use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Meta {
    pub timestamp: f64,
    pub width: usize,
    pub height: usize,
    pub dx: f32,
    pub x_min: f32,
    pub x_max: f32,
    pub dy: f32,
    pub y_min: f32,
    pub y_max: f32,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct HouseKeeping {
    pub dx: f32,
    pub x_range: [f32; 2],
    pub dy: f32,
    pub y_range: [f32; 2],
    pub t_begin: f32,
    pub range: f32,
    pub ambient_temperature: f64,
    pub ambient_pressure: f64,
    pub ambient_humidity: f64,
    pub sample_temperature: f64,
}

impl Default for HouseKeeping {
    fn default() -> Self {
        Self {
            dx: 1.0,
            x_range: [0.0, 10.0],
            dy: 1.0,
            y_range: [0.0, 10.0],
            t_begin: 1000.0,
            range: 50.0,
            ambient_temperature: 22.0,
            ambient_pressure: 950.0,
            ambient_humidity: 50.0,
            sample_temperature: 0.0,
        }
    }
}

#[derive(Clone, Default, Debug)]
pub struct DataPoint {
    pub hk: HouseKeeping,
    pub time: Vec<f32>,
    pub signal_1: Vec<f32>,
    pub filtered_signal_1: Vec<f32>,
    pub frequencies: Vec<f32>,
    pub signal_1_fft: Vec<f32>,
    pub phase_1_fft: Vec<f32>,
    pub filtered_signal_1_fft: Vec<f32>,
    pub filtered_phase_fft: Vec<f32>,
}

#[derive(Clone)]
pub struct ScannedImage {
    pub x_min: Option<f32>,
    pub dx: Option<f32>,
    pub y_min: Option<f32>,
    pub dy: Option<f32>,
    pub height: usize,
    pub width: usize,
    pub scaling: usize,
    pub r2c: Option<Arc<dyn RealToComplex<f32>>>,
    pub c2r: Option<Arc<dyn ComplexToReal<f32>>>,
    pub time: Array1<f32>,
    pub frequencies: Array1<f32>,
    pub raw_img: Array2<f32>,
    pub raw_data: Array3<f32>,
    pub scaled_img: Array2<f32>,
    pub scaled_data: Array3<f32>,
    pub filtered_img: Array2<f32>,
    pub filtered_data: Array3<f32>,
}

impl Default for ScannedImage {
    fn default() -> Self {
        ScannedImage {
            x_min: None,
            dx: None,
            y_min: None,
            dy: None,
            height: 0,
            width: 0,
            scaling: 1,
            r2c: None,
            c2r: None,
            time: Array1::default(1),
            frequencies: Array1::default(1),
            raw_img: Array2::default((1, 1)),
            raw_data: Array3::default((1, 1, 1)),
            scaled_img: Array2::default((1, 1)),
            scaled_data: Array3::default((1, 1, 1)),
            filtered_img: Array2::default((1, 1)),
            filtered_data: Array3::default((1, 1, 1)),
        }
    }
}

impl ScannedImage {
    pub fn new(
        height: usize,
        width: usize,
        x_min: f32,
        y_min: f32,
        dx: f32,
        dy: f32,
    ) -> ScannedImage {
        ScannedImage {
            x_min: Some(x_min),
            dx: Some(dx),
            y_min: Some(y_min),
            dy: Some(dy),
            height,
            width,
            scaling: 1,
            r2c: None,
            c2r: None,
            time: Array1::default(1),
            frequencies: Array1::default(1),
            raw_img: Array2::default((1, 1)),
            raw_data: Array3::default((1, 1, 1)),
            scaled_img: Array2::default((1, 1)),
            scaled_data: Array3::default((1, 1, 1)),
            filtered_img: Array2::default((1, 1)),
            filtered_data: Array3::default((1, 1, 1)),
        }
    }

    // pub fn get_raw_data(&mut self, x: usize, y: usize) -> DataPoint {
    //     self.raw_data[x * self.width + y].clone()
    // }
    //
    // pub fn set_raw_data(&mut self, x: usize, y: usize, data: DataPoint) {
    //     self.raw_data[x * self.width + y] = data;
    // }
    //
    // pub fn get_scaled_data(&mut self, x: usize, y: usize) -> DataPoint {
    //     self.scaled_data[x * self.width + y].clone()
    // }
    //
    // pub fn set_scaled_data(&mut self, x: usize, y: usize, data: DataPoint) {
    //     self.scaled_data[x * self.width + y] = data;
    // }
    //
    // pub fn get_raw_img(&mut self, x: usize, y: usize) -> f32 {
    //     self.raw_img[x * self.width + y]
    // }
    //
    // pub fn set_raw_img(&mut self, x: usize, y: usize, val: f32) {
    //     self.raw_img[x * self.width + y] = val;
    // }

    pub fn rescale(&mut self) {
        let scale = self.scaling;
        if scale <= 1 {
            // No rescaling needed if scale is 1 or less
            self.scaled_img = self.raw_img.clone();
            self.scaled_data = self.raw_data.clone();
            return;
        }

        let new_height = self.height / scale;
        let new_width = self.width / scale;

        // Initialize scaled_img with the new dimensions
        self.scaled_img = Array2::zeros((new_height, new_width));

        // Rescale the raw_img into scaled_img
        for y in 0..new_height {
            for x in 0..new_width {
                // Calculate the sum of the scale x scale block
                let mut sum = 0.0;
                for dy in 0..scale {
                    for dx in 0..scale {
                        sum += self.raw_img[(x * scale + dx, y * scale + dy)];
                    }
                }
                // Average and store in scaled_img
                self.scaled_img[(x, y)] = sum / (scale * scale) as f32;
            }
        }

        // Initialize scaled_data with the new dimensions and depth
        let depth = self.time.len();
        self.scaled_data = Array3::zeros((new_height, new_width, depth));

        // Rescale each layer in raw_data into scaled_data
        for y in 0..new_height {
            for x in 0..new_width {
                for d in 0..depth {
                    // Calculate the sum of the scale x scale block for each depth slice
                    let mut sum = 0.0;
                    for dy in 0..scale {
                        for dx in 0..scale {
                            sum += self.raw_data[(x * scale + dx, y * scale + dy, d)];
                        }
                    }
                    // Average and store in scaled_data
                    self.scaled_data[(x, y, d)] = sum / (scale * scale) as f32;
                }
            }
        }
    }
}
//
// impl ScannedImage {
//     fn iter(
//         &self,
//     ) -> ScannedImageIter<impl Iterator<Item = &DataPoint>, impl Iterator<Item = &f32>> {
//         ScannedImageIter {
//             data: self.data.iter(),
//             img: self.img.iter(),
//         }
//     }
//     fn iter_mut(
//         &mut self,
//     ) -> ScannedImageIterMut<impl Iterator<Item = &mut DataPoint>, impl Iterator<Item = &mut f32>>
//     {
//         ScannedImageIterMut {
//             data: self.data.iter_mut(),
//             img: self.img.iter_mut(),
//         }
//     }
// }
//
// struct ScannedImageIter<D, I> {
//     data: D,
//     img: I,
// }
//
// impl<'r, D, I> Iterator for ScannedImageIter<D, I>
// where
//     D: Iterator<Item = &'r DataPoint>,
//     I: Iterator<Item = &'r f32>,
// {
//     type Item = (&'r DataPoint, &'r f32);
//
//     fn next(&mut self) -> Option<Self::Item> {
//         match self.data.next() {
//             Some(d) => self.img.next().map(|i| (d, i)),
//             None => None,
//         }
//     }
// }
//
// struct ScannedImageIterMut<D, I> {
//     data: D,
//     img: I,
// }
//
// impl<'r, D, I> Iterator for ScannedImageIterMut<D, I>
// where
//     D: Iterator<Item = &'r mut DataPoint>,
//     I: Iterator<Item = &'r mut f32>,
// {
//     type Item = (&'r mut DataPoint, &'r mut f32);
//
//     fn next(&mut self) -> Option<Self::Item> {
//         match self.data.next() {
//             Some(d) => self.img.next().map(|i| (d, i)),
//             None => None,
//         }
//     }
// }
