//! This module defines data structures and functionality for managing meta-information, housekeeping data,
//! scanned images, and related operations for image and signal processing tasks.

use ndarray::{Array1, Array2, Array3};
use realfft::{ComplexToReal, RealToComplex};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use realfft::num_complex::Complex32;

/// Represents the metadata associated with an image or scan.
///
/// # Fields
/// - `timestamp`: The time at which the data was recorded, represented as a floating-point value.
/// - `width`: Width of the scanned image in pixels.
/// - `height`: Height of the scanned image in pixels.
/// - `dx`: The horizontal resolution or spacing between data points.
/// - `x_min`: The minimum value of the x-axis in the scan.
/// - `x_max`: The maximum value of the x-axis in the scan.
/// - `dy`: The vertical resolution or spacing between data points.
/// - `y_min`: The minimum value of the y-axis in the scan.
/// - `y_max`: The maximum value of the y-axis in the scan.
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

/// Represents housekeeping data associated with a scan or experiment.
///
/// # Fields
/// - `dx`, `dy`: Resolutions in the x and y directions.
/// - `x_range`, `y_range`: The range of values for the x and y axes.
/// - `t_begin`: The beginning time of the scan or measurement.
/// - `range`: The total measurement range.
/// - `ambient_temperature`, `ambient_pressure`, `ambient_humidity`: Environmental parameters.
/// - `sample_temperature`: The temperature of the sample being scanned or measured.
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

/// Represents a single data point in the scan or experiment.
///
/// # Fields
/// - `hk`: Associated housekeeping metadata.
/// - `time`: Time axis data.
/// - `signal_1`: Primary signal data.
/// - `filtered_signal_1`: Filtered signal data.
/// - `avg_signal_1`: Averaged signal data.
/// - `frequencies`: Frequency axis data.
/// - `signal_1_fft`, `phase_1_fft`: Raw FFT and phase data for `signal_1`.
/// - `filtered_signal_1_fft`, `filtered_phase_fft`: FFT and phase data for the filtered signal.
/// - `avg_signal_1_fft`, `avg_phase_fft`: Averaged FFT and phase data.
#[derive(Clone, Default, Debug)]
pub struct DataPoint {
    pub hk: HouseKeeping,
    pub time: Vec<f32>,
    pub filtered_time: Vec<f32>,
    pub signal_1: Vec<f32>,
    pub filtered_signal_1: Vec<f32>,
    pub avg_signal_1: Vec<f32>,
    pub frequencies: Vec<f32>,
    pub filtered_frequencies: Vec<f32>,
    pub signal_1_fft: Vec<f32>,
    pub phase_1_fft: Vec<f32>,
    pub filtered_signal_1_fft: Vec<f32>,
    pub filtered_phase_fft: Vec<f32>,
    pub avg_signal_1_fft: Vec<f32>,
    pub avg_phase_fft: Vec<f32>,
}

/// Represents a scanned image including raw and processed image data,
/// metadata, and auxiliary processing information.
///
/// # Fields
/// - `x_min`, `dx`: Minimum x value and resolution in the x-direction.
/// - `y_min`, `dy`: Minimum y value and resolution in the y-direction.
/// - `height`, `width`: Dimensions of the image in pixels.
/// - `scaling`: Scaling factor applied to the image.
/// - `r2c`: Real-to-complex FFT transform (optional).
/// - `c2r`: Complex-to-real FFT transform (optional).
/// - `time`, `frequencies`: Time and frequency axis data.
/// - `raw_img`, `filtered_img`: 2D arrays representing raw and filtered image data.
/// - `raw_data`, `scaled_data`, `filtered_data`: 3D arrays for scan data in various states.
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
    pub filtered_r2c: Option<Arc<dyn RealToComplex<f32>>>,
    pub filtered_c2r: Option<Arc<dyn ComplexToReal<f32>>>,
    pub time: Array1<f32>,
    pub filtered_time: Array1<f32>,
    pub frequencies: Array1<f32>,
    pub filtered_frequencies: Array1<f32>,
    pub raw_img: Array2<f32>,
    pub raw_data: Array3<f32>,
    pub scaled_data: Array3<f32>,
    pub filtered_img: Array2<f32>,
    pub filtered_data: Array3<f32>,
}

impl Default for ScannedImage {
    /// Creates a new `ScannedImage` instance with the specified dimensions and parameters.
    ///
    /// # Arguments
    /// - `height`: The height of the image in pixels.
    /// - `width`: The width of the image in pixels.
    /// - `x_min`, `dx`: Minimum x value and resolution in the x-direction.
    /// - `y_min`, `dy`: Minimum y value and resolution in the y-direction.
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
            filtered_r2c: None,
            filtered_c2r: None,
            time: Array1::default(1),
            filtered_time: Array1::default(1),
            frequencies: Array1::default(1),
            filtered_frequencies: Array1::default(1),
            raw_img: Array2::default((1, 1)),
            raw_data: Array3::default((1, 1, 1)),
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
            filtered_r2c: None,
            filtered_c2r: None,
            time: Array1::default(1),
            filtered_time: Array1::default(1),
            frequencies: Array1::default(1),
            filtered_frequencies: Array1::default(1),
            raw_img: Array2::default((1, 1)),
            raw_data: Array3::default((1, 1, 1)),
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

    /// Rescales the image and associated data using the current scaling factor.
    ///
    /// - If scaling is `1`, no changes are made to the data.
    /// - Data is averaged in blocks of size `(scaling x scaling)` to produce scaled-down versions for both 2D and 3D data.
    pub fn rescale(&mut self) {
        let scale = self.scaling;
        if scale <= 1 {
            // No rescaling needed if scale is 1 or less
            self.scaled_data = self.raw_data.clone();
            return;
        }

        let new_height = self.height / scale;
        let new_width = self.width / scale;

        // Initialize scaled_img with the new dimensions
        self.filtered_img = Array2::zeros((new_width, new_height));

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
                self.filtered_img[(x, y)] = sum / (scale * scale) as f32;
            }
        }

        // Initialize scaled_data with the new dimensions and depth
        let depth = self.time.len();
        self.scaled_data = Array3::zeros((new_width, new_height, depth));

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

#[derive(Default, Clone)]
pub struct ScannedImageFilterData {
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
    pub img: Array2<f32>,
    pub data: Array3<f32>,
    pub frequency: Array1<f32>,
    pub fft: Array3<Complex32>,
    pub amplitudes: Array3<f32>,
    pub phases: Array3<f32>,
}

#[derive(Default, Clone)]
pub struct ScannedImageTimeDomain {
    pub x_min: Option<f32>,
    pub dx: Option<f32>,
    pub y_min: Option<f32>,
    pub dy: Option<f32>,
    pub height: usize,
    pub width: usize,
    pub scaling: usize,
    pub c2r: Option<Arc<dyn ComplexToReal<f32>>>,
    pub time: Array1<f32>,
    pub img: Array2<f32>,
    pub data: Array3<f32>,
}

#[derive(Default, Clone)]
pub struct ScannedImageFrequencyDomain {
    pub x_min: Option<f32>,
    pub dx: Option<f32>,
    pub y_min: Option<f32>,
    pub dy: Option<f32>,
    pub height: usize,
    pub width: usize,
    pub scaling: usize,
    pub c2r: Option<Arc<dyn ComplexToReal<f32>>>,
    pub frequency: Array1<f32>,
    pub fft: Array3<Complex32>,
    pub amplitudes: Array3<f32>,
    pub phases: Array3<f32>,
}