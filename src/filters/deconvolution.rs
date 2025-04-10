//! This module implements a custom filter named `Deconvolution`, which operates on scanned images
//! and performs a deconvolution operation in the frequency domain.
//!
//! The implementation includes a Richardson-Lucy deconvolution algorithm placeholder,
//! allowing for further customization and parameterization.

use crate::config::GuiThreadCommunication;
use crate::data_container::ScannedImage;
use crate::filters::filter::{Filter, FilterConfig, FilterDomain};
use crate::filters::psf::create_psf_2d;
use crate::filters::psf::gaussian2;
use crate::gui::application::GuiSettingsContainer;
use eframe::egui::{self, Ui};
use filter_macros::register_filter;
use ndarray::parallel::prelude::IntoParallelIterator;
use ndarray::parallel::prelude::ParallelIterator;
use ndarray::{arr1, s, Array1, Array2, Array3, Axis, Zip};
use rustfft::{num_complex::Complex, FftPlanner};

/// Represents a `Deconvolution` filter.
///
/// This filter is designed to perform deconvolution using a configurable number of iterations
/// and a defined frequency range. It is implemented to work in the frequency domain.
///
/// Fields:
/// - `filter_number`: A placeholder for selecting predefined filters within the algorithm.
/// - `start_frequency`: The starting range for the frequency domain.
/// - `end_frequency`: The ending range for the frequency domain.
/// - `n_iterations`: The number of iterations for performing the deconvolution.
#[derive(Debug)]
#[register_filter]
pub struct Deconvolution {
    pub n_iterations: usize,
    pub filter_number: usize,
    pub start_frequency: f64,
    pub end_frequency: f64,
}

/// Perform fast convolution using FFT for signals of different lengths
pub fn convolve1d(a: &Array1<f32>, b: &Array1<f32>) -> Array1<f32>{
    let conv_size = a.len(); // Adjusted convolution size
    let fft_size = conv_size.next_power_of_two(); // Use next power of two for efficiency

    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(fft_size);
    let ifft = planner.plan_fft_inverse(fft_size);

    // Pad input signals to the FFT size
    let mut a_padded: Vec<Complex<f64>> = vec![Complex { re: 0.0, im: 0.0 }; fft_size];
    let mut b_padded: Vec<Complex<f64>> = vec![Complex { re: 0.0, im: 0.0 }; fft_size];

    for i in 0..a.len() {
        a_padded[i] = Complex { re: a[i] as f64, im: 0.0 };
    }
    for i in 0..b.len() {
        b_padded[i] = Complex { re: b[i] as f64, im: 0.0 };
    }

    // Perform FFT on both signals
    fft.process(&mut a_padded);
    fft.process(&mut b_padded);

    // Pointwise multiplication in the frequency domain
    let mut result_freq: Vec<Complex<f64>> = a_padded
        .iter()
        .zip(b_padded.iter())
        .map(|(x, y)| x * y)
        .collect();

    // Perform inverse FFT to get back to time domain
    ifft.process(&mut result_freq);

    // Normalize and extract the result
    Array1::from(
        result_freq[..conv_size]
            .iter()
            .take(a.len())
            .map(|c| (c.re / fft_size as f64) as f32) // Normalize by FFT size and cast to f32
            .collect::<Vec<f32>>(),
    )
}

/// Perform fast 2D convolution using FFT for matrices of different sizes
pub fn convolve2d(a: &Array2<f32>, b: &Array2<f32>) -> Array2<f32> {
    let shape_a = a.shape();
    let shape_b = b.shape();
    let conv_shape = (shape_a[0] + shape_b[0] - 1, shape_a[1] + shape_b[1] - 1);
    let fft_shape = (
        conv_shape.0.next_power_of_two(),
        conv_shape.1.next_power_of_two(),
    );

    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(fft_shape.0 * fft_shape.1);
    let ifft = planner.plan_fft_inverse(fft_shape.0 * fft_shape.1);

    // Pad input arrays to the FFT size
    let mut a_padded: Vec<Complex<f64>> = vec![Complex { re: 0.0, im: 0.0 }; fft_shape.0 * fft_shape.1];
    let mut b_padded: Vec<Complex<f64>> = vec![Complex { re: 0.0, im: 0.0 }; fft_shape.0 * fft_shape.1];

    for i in 0..shape_a[0] {
        for j in 0..shape_a[1] {
            a_padded[i * fft_shape.1 + j] = Complex { re: a[[i, j]] as f64, im: 0.0 };
        }
    }
    for i in 0..shape_b[0] {
        for j in 0..shape_b[1] {
            b_padded[i * fft_shape.1 + j] = Complex { re: b[[i, j]] as f64, im: 0.0 };
        }
    }

    // Perform FFT on both arrays
    fft.process(&mut a_padded);
    fft.process(&mut b_padded);

    // Pointwise multiplication in the frequency domain
    let mut result_freq: Vec<Complex<f64>> = a_padded
        .iter()
        .zip(b_padded.iter())
        .map(|(x, y)| x * y)
        .collect();

    // Perform inverse FFT to get back to the spatial domain
    ifft.process(&mut result_freq);

    // Normalize and extract the result
    let mut result = Array2::<f32>::zeros(conv_shape);
    for i in 0..conv_shape.0 {
        for j in 0..conv_shape.1 {
            result[[i, j]] = (result_freq[i * fft_shape.1 + j].re / (fft_shape.0 * fft_shape.1) as f64) as f32;
        }
    }

    result
}

impl Deconvolution {
    /// Computes the minimum maximum range for the deconvolution algorithm
    /// as a range_max too small can lead to deconvolution errors.
    fn range_max_min(&self, range_max: f32, wmin: f32) -> f32 {
        if range_max < wmin {
            wmin
        } else {
            range_max
        }
    }

    /// Computes the filtered scan with the FIR filter by convolving each time trace with the filter.
    fn filter_scan(&self, _scan: &mut ScannedImage, filter: &Array1<f32>) -> Array3<f32> {
        println!("Starting FIR filter scan...");
        let mut filtered_data = Array3::<f32>::zeros((
            _scan.raw_data.dim().0,
            _scan.raw_data.dim().1,
            _scan.raw_data.dim().2,
        ));
        for i in 0.._scan.raw_data.dim().0 {
            for j in 0.._scan.raw_data.dim().1 {
                println!("Filtering time trace at position ({}, {})", i, j);
                filtered_data.slice_mut(s![i, j, ..]).assign(&convolve1d(
                    &_scan.raw_data.slice(s![i, j, ..]).to_owned(),
                    filter
                ));
            }
        }
        println!("FIR filter scan completed.");
        filtered_data
    }

    fn richardson_lucy(
        &self,
        image: &Array2<f32>,
        psf: &Array2<f32>,
        n_iterations: usize,
    ) -> Array2<f32> {
        println!("Starting Richardson-Lucy deconvolution with {} iterations...", n_iterations);
        let psf_mirror = psf.slice(s![..;-1, ..;-1]).to_owned(); // Flip kernel
                                                                 // Copying d in u as a first guess
        let mut u = image.clone();
        // Regularization parameter
        let eps: f32 = 1e-12;
        // Iterating
        for iter in 0..n_iterations {
            println!("Iteration {}/{}", iter + 1, n_iterations);
            let conv = convolve2d(&u, &psf);
            let relative_blur = Zip::from(image)
                .and(&conv)
                .map_collect(|&o, &c| o / (c + eps)); // Avoid division by zero
            let correction = convolve2d(
                &relative_blur,
                &psf_mirror,
            );
            Zip::from(&mut u).and(&correction).for_each(|e, &c| *e *= c); // Element-wise multiplication
        }
        println!("Richardson-Lucy deconvolution completed.");
        u
    }
}

impl Filter for Deconvolution {
    /// Creates a new `Deconvolution` filter with default settings.
    ///
    /// Default values:
    /// - `n_iterations`: 10
    /// - `filter_number`: 10
    /// - `start_frequency`: 0.0
    /// - `end_frequency`: 10.0
    fn new() -> Self {
        Deconvolution {
            n_iterations: 10,
            filter_number: 10,
            start_frequency: 0.0,
            end_frequency: 10.0,
        }
    }

    fn config(&self) -> FilterConfig {
        FilterConfig {
            name: "Deconvolution".to_string(),
            domain: FilterDomain::Frequency,
        }
    }

    /// Applies the deconvolution algorithm to a scanned image.
    ///
    /// # Arguments:
    /// - `_scan`: Mutable reference to the scanned image to be processed.
    /// - `_gui_settings`: Mutable reference to the GUI settings associated with the filter.
    ///
    /// # Notes:
    /// This method currently contains a placeholder for the Richardson-Lucy algorithm.
    fn filter(&self, scan: &mut ScannedImage, gui_settings: &mut GuiSettingsContainer) {
        println!("Starting deconvolution filter...");
        // Initializing _scan.filtered_data to zeros
        scan.filtered_data = Array3::zeros((
            scan.raw_data.dim().0,
            scan.raw_data.dim().1,
            scan.raw_data.dim().2,
        ));
        // Iterate over the frequencies/filters contained in the psf
        for (i, filter) in gui_settings.psf.filters.outer_iter().enumerate() {
            println!("Processing filter {}...", i);
            // Compute range_max_x and range_max_y with (w_x + |x_0|) * 3 and (w_y + |y_0|) * 3
            let mut range_max_x: f32 = (gui_settings.psf.popt_x.row(i)[1] as f32
                + gui_settings.psf.popt_x.row(i)[0].abs() as f32)
                * 3.0;
            let mut range_max_y: f32 = (gui_settings.psf.popt_y.row(i)[1] as f32
                + gui_settings.psf.popt_y.row(i)[0].abs() as f32)
                * 3.0;
            // Compute the minimum range_max_x and range_max_y
            // wmin is set to 2.5 to avoid deconvolution errors
            let wmin: f32 = 2.5;
            range_max_x = self.range_max_min(range_max_x, wmin);
            range_max_y = self.range_max_min(range_max_y, wmin);
            // Round the range_max_x and range_max_y to the nearest dx and dy steps
            range_max_x = (range_max_x / scan.dx.unwrap() as f32).floor() * scan.dx.unwrap() as f32
                + scan.dx.unwrap() as f32;
            range_max_y = (range_max_y / scan.dy.unwrap() as f32).floor() * scan.dy.unwrap() as f32
                + scan.dy.unwrap() as f32;
            // Create two vectors x and y with range_max_x and range_max_y using the dx and dy steps from the scan
            let x: Vec<f32> = (-((range_max_x / scan.dx.unwrap() as f32).floor() as isize)
                ..=((range_max_x / scan.dx.unwrap() as f32).floor() as isize))
                .map(|i| i as f32 * scan.dx.unwrap() as f32)
                .collect();
            let y: Vec<f32> = (-((range_max_y / scan.dy.unwrap() as f32).floor() as isize)
                ..=((range_max_y / scan.dy.unwrap() as f32).floor() as isize))
                .map(|i| i as f32 * scan.dy.unwrap() as f32)
                .collect();
            // Create the x and y psfs

            // TODO @Arnaud, as I understand popt_x is a (2,100) dimensional array, but the gaussian2 functions takes two single parameters, w and x0
            let gaussian_x: Vec<f32> = gaussian2(&arr1(&x), &gui_settings.psf.popt_x.row(i).as_slice().unwrap()).to_vec();
            let gaussian_y: Vec<f32> = gaussian2(&arr1(&y), &gui_settings.psf.popt_y.row(i).as_slice().unwrap()).to_vec();
            // Create the 2D PSF
            println!("Creating 2D PSF...");
            let psf_2d: Array2<f32> = create_psf_2d(
                gaussian_x,
                gaussian_y,
                x,
                y,
                scan.dx.unwrap() as f32,
                scan.dy.unwrap() as f32,
            );
            // Filter the scan with the FIR filter of the given frequency
            println!("Filtering scan with FIR filter...");
            let mut filtered_data: Array3<f32> = self.filter_scan(scan, &gui_settings.psf.filters.row(i).to_owned());

            // Computing the filtered image by summing the squared samples for each pixel
            let mut filtered_image =
                Array2::default((filtered_data.shape()[0], filtered_data.shape()[1]));
            (
                filtered_data.axis_iter_mut(Axis(0)),
                filtered_image.axis_iter_mut(Axis(0)),
            )
                .into_par_iter()
                .for_each(|(mut filtered_data_columns, mut filtered_img_columns)| {
                    (
                        filtered_data_columns.axis_iter_mut(Axis(0)),
                        filtered_img_columns.axis_iter_mut(Axis(0)),
                    )
                        .into_par_iter()
                        .for_each(|(filtered_data, mut filtered_img)| {
                            *filtered_img.into_scalar() =
                                filtered_data.iter().map(|xi| xi * xi).sum::<f32>();
                        });
                });
            // Number of iterations for the deconvolution
            let wx_min: f32 = gui_settings
                .psf
                .popt_x
                .rows()
                .into_iter()
                .map(|row| row[1])
                .filter(|&x| x.is_finite()) // Filter out NaN and Infinity values
                .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Greater)) // Use partial_cmp to handle f32 comparison
                .unwrap(); // Unwrap since we know there's at least one finite value
            let wx_max: f32 = gui_settings
                .psf
                .popt_x
                .rows()
                .into_iter()
                .map(|row| row[1])
                .filter(|&x| x.is_finite()) // Filter out NaN and Infinity values
                .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Less)) // Use partial_cmp to handle f32 comparison
                .unwrap();
            let wy_min: f32 = gui_settings
                .psf
                .popt_y
                .rows()
                .into_iter()
                .map(|row| row[1])
                .filter(|&x| x.is_finite()) // Filter out NaN and Infinity values
                .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Greater)) // Use partial_cmp to handle f32 comparison
                .unwrap();
            let wy_max: f32 = gui_settings
                .psf
                .popt_y
                .rows()
                .into_iter()
                .map(|row| row[1])
                .filter(|&x| x.is_finite()) // Filter out NaN and Infinity values
                .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Less)) // Use partial_cmp to handle f32 comparison
                .unwrap(); // Unwrap since we know there's at least one finite value
            let w_min: f32 = wx_min.min(wy_min);
            let w_max: f32 = wx_max.max(wy_max);
            let n_iter_min: usize = 1;
            // The number of iterations depends on the width of the PSF
            let n_iter: usize = (((gui_settings.psf.popt_x.row(i)[1] - w_min) / (w_max - w_min)
                * (self.n_iterations as f32 - n_iter_min as f32)
                + n_iter_min as f32)
                .floor()) as usize;

            // Perform the deconvolution with the Richardson-Lucy algorithm
            println!("Performing Richardson-Lucy deconvolution...");
            let deconvolved_image: Array2<f32> =
                self.richardson_lucy(&filtered_image, &psf_2d, n_iter);
            // Computing the gains per pixel for the current frequency
            let mut deconvolution_gains: Array2<f32> =
                Array2::zeros((scan.raw_data.dim().0, scan.raw_data.dim().1));

            // TODO: this does not work yet

            println!("Applying deconvolution gains...");
            Zip::from(&deconvolved_image)
                .and(&filtered_image)
                .and(&mut deconvolution_gains)
                .for_each(|&d, &f, g| *g = (d / f).sqrt());
            // Applying the gains to the filtered data
            for (data, gain) in filtered_data.iter_mut().zip(deconvolution_gains.iter()) {
                *data *= gain;
               }
            // Adding filtered data to scan.filtered_data
            println!("Adding filtered data to scan.filtered_data...");
            scan.filtered_data += &filtered_data;
        }

        // Computing the filtered image in scan.filtered_img by summing the squared samples for each pixel
        (
            scan.filtered_data.axis_iter_mut(Axis(0)),
            scan.filtered_img.axis_iter_mut(Axis(0)),
        )
            .into_par_iter()
            .for_each(|(mut filtered_data_columns, mut filtered_img_columns)| {
                (
                    filtered_data_columns.axis_iter_mut(Axis(0)),
                    filtered_img_columns.axis_iter_mut(Axis(0)),
                )
                    .into_par_iter()
                    .for_each(|(filtered_data, mut filtered_img)| {
                        *filtered_img.into_scalar() =
                            filtered_data.iter().map(|xi| xi * xi).sum::<f32>();
                    });
            });
        println!("Deconvolution filter completed.");
    }
    fn ui(
        &mut self,
        ui: &mut Ui,
        _thread_communication: &mut GuiThreadCommunication,
    ) -> egui::Response {
        // thread_communication can be used, but is not required. It contains the gui_settings GuiSettingsContainer
        // implement your GUI parameter handling here, for example like this:
        ui.horizontal(|ui| {
            ui.label("Iterations: ");
            ui.add(egui::Slider::new(&mut self.n_iterations, 0..=10))
        })
        .inner
    }
}
