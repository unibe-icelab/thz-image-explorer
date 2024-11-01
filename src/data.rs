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
    pub path: String,
    pub x_min: f32,
    pub dx: f32,
    pub y_min: f32,
    pub dy: f32,
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
            path: "".to_string(),
            x_min: 0.0,
            dx: 0.0,
            y_min: 0.0,
            dy: 0.0,
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
            path: "test".to_string(),
            x_min,
            dx,
            y_min,
            dy,
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

    pub fn rescale(&mut self, scaling: usize) {
        self.scaled_img = Array2::zeros((self.width / scaling, self.height / scaling));
        self.scaled_data =
            Array3::zeros((self.width / scaling, self.height / scaling, self.time.len()));

        for x in 0..self.width / scaling {
            for y in 0..self.height / scaling {
                // calculate the intensity by summing the squares
                let mut averaged_pulse: Array1<f32> = Array1::zeros(self.time.len());
                for x_step in 0..scaling {
                    for y_step in 0..scaling {
                        averaged_pulse.add_assign(
                            &self
                                .raw_data
                                .index_axis(Axis(0), x * scaling + x_step)
                                .index_axis(Axis(0), y * scaling + y_step),
                        );
                    }
                }
                averaged_pulse
                    .view_mut()
                    .mapv_inplace(|p| p / (scaling * scaling) as f32);

                self.scaled_data
                    .index_axis_mut(Axis(0), x)
                    .index_axis_mut(Axis(0), y)
                    .assign(&averaged_pulse);
                let sig_squared_sum = self
                    .scaled_data
                    .index_axis(Axis(0), x)
                    .index_axis(Axis(0), y)
                    .mapv(|xi| xi * xi)
                    .sum();
                self.scaled_img[[x, y]] = sig_squared_sum;
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
