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
use ndarray::{arr1, s, Array1, Array2, Array3, Axis, Zip};
use num_complex::Complex32;
use rayon::prelude::*;
use rustfft::{num_complex::Complex, FftPlanner};
use std::time::Instant;
use std::io::{self, Write};

use std::sync::Arc;

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
    pub n_iterations: usize
}

pub fn convolve1d(a: &Array1<f32>, b: &Array1<f32>) -> Array1<f32> {
    let conv_size = a.len() + b.len() - 1; // Adjusted convolution size
    let fft_size = conv_size.next_power_of_two(); // Use next power of two for efficiency

    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(fft_size);
    let ifft = planner.plan_fft_inverse(fft_size);

    // Pad input signals to the FFT size
    let mut a_padded: Vec<Complex<f64>> = vec![Complex { re: 0.0, im: 0.0 }; fft_size];
    let mut b_padded: Vec<Complex<f64>> = vec![Complex { re: 0.0, im: 0.0 }; fft_size];

    // Copy input data into padded arrays
    a.iter().enumerate().for_each(|(i, &val)| {
        a_padded[i] = Complex {
            re: val as f64,
            im: 0.0,
        };
    });

    b.iter().enumerate().for_each(|(i, &val)| {
        b_padded[i] = Complex {
            re: val as f64,
            im: 0.0,
        };
    });

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
        result_freq[..a.len()]
            .iter()
            .map(|c| (c.re / fft_size as f64) as f32) // Normalize by FFT size and cast to f32
            .collect::<Vec<f32>>(),
    )
}


/// Pad size to the next power of two
fn next_power_of_two(n: usize) -> usize {
    n.next_power_of_two()
}

/// Perform element-wise multiplication of two complex matrices
fn multiply_freq_domain(
    a: &Array2<Complex<f32>>,
    b: &Array2<Complex<f32>>,
) -> Array2<Complex<f32>> {
    let mut result = a.clone();
    Zip::from(&mut result)
        .and(b)
        .for_each(|r, &bval| *r *= bval);
    result
}

pub fn pad_array(input: &Array2<f32>, padded_shape: (usize, usize)) -> Array2<Complex32> {
    let (input_rows, input_cols) = input.dim();
    let (padded_rows, padded_cols) = padded_shape;

    assert!(padded_rows >= input_rows && padded_cols >= input_cols);

    let mut output = Array2::<Complex32>::zeros((padded_rows, padded_cols));

    // Copy input into top-left corner of output, converting to Complex32
    for y in 0..input_rows {
        for x in 0..input_cols {
            output[[y, x]] = Complex32::new(input[[y, x]], 0.0);
        }
    }

    output
}

/// Perform 2D FFT (in-place) on a matrix
fn fft2d(matrix: &mut Array2<Complex<f32>>, planner: &mut FftPlanner<f32>, inverse: bool) {
    let (rows, cols) = matrix.dim();
    let fft_cols: Arc<dyn rustfft::Fft<f32>> = if inverse {
        planner.plan_fft_inverse(cols)
    } else {
        planner.plan_fft_forward(cols)
    };
    let fft_rows: Arc<dyn rustfft::Fft<f32>> = if inverse {
        planner.plan_fft_inverse(rows)
    } else {
        planner.plan_fft_forward(rows)
    };

    // FFT on rows
    for mut row in matrix.outer_iter_mut() {
        fft_cols.process(row.as_slice_mut().unwrap());
    }

    // FFT on columns
    for x in 0..cols {
        let mut column: Vec<_> = (0..rows).map(|y| matrix[[y, x]]).collect();
        fft_rows.process(&mut column);
        for (y, val) in column.iter().enumerate() {
            matrix[[y, x]] = *val;
        }
    }

    // Normalize if inverse
    if inverse {
        let scale = (rows * cols) as f32;
        matrix.mapv_inplace(|v| v / scale);
    }
}

/// Direct 2D Convolution for small kernels
fn direct_convolve2d(a: &Array2<f32>, b: &Array2<f32>) -> Array2<f32> {
    let (a_rows, a_cols) = a.dim();
    let (b_rows, b_cols) = b.dim();

    let mut result = Array2::<f32>::zeros((a_rows, a_cols));

    for i in 0..a_rows {
        for j in 0..a_cols {
            let mut sum = 0.0;
            for m in 0..b_rows {
                for n in 0..b_cols {
                    let x = i + m - (b_rows / 2);
                    let y = j + n - (b_cols / 2);
                    if x < a_rows && y < a_cols {
                        sum += a[[x, y]] * b[[m, n]];
                    }
                }
            }
            result[[i, j]] = sum;
        }
    }
    result
}

/// FFT-based convolution (output same size as `a`)
pub fn convolve2d(a: &Array2<f32>, b: &Array2<f32>) -> Array2<f32> {
    let (a_rows, a_cols) = a.dim();
    let (b_rows, b_cols) = b.dim();

    // If the kernel is small, we use direct convolution
    const THRESHOLD: usize = 256;
    if b_rows * b_cols <= THRESHOLD {
        return direct_convolve2d(a, b);
    }

    let padded_rows = next_power_of_two(a_rows + b_rows - 1);
    let padded_cols = next_power_of_two(a_cols + b_cols - 1);

    let mut planner = FftPlanner::new();

    // Pad both inputs to complex arrays
    let mut a_padded = pad_array(a, (padded_rows, padded_cols));
    let mut b_padded = pad_array(b, (padded_rows, padded_cols));

    // FFT
    fft2d(&mut a_padded, &mut planner, false);
    fft2d(&mut b_padded, &mut planner, false);

    // Frequency domain multiplication
    let mut result_freq = multiply_freq_domain(&a_padded, &b_padded);

    // Inverse FFT
    fft2d(&mut result_freq, &mut planner, true);

    // Crop the central part
    let start_row = (b_rows - 1) / 2;
    let start_col = (b_cols - 1) / 2;

    // Instead of manual loop, slice
    let result_view = result_freq.slice(s![
        start_row..start_row + a_rows,
        start_col..start_col + a_cols
    ]);

    // Extract real part
    let mut result = Array2::<f32>::zeros((a_rows, a_cols));
    Zip::from(&mut result)
        .and(result_view)
        .for_each(|r, &c| *r = c.re);

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
    fn filter_scan(&self, _scan: &ScannedImage, filter: &Array1<f32>) -> Array3<f32> {
        let (rows, cols, depth) = _scan.raw_data.dim();
        let mut filtered_data = Array3::<f32>::zeros((rows, cols, depth));
    
        // Iterate over the 2D space (rows, cols)
        for j in 0..cols {
            for i in 0..rows {
                // Get the slice along the last axis (depth)
                let slice = _scan.raw_data.slice(s![i, j, ..]).to_owned();
                
                // Apply convolution and assign the result directly to filtered_data
                filtered_data.slice_mut(s![i, j, ..])
                    .assign(&convolve1d(&slice, &filter));
            }
        }
    
        filtered_data
    }

    fn richardson_lucy(
        &self,
        image: &Array2<f32>,
        psf: &Array2<f32>,
        n_iterations: usize,
    ) -> Array2<f32> {
        let psf_mirror = psf.slice(s![..;-1, ..;-1]).to_owned(); // Flip kernel

        let pad_y = psf.nrows() / 2;
        let pad_x = psf.ncols() / 2;

        let (h, w) = (image.nrows(), image.ncols());
        let padded_h = h + 2 * pad_y;
        let padded_w = w + 2 * pad_x;

        let mut padded_image = Array2::<f32>::zeros((padded_h, padded_w));

        // Center
        padded_image
            .slice_mut(s![pad_y..pad_y + h, pad_x..pad_x + w])
            .assign(image);

        // Top and Bottom reflection
        for i in 0..pad_y {
            let src_top = image.slice(s![pad_y - i, ..]);
            let src_bottom = image.slice(s![h - 2 - i, ..]);

            padded_image
                .slice_mut(s![i, pad_x..pad_x + w])
                .assign(&src_top);
            padded_image
                .slice_mut(s![pad_y + h + i, pad_x..pad_x + w])
                .assign(&src_bottom);
        }

        // Left and Right reflection
        for j in 0..pad_x {
            let src_left = padded_image.slice(s![.., pad_x + (pad_x - j)]).to_owned();
            let src_right = padded_image.slice(s![.., pad_x + w - 2 - j]).to_owned();

            padded_image.slice_mut(s![.., j]).assign(&src_left);
            padded_image
                .slice_mut(s![.., pad_x + w + j])
                .assign(&src_right);
        }

        let mut u = padded_image.clone(); // Initial guess

        let eps: f32 = 1e-12;

        for _ in 0..n_iterations {
            let ustarp = convolve2d(&u, &psf);
            let relative_blur = Zip::from(&padded_image)
                .and(&ustarp)
                .map_collect(|&d, &c| d / (c + eps));
            let correction = convolve2d(&relative_blur, &psf_mirror);
            Zip::from(&mut u).and(&correction).for_each(|e, &c| *e *= c);
        }

        // Crop result to original image size
        u.slice(s![pad_y..pad_y + h, pad_x..pad_x + w]).to_owned()
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
            n_iterations: 500
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
        if scan.dx.is_none() || scan.dy.is_none() {
            println!("No data loaded, skipping deconvolution.");
            return;
        }

        println!("Starting deconvolution filter...");
        scan.filtered_data = Array3::zeros((
            scan.raw_data.dim().0,
            scan.raw_data.dim().1,
            scan.raw_data.dim().2,
        ));

        // Pré-calcul des valeurs minimales et maximales pour éviter de les recalculer à chaque itération
        let (wx_min, wx_max) = gui_settings
            .psf
            .popt_x
            .rows()
            .into_iter()
            .filter_map(|row| row[1].is_finite().then_some(row[1]))
            .fold((f32::INFINITY, f32::NEG_INFINITY), |(min, max), val| {
                (min.min(val), max.max(val))
            });

        let (wy_min, wy_max) = gui_settings
            .psf
            .popt_y
            .rows()
            .into_iter()
            .filter_map(|row| row[1].is_finite().then_some(row[1]))
            .fold((f32::INFINITY, f32::NEG_INFINITY), |(min, max), val| {
                (min.min(val), max.max(val))
            });

        let w_min = wx_min.min(wy_min);
        let w_max = wx_max.max(wy_max);

        let start = Instant::now();
        
        let processed_data: Vec<Array3<f32>> = gui_settings
            .psf
            .filters
            .outer_iter()
            .enumerate()
            .par_bridge()
            .map(|(i, _)| {
                print!("\rProcessing frequency band {}/{}...", i + 1, gui_settings.psf.n_filters);
                io::stdout().flush().unwrap();

                // Calculating the range for the PSF
                let range_max_x = self.range_max_min(
                    (gui_settings.psf.popt_x.row(i)[1] as f32
                        + gui_settings.psf.popt_x.row(i)[0].abs() as f32)
                        * 3.0,
                    2.5,
                );
                let range_max_y = self.range_max_min(
                    (gui_settings.psf.popt_y.row(i)[1] as f32
                        + gui_settings.psf.popt_y.row(i)[0].abs() as f32)
                        * 3.0,
                    2.5,
                );

                let dx = scan.dx.unwrap() as f32;
                let dy = scan.dy.unwrap() as f32;

                let range_max_x = (range_max_x / dx).floor() * dx + dx;
                let range_max_y = (range_max_y / dy).floor() * dy + dy;

                let x: Vec<f32> = (-((range_max_x / dx).floor() as isize)
                    ..=((range_max_x / dx).floor() as isize))
                    .map(|i| i as f32 * dx)
                    .collect();
                let y: Vec<f32> = (-((range_max_y / dy).floor() as isize)
                    ..=((range_max_y / dy).floor() as isize))
                    .map(|i| i as f32 * dy)
                    .collect();

                let gaussian_x = gaussian2(
                    &arr1(&x),
                    &gui_settings.psf.popt_x.row(i).as_slice().unwrap(),
                )
                .to_vec();
                let gaussian_y = gaussian2(
                    &arr1(&y),
                    &gui_settings.psf.popt_y.row(i).as_slice().unwrap(),
                )
                .to_vec();

                let psf_2d = create_psf_2d(gaussian_x, gaussian_y, x, y, dx, dy);

                // Filtering the scan data
                let mut filtered_data =
                    self.filter_scan(&scan.clone(), &gui_settings.psf.filters.row(i).to_owned());

                // Computing the filtered image
                let filtered_image = filtered_data.mapv(|x| x * x).sum_axis(Axis(2));

                /*
                let n_iter = (((gui_settings.psf.popt_x.row(i)[1] - w_min) / (w_max - w_min)
                    * (self.n_iterations as f32 - 1.0)
                    + 1.0)
                    .floor()) as usize;
                */
                let n_iter = (((gui_settings.psf.popt_x.row(i)[1] - w_min) / (w_max - w_min)
                * (self.n_iterations as f32 - 1.0)
                + 1.0)
                .floor()) as usize;

                let deconvolved_image = self
                    .richardson_lucy(&filtered_image, &psf_2d, n_iter)
                    .mapv(|x| x.max(0.0)); // Force negative values to zero

                let mut deconvolution_gains =
                    Array2::zeros((scan.raw_data.dim().0, scan.raw_data.dim().1));
                Zip::from(&deconvolved_image)
                    .and(&filtered_image)
                    .and(&mut deconvolution_gains)
                    .for_each(|&u, &d, g| *g = (u / d).sqrt());

                // Apply the deconvolution gains to the filtered data
                filtered_data
                    .iter_mut()
                    .zip(deconvolution_gains.iter())
                    .for_each(|(data, &gain)| *data *= gain);

                filtered_data
            })
            .collect();

            let duration = start.elapsed();
            println!("\nTime elapsed for filtering: {:?}", duration);


        println!("Combining processed data...");

        let start = Instant::now();

        for data in processed_data {
            scan.filtered_data += &data;
        }

        let duration = start.elapsed();
        println!("Time elapsed: {:?}", duration);

        println!("Calculating filtered image...");

        scan.filtered_img = scan.filtered_data.mapv(|x| x * x).sum_axis(Axis(2));

        println!("Deconvolution filter completed.");
    }

    fn ui(
        &mut self,
        ui: &mut Ui,
        _thread_communication: &mut GuiThreadCommunication,
    ) -> egui::Response {
        // thread_communication can be used, but is not required. It contains the gui_settings GuiSettingsContainer
        // implement your GUI parameter handling here, for example like this:
        let mut clicked = false;
        let mut response = ui.horizontal(|ui| {
             let button_response =  ui.add(egui::Button::new("Apply"));
                if button_response.clicked() {
                    clicked = true;
                }
            button_response
            })
            .inner;
        if clicked {
            response.mark_changed();
        }
        response 
    }
}
