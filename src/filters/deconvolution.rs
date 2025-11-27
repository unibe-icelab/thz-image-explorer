//! This module implements a custom filter named `Deconvolution`, which operates on scanned images
//! and performs a deconvolution operation in the frequency domain.
//!
//! The implementation includes a Richardson-Lucy deconvolution algorithm placeholder,
//! allowing for further customization and parameterization.

use crate::config::ThreadCommunication;
use crate::data_container::ScannedImageFilterData;
use crate::filters::filter::{CopyStaticFieldsTrait, Filter, FilterConfig, FilterDomain};
use crate::filters::psf::{create_psf_2d, gaussian};
use crate::gui::application::GuiSettingsContainer;
use bevy_egui::egui::{self, Ui};
use filter_macros::{register_filter, CopyStaticFields};
use ndarray::{arr1, s, Array1, Array2, Array3, Axis, Zip};
use num_complex::Complex32;
use rayon::prelude::*;
use rustfft::{num_complex::Complex, FftPlanner};
use std::error::Error;

use cancellable_loops::par_for_each_cancellable_reduce;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, RwLock};

// ============================================================================
// Filter generation utilities (ported from thz-point-spread-function-tool)
// ============================================================================

/// Calculate Kaiser window attenuation
fn kaiser_atten(ntaps: usize, width_ratio: f64) -> f64 {
    let a = 2.285 * (ntaps as f64 - 1.0) * std::f64::consts::PI * width_ratio + 7.95;
    a.max(0.0)
}

/// Calculate Kaiser window beta parameter
fn kaiser_beta(atten: f64) -> f64 {
    if atten > 50.0 {
        0.1102 * (atten - 8.7)
    } else if atten >= 21.0 {
        0.5842 * (atten - 21.0).powf(0.4) + 0.07886 * (atten - 21.0)
    } else {
        0.0
    }
}

/// Modified Bessel function of the first kind, order 0
fn i0(x: f64) -> f64 {
    let mut sum = 1.0;
    let mut term = 1.0;
    let x_half_sq = (x / 2.0).powi(2);

    for k in 1..50 {
        term *= x_half_sq / ((k * k) as f64);
        sum += term;
        if term < 1e-12 * sum {
            break;
        }
    }
    sum
}

/// Sinc function sin(x)/x
fn sinc(x: f64) -> f64 {
    if x.abs() < 1e-10 {
        1.0
    } else {
        x.sin() / x
    }
}

/// Create a Kaiser window coefficient
fn kaiser_window_coeff(n: usize, n_taps: usize, beta: f64) -> f64 {
    if n == 0 || n == n_taps - 1 {
        0.0
    } else {
        let arg = 2.0 * n as f64 / (n_taps as f64 - 1.0) - 1.0;
        let num = i0(beta * (1.0 - arg * arg).sqrt());
        let denom = i0(beta);
        num / denom
    }
}

/// FIR low-pass design using time-domain Kaiser-windowed sinc
fn firwin_kaiser_lowpass(n_taps: usize, cutoff_hz: f64, beta: f64, sampling_freq: f64) -> Vec<f64> {
    let adjusted_n_taps = if n_taps % 2 == 0 { n_taps - 1 } else { n_taps };
    let mid = (adjusted_n_taps - 1) as f64 / 2.0;
    let cutoff = cutoff_hz / sampling_freq;

    let mut filter: Vec<f64> = (0..adjusted_n_taps)
        .map(|n| {
            let sinc_val = sinc(2.0 * std::f64::consts::PI * cutoff * (n as f64 - mid));
            let window_val = kaiser_window_coeff(n, adjusted_n_taps, beta);
            sinc_val * window_val
        })
        .collect();

    // Normalize filter for unitary gain at DC
    let sum_filter: f64 = filter.iter().sum();
    if sum_filter.abs() > 1e-10 {
        filter.iter_mut().for_each(|x| *x /= sum_filter);
    }

    if n_taps % 2 == 0 {
        filter.push(0.0);
    }

    filter
}

/// High-pass FIR design by spectral inversion
fn firwin_kaiser_highpass(
    n_taps: usize,
    cutoff_hz: f64,
    beta: f64,
    sampling_freq: f64,
) -> Vec<f64> {
    let adjusted_n_taps = if n_taps % 2 == 0 { n_taps - 1 } else { n_taps };
    let mid = (adjusted_n_taps - 1) as f64 / 2.0;
    let mut filter = firwin_kaiser_lowpass(adjusted_n_taps, cutoff_hz, beta, sampling_freq);

    // Spectral inversion: h_hp(n) = δ(n) - h_lp(n)
    filter.iter_mut().enumerate().for_each(|(i, h)| {
        *h = if i == mid as usize { 1.0 - *h } else { -*h };
    });

    if n_taps % 2 == 0 {
        filter.push(0.0);
    }

    filter
}

/// Design a Kaiser-windowed FIR bandpass filter
fn bandpass_kaiser(ntaps: usize, lowcut: f64, highcut: f64, fs: f64, width: f64) -> Vec<f64> {
    let width_ratio = width / (0.5 * fs);
    let atten = kaiser_atten(ntaps, width_ratio);
    let beta = kaiser_beta(atten);

    // Determine filter type and cutoffs
    if lowcut <= 0.0 {
        // Lowpass
        firwin_kaiser_lowpass(ntaps, highcut, beta, fs)
    } else if highcut >= 0.5 * fs {
        // Highpass
        firwin_kaiser_highpass(ntaps, lowcut, beta, fs)
    } else {
        // Bandpass: highpass(lowcut) - highpass(highcut)
        let h_low = firwin_kaiser_highpass(ntaps, lowcut, beta, fs);
        let h_high = firwin_kaiser_highpass(ntaps, highcut, beta, fs);
        h_low
            .iter()
            .zip(h_high.iter())
            .map(|(l, h)| l - h)
            .collect()
    }
}

/// Create logarithmically spaced bandpass filters
/// First filter is lowpass, last filter is highpass, middle filters are bandpass
fn create_filter_bank(
    n_filters: usize,
    start_freq: f64,
    end_freq: f64,
    win_width: f64,
    time_array: &Array1<f32>,
) -> (Array2<f32>, Vec<f32>) {
    let ntaps = 499;

    // Calculate sampling frequency
    let dt = (time_array[1] - time_array[0]) as f64;
    let fs = 1.0 / dt; // in THz

    // Logarithmically spaced center frequencies
    let log_start = start_freq.ln();
    let log_end = end_freq.ln();
    let log_step = (log_end - log_start) / ((n_filters - 1) as f64);

    let center_frequencies: Vec<f32> = (0..n_filters)
        .map(|i| (log_start + i as f64 * log_step).exp() as f32)
        .collect();

    // Create filters
    let mut filters = Array2::zeros((n_filters, ntaps));

    for (i, &center_freq) in center_frequencies.iter().enumerate() {
        let center_freq_f64 = center_freq as f64;

        // Calculate lowcut and highcut for this filter
        let lowcut = if i == 0 {
            0.0  // First filter is lowpass
        } else {
            ((center_frequencies[i - 1] as f64) * center_freq_f64).sqrt()
        };

        let highcut = if i == n_filters - 1 {
            0.5 * fs  // Last filter is highpass (Nyquist frequency)
        } else {
            (center_freq_f64 * (center_frequencies[i + 1] as f64)).sqrt()
        };

        // Design the filter
        let filter_coeffs = bandpass_kaiser(ntaps, lowcut, highcut, fs, win_width);

        // Store in array
        for (j, &coeff) in filter_coeffs.iter().enumerate() {
            filters[[i, j]] = coeff as f32;
        }
    }

    (filters, center_frequencies)
}

// ============================================================================
// Deconvolution filter structure
// ============================================================================

/// Represents a `Deconvolution` filter.
///
/// This filter is designed to perform deconvolution using a configurable number of iterations
/// and a defined frequency range. It is implemented to work in the frequency domain.
///
/// Fields:
/// - `n_iterations`: The number of iterations for performing the deconvolution.
/// - `n_filters`: The number of logarithmically-spaced filters to create (first is lowpass, last is highpass).
/// - `start_freq`: First center frequency for the filter bank (THz).
/// - `end_freq`: Last center frequency for the filter bank (THz).
/// - `win_width`: Kaiser window width parameter for filter design (THz).
#[register_filter]
#[derive(Clone, Debug, CopyStaticFields)]
/// Represents the Deconvolution filter configuration.
///
/// # Fields
/// - `n_iterations` (*usize*): The number of iterations for the deconvolution algorithm.
/// - `n_filters` (*usize*): The number of logarithmically-spaced filters (first is lowpass, last is highpass).
/// - `start_freq` (*f32*): First center frequency (THz).
/// - `end_freq` (*f32*): Last center frequency (THz).
/// - `win_width` (*f32*): Kaiser window width (THz).
pub struct Deconvolution {
    // Number of iterations for the deconvolution algorithm
    pub n_iterations: usize,
    // Number of logarithmically-spaced filters
    pub n_filters: usize,
    // First center frequency (THz)
    pub start_freq: f32,
    // Last center frequency (THz)
    pub end_freq: f32,
    // Kaiser window width (THz)
    pub win_width: f32,
}

/// Performs 1D convolution using FFT.
///
/// # Arguments
/// - `a` (*&Array1<f32>*): The first input array.
/// - `b` (*&Array1<f32>*): The second input array (kernel).
/// - `fft` (*&dyn rustfft::Fft<f64>*): The FFT implementation.
/// - `ifft` (*&dyn rustfft::Fft<f64>*): The inverse FFT implementation.
/// - `fft_size` (*usize*): The size of the FFT to be performed.
///
/// # Returns
/// - (*Array1<f32>*): The result of the convolution.
pub fn convolve1d(
    a: &Array1<f32>,
    b: &Array1<f32>,
    fft: &dyn rustfft::Fft<f64>,
    ifft: &dyn rustfft::Fft<f64>,
    fft_size: usize,
) -> Array1<f32> {
    // Pad input signals to the FFT size
    // Initialize padded arrays for the input signals with zero values
    let mut a_padded: Vec<Complex<f64>> = vec![Complex { re: 0.0, im: 0.0 }; fft_size];
    let mut b_padded: Vec<Complex<f64>> = vec![Complex { re: 0.0, im: 0.0 }; fft_size];

    // Calculate the shift length for aligning the convolution result
    let shift_len = (b.len() - 1) / 2;

    // Copy input data into the padded arrays
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

    // Perform FFT on both input signals
    fft.process(&mut a_padded);
    fft.process(&mut b_padded);

    // Perform pointwise multiplication in the frequency domain
    let mut result_freq: Vec<Complex<f64>> = a_padded
        .iter()
        .zip(b_padded.iter())
        .map(|(x, y)| x * y)
        .collect();

    // Perform inverse FFT to transform the result back to the time domain
    ifft.process(&mut result_freq);

    // Normalize the result by dividing by the FFT size and extract the real part
    Array1::from(
        result_freq[shift_len..a.len() + shift_len]
            .iter()
            .map(|c| (c.re / fft_size as f64) as f32) // Normalize by FFT size and cast to f32
            .collect::<Vec<f32>>(),
    )
}

/// Perform element-wise multiplication of two complex matrices
/// Performs element-wise multiplication of two complex matrices in the frequency domain.
///
/// # Arguments
/// - `a` (*&Array2<Complex<f32>>*): The first complex matrix.
/// - `b` (*&Array2<Complex<f32>>*): The second complex matrix.
///
/// # Returns
/// - (*Array2<Complex<f32>>*): The result of the element-wise multiplication.
fn multiply_freq_domain(
    a: &Array2<Complex<f32>>,
    b: &Array2<Complex<f32>>,
) -> Array2<Complex<f32>> {
    // Clone the first matrix to store the result
    let mut result = a.clone();

    // Perform element-wise multiplication of the two matrices
    Zip::from(&mut result)
        .and(b)
        .for_each(|r, &bval| *r *= bval);

    result
}

/// Pads a 2D array with zeros to a specified shape.
///
/// # Arguments
/// - `input` (*&Array2<f32>*): The input array to be padded.
/// - `padded_shape` (*tuple(usize, usize)*): The desired shape of the padded array.
///
/// # Returns
/// - (*Array2<Complex32>*): The padded array with complex values.
pub fn pad_array(input: &Array2<f32>, padded_shape: (usize, usize)) -> Array2<Complex32> {
    let (input_rows, input_cols) = input.dim();
    let (padded_rows, padded_cols) = padded_shape;

    // Ensure the padded dimensions are larger than or equal to the input dimensions
    assert!(padded_rows >= input_rows && padded_cols >= input_cols);

    // Initialize the output array with zeros
    let mut output = Array2::<Complex32>::zeros((padded_rows, padded_cols));

    // Copy the input array into the top-left corner of the output array
    // Convert each value from f32 to Complex32
    for y in 0..input_rows {
        for x in 0..input_cols {
            output[[y, x]] = Complex32::new(input[[y, x]], 0.0);
        }
    }

    // Return the padded array
    output
}

/// Perform 2D FFT (in-place) on a matrix
/// Performs 2D FFT or inverse FFT on a matrix (in-place).
///
/// # Arguments
/// - `matrix` (*&mut Array2<Complex<f32>>*): The matrix to be transformed.
/// - `fft_cols` (*&dyn rustfft::Fft<f32>*): The FFT implementation for columns.
/// - `fft_rows` (*&dyn rustfft::Fft<f32>*): The FFT implementation for rows.
/// - `inverse` (*bool*): Whether to perform an inverse FFT.
///
/// # Notes
/// - Normalizes the result if `inverse` is true.
fn fft2d(
    matrix: &mut Array2<Complex<f32>>,
    fft_cols: &dyn rustfft::Fft<f32>,
    fft_rows: &dyn rustfft::Fft<f32>,
    inverse: bool,
) -> Result<(), Box<dyn Error>> {
    let (rows, cols) = matrix.dim();

    // FFT on rows
    // Perform FFT on each row of the matrix
    for mut row in matrix.outer_iter_mut() {
        match row.as_slice_mut() {
            Some(slice) => {
                fft_cols.process(slice);
            }
            None => {
                return Err("Row is not contiguous, cannot process FFT".into());
            }
        }
    }

    // Perform FFT on each column of the matrix
    for x in 0..cols {
        // Extract the column as a vector
        let mut column: Vec<_> = (0..rows).map(|y| matrix[[y, x]]).collect();

        // Apply FFT to the column
        fft_rows.process(&mut column);

        // Write the transformed column back into the matrix
        for (y, val) in column.iter().enumerate() {
            matrix[[y, x]] = *val;
        }
    }

    // If performing an inverse FFT, normalize the matrix by dividing by the total number of elements
    if inverse {
        let scale = (rows * cols) as f32;
        matrix.mapv_inplace(|v| v / scale);
    }
    Ok(())
}

/// Direct 2D Convolution for small kernels
/// Performs direct 2D convolution for small kernels.
///
/// # Arguments
/// - `a` (*&Array2<f32>*): The input array.
/// - `b` (*&Array2<f32>*): The kernel array.
///
/// # Returns
/// - (*Array2<f32>*): The result of the convolution.
fn direct_convolve2d(a: &Array2<f32>, b: &Array2<f32>) -> Array2<f32> {
    let (a_rows, a_cols) = a.dim();
    let (b_rows, b_cols) = b.dim();

    let mut result = Array2::<f32>::zeros((a_rows, a_cols));

    let half_b_rows = b_rows / 2;
    let half_b_cols = b_cols / 2;

    for i in 0..a_rows {
        for j in 0..a_cols {
            let mut sum = 0.0;
            for m in 0..b_rows {
                for n in 0..b_cols {
                    let x = i as isize + m as isize - half_b_rows as isize;
                    let y = j as isize + n as isize - half_b_cols as isize;

                    if x >= 0 && y >= 0 && (x as usize) < a_rows && (y as usize) < a_cols {
                        sum += a[[x as usize, y as usize]] * b[[m, n]];
                    }
                }
            }
            result[[i, j]] = sum;
        }
    }
    result
}

/// FFT-based convolution (output same size as `a`)
/// Performs 2D convolution using FFT (output same size as `a`).
///
/// # Arguments
/// - `a` (*&Array2<f32>*): The input array.
/// - `b` (*&Array2<f32>*): The kernel array.
/// - `fft_cols` (*&dyn rustfft::Fft<f32>*): The FFT implementation for columns.
/// - `ifft_cols` (*&dyn rustfft::Fft<f32>*): The inverse FFT implementation for columns.
/// - `fft_rows` (*&dyn rustfft::Fft<f32>*): The FFT implementation for rows.
/// - `ifft_rows` (*&dyn rustfft::Fft<f32>*): The inverse FFT implementation for rows.
///
/// # Returns
/// - (*Array2<f32>*): The result of the convolution.
pub fn convolve2d(
    a: &Array2<f32>,
    b: &Array2<f32>,
    fft_cols: &dyn rustfft::Fft<f32>,
    ifft_cols: &dyn rustfft::Fft<f32>,
    fft_rows: &dyn rustfft::Fft<f32>,
    ifft_rows: &dyn rustfft::Fft<f32>,
) -> Result<Array2<f32>, Box<dyn std::error::Error>> {
    let (a_rows, a_cols) = a.dim();
    let (b_rows, b_cols) = b.dim();

    // If the kernel is small, we use direct convolution for efficiency
    const THRESHOLD: usize = 256;
    if b_rows * b_cols <= THRESHOLD {
        return Ok(direct_convolve2d(a, b));
    }

    // Calculate padding to ensure output size matches input
    let padded_rows = (a_rows + b_rows - 1).next_power_of_two();
    let padded_cols = (a_cols + b_cols - 1).next_power_of_two();

    // Pad both input arrays to the calculated dimensions
    let mut a_padded = pad_array(a, (padded_rows, padded_cols));
    let mut b_padded = pad_array(b, (padded_rows, padded_cols));

    // Perform FFT on both padded arrays
    if let Err(e) = fft2d(&mut a_padded, &*fft_cols, &*fft_rows, false) {
        log::error!("fft2d on a_padded failed: {}", e);
        return Err(e);
    }
    if let Err(e) = fft2d(&mut b_padded, &*fft_cols, &*fft_rows, false) {
        log::error!("fft2d on b_padded failed: {}", e);
        return Err(e);
    }

    // Multiply the two arrays in the frequency domain
    let mut result_freq = multiply_freq_domain(&a_padded, &b_padded);

    // Perform inverse FFT to transform back to the spatial domain
    if let Err(e) = fft2d(&mut result_freq, &*ifft_cols, &*ifft_rows, true) {
        log::error!("fft2d on result_freq failed: {}", e);
        return Err(e);
    }

    // For "same" size convolution, we need to extract the center part
    let start_row = (b_rows - 1) / 2;
    let start_col = (b_cols - 1) / 2;

    // Ensure we have enough data to extract the required output size
    let available_rows = result_freq.nrows();
    let available_cols = result_freq.ncols();

    if start_row + a_rows > available_rows || start_col + a_cols > available_cols {
        log::error!("Insufficient data after convolution: need {}x{} starting at ({}, {}), but only have {}x{}",
                   a_rows, a_cols, start_row, start_col, available_rows, available_cols);

        // Fallback: use direct convolution
        return Ok(direct_convolve2d(a, b));
    }

    // Extract the result with the same size as input 'a'
    let result_view = result_freq.slice(s![
        start_row..start_row + a_rows,
        start_col..start_col + a_cols
    ]);

    // Extract the real part of the complex result
    let mut result = Array2::<f32>::zeros((a_rows, a_cols));
    Zip::from(&mut result)
        .and(result_view)
        .for_each(|r, &c| *r = c.re);

    Ok(result)
}

impl Deconvolution {
    /// Computes the minimum maximum range for the deconvolution algorithm.
    /// Ensures that the range_max value is not smaller than the minimum allowable range (wmin).
    ///
    /// # Arguments
    /// - `range_max` (*f32*): The maximum range value.
    /// - `wmin` (*f32*): The minimum allowable range.
    ///
    /// # Returns
    /// - (*f32*): The adjusted maximum range.
    fn range_max_min(&self, range_max: f32, wmin: f32) -> f32 {
        // If range_max is smaller than wmin, return wmin; otherwise, return range_max
        if range_max < wmin {
            wmin
        } else {
            range_max
        }
    }

    /// Computes the filtered scan with the FIR filter by convolving each time trace with the filter.
    /// Applies a filter to a scanned image using convolution.
    ///
    /// # Arguments
    /// - `_scan` (*&ScannedImageFilterData*): The scanned image data to be filtered.
    /// - `filter` (*&Array1<f32>*): The filter to apply.
    ///
    /// # Returns
    /// - (*Array3<f32>*): The filtered image data.
    fn filter_scan(&self, _scan: &ScannedImageFilterData, filter: &Array1<f32>) -> Array3<f32> {
        let (rows, cols, depth) = _scan.data.dim();
        let mut filtered_data = Array3::<f32>::zeros((rows, cols, depth));

        // Calculate the convolution size and round it up to the next power of two for FFT efficiency
        let conv_size = _scan.data.slice(s![0, 0, ..]).len() + filter.len() - 1;
        let fft_size = conv_size.next_power_of_two();

        // Create FFT and inverse FFT plans
        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(fft_size);
        let ifft = planner.plan_fft_inverse(fft_size);

        // Filter each time trace in the scan
        filtered_data
            .axis_iter_mut(Axis(0)) // Iterate over the first axis (rows)
            .into_iter()
            .enumerate()
            .for_each(|(i, mut row)| {
                row.axis_iter_mut(Axis(0)) // Iterate over the second axis (columns)
                    .enumerate()
                    .for_each(|(j, mut slice)| {
                        // Extract the time trace for the current row and column
                        let input_slice = _scan.data.slice(s![i, j, ..]);
                        // Perform 1D convolution and assign the result to the slice
                        slice.assign(&convolve1d(
                            &input_slice.to_owned(),
                            &filter,
                            &*fft,
                            &*ifft,
                            fft_size,
                        ));
                    });
            });
        filtered_data
    }

    /// Performs Richardson-Lucy deconvolution on an image.
    ///
    /// # Arguments
    /// - `image` (*&Array2<f32>*): The input image to be deconvolved.
    /// - `psf` (*&Array2<f32>*): The point spread function (PSF).
    /// - `n_iterations` (*usize*): The number of iterations to perform.
    ///
    /// # Returns
    /// - (*Array2<f32>*): The deconvolved image.
    fn richardson_lucy(
        &self,
        image: &Array2<f32>,
        psf: &Array2<f32>,
        n_iterations: usize,
    ) -> Result<Array2<f32>, Box<dyn std::error::Error>> {
        // Flip the PSF kernel to create its mirrored version
        let psf_mirror = psf.slice(s![..;-1, ..;-1]).to_owned();

        // Calculate padding sizes for rows and columns
        let pad_y = psf.nrows() / 2;
        let pad_x = psf.ncols() / 2;

        let (h, w) = (image.nrows(), image.ncols());
        let padded_h = h + 2 * pad_y;
        let padded_w = w + 2 * pad_x;

        // Initialize the padded image with zeros
        let mut padded_image = Array2::<f32>::zeros((padded_h, padded_w));

        // Copy the input image into the center of the padded image
        padded_image
            .slice_mut(s![pad_y..pad_y + h, pad_x..pad_x + w])
            .assign(image);

        // Reflect the top and bottom edges of the image into the padding
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

        // Reflect the left and right edges of the image into the padding
        for j in 0..pad_x {
            let src_left = padded_image.slice(s![.., pad_x + (pad_x - j)]).to_owned();
            let src_right = padded_image.slice(s![.., pad_x + w - 2 - j]).to_owned();

            padded_image.slice_mut(s![.., j]).assign(&src_left);
            padded_image
                .slice_mut(s![.., pad_x + w + j])
                .assign(&src_right);
        }

        // Initialize the deconvolved image with the padded image as the initial guess
        let mut u = padded_image.clone();

        let eps: f32 = 1e-12; // Small constant to avoid division by zero

        // Calculate the next power of two for FFT dimensions
        let (n_rows, n_cols) = padded_image.dim();
        let n_rows = n_rows.next_power_of_two();
        let n_cols = n_cols.next_power_of_two();

        // Create FFT and inverse FFT plans
        let mut planner = FftPlanner::new();
        let fft_cols: Arc<dyn rustfft::Fft<f32>> = planner.plan_fft_forward(n_cols);
        let ifft_cols: Arc<dyn rustfft::Fft<f32>> = planner.plan_fft_inverse(n_cols);
        let fft_rows: Arc<dyn rustfft::Fft<f32>> = planner.plan_fft_forward(n_rows);
        let ifft_rows: Arc<dyn rustfft::Fft<f32>> = planner.plan_fft_inverse(n_rows);

        // Perform Richardson-Lucy iterations
        for _ in 0..n_iterations {
            // Convolve the current estimate with the PSF
            let ustarp = convolve2d(&u, psf, &*fft_cols, &*ifft_cols, &*fft_rows, &*ifft_rows)?;

            // Compute the relative blur
            let relative_blur = Zip::from(&padded_image)
                .and(&ustarp)
                .map_collect(|&d, &c| d / (c + eps));

            // Convolve the relative blur with the mirrored PSF
            let correction = convolve2d(
                &relative_blur,
                &psf_mirror,
                &*fft_cols,
                &*ifft_cols,
                &*fft_rows,
                &*ifft_rows,
            )?;

            // Update the estimate by multiplying with the correction
            Zip::from(&mut u).and(&correction).for_each(|e, &c| *e *= c);
        }

        // Crop the deconvolved image to the original size
        Ok(u.slice(s![pad_y..pad_y + h, pad_x..pad_x + w]).to_owned())
    }
}

impl Filter for Deconvolution {
    /// Creates a new `Deconvolution` filter with default settings.
    ///
    /// Default values:
    /// - `n_iterations`: 500
    /// - `n_filters`: 20
    /// - `start_freq`: 0.25 THz
    /// - `end_freq`: 4.0 THz
    /// - `win_width`: 0.5 THz
    fn new() -> Self {
        Deconvolution {
            n_iterations: 500,
            n_filters: 20,
            start_freq: 0.25,
            end_freq: 4.0,
            win_width: 0.5,
        }
    }

    /// Resets the filter state. Not used.
    fn reset(&mut self, _time: &Array1<f32>, _shape: &[usize]) {
        // NOOP
    }

    /// Displays the filter settings in the GUI. Not used, no data to show.
    fn show_data(&mut self, _data: &ScannedImageFilterData) {
        // NOOP
    }

    fn config(&self) -> FilterConfig {
        FilterConfig {
            name: "Deconvolution".to_string(),
            description: "Frequency-dependent deconvolution for enhanced THz-TDS scans, accounting for beam width variations in time traces.\n\nCitation: A. Demion, L. L. Stöckli, N. Thomas and S. Zahno, \"Frequency-Dependent Deconvolution for Enhanced THz-TDS Scans: Accounting for Beam Width Variations in Time Traces,\" in IEEE Transactions on Terahertz Science and Technology, vol. 15, no. 3, pp. 505-513, May 2025".to_string(),
            hyperlink: Some((Some("TTHZ.2025.3546756".to_string()),"https://doi.org/10.1109/TTHZ.2025.3546756".to_string())),
            domain: FilterDomain::TimeAfterFFTPrioLast,
        }
    }

    /// Applies the deconvolution algorithm to a scanned image.
    ///
    /// # Arguments:
    /// - `input_data`: Reference to the scanned image to be processed.
    /// - `gui_settings`: Mutable reference to the GUI settings associated with the filter.
    /// - `progress_lock`: Progress indicator for the operation.
    /// - `abort_flag`: Flag to cancel the operation.
    ///
    /// # Notes:
    /// This method generates a filter bank on-the-fly based on the filter parameters,
    /// evaluates PSF parameters using cubic splines, and performs Richardson-Lucy deconvolution.
    fn filter(
        &mut self,
        input_data: &ScannedImageFilterData,
        gui_settings: &mut GuiSettingsContainer,
        progress_lock: &mut Arc<RwLock<Option<f32>>>,
        abort_flag: &Arc<AtomicBool>,
    ) -> ScannedImageFilterData {
        // Initialize the progress lock to indicate the start of the filtering process
        if let Ok(mut p) = progress_lock.write() {
            *p = Some(0.0);
        }

        let mut output_data = input_data.clone();

        // Check if the input data contains valid spatial resolution (dx, dy)
        if input_data.dx.is_none() || input_data.dy.is_none() {
            log::error!("No data loaded, skipping deconvolution.");
            if let Ok(mut p) = progress_lock.write() {
                *p = None;
            }
            return input_data.clone();
        }

        // Check if PSF splines have been loaded
        if gui_settings.psf.wx_spline.knots.is_empty() {
            log::error!("PSF splines appear empty — a PSF may not have been loaded. Skipping deconvolution.");
            if let Ok(mut p) = progress_lock.write() {
                *p = None;
            }
            return input_data.clone();
        }

        // Get image dimensions for bounds checking
        let (img_rows, img_cols, _) = input_data.data.dim();

        // Check minimum image size for meaningful deconvolution
        const MIN_IMAGE_SIZE: usize = 16;
        if img_rows < MIN_IMAGE_SIZE || img_cols < MIN_IMAGE_SIZE {
            log::warn!(
                "Image dimensions ({}x{}) are too small for deconvolution (minimum {}x{}). Skipping.",
                img_rows, img_cols, MIN_IMAGE_SIZE, MIN_IMAGE_SIZE
            );
            if let Ok(mut p) = progress_lock.write() {
                *p = None;
            }
            return input_data.clone();
        }

        // Log the start of the deconvolution process
        log::info!(
            "Starting deconvolution filter with {} filters...",
            self.n_filters
        );

        // Generate filter bank
        let (filters, center_frequencies) = create_filter_bank(
            self.n_filters,
            self.start_freq as f64,
            self.end_freq as f64,
            self.win_width as f64,
            &input_data.time,
        );

        log::info!(
            "Generated {} filters with center frequencies from {:.3} to {:.3} THz",
            self.n_filters,
            center_frequencies[0],
            center_frequencies[center_frequencies.len() - 1]
        );

        // Initialize output data array
        output_data.data = Array3::zeros((
            output_data.data.dim().0,
            output_data.data.dim().1,
            output_data.data.dim().2,
        ));

        // Evaluate beam widths at all filter frequencies to find min/max
        let wx_values: Vec<f32> = center_frequencies
            .iter()
            .map(|&freq| gui_settings.psf.wx_spline.eval_single(freq))
            .collect();
        let wy_values: Vec<f32> = center_frequencies
            .iter()
            .map(|&freq| gui_settings.psf.wy_spline.eval_single(freq))
            .collect();

        let wx_min = wx_values.iter().cloned().fold(f32::INFINITY, f32::min);
        let wx_max = wx_values.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let wy_min = wy_values.iter().cloned().fold(f32::INFINITY, f32::min);
        let wy_max = wy_values.iter().cloned().fold(f32::NEG_INFINITY, f32::max);

        let w_min = wx_min.min(wy_min);
        let w_max = wx_max.max(wy_max);

        let (dx, dy) = match (input_data.dx, input_data.dy) {
            (Some(dx_val), Some(dy_val)) => (dx_val as f32, dy_val as f32),
            _ => {
                log::error!("dx or dy is missing in input_data");
                if let Ok(mut p) = progress_lock.write() {
                    *p = None;
                }
                return input_data.clone();
            }
        };

        // Check if maximum PSF dimensions would be too large for the image
        let max_psf_width_x = ((wx_max / dx).ceil() as usize * 2 + 1).max(3);
        let max_psf_width_y = ((wy_max / dy).ceil() as usize * 2 + 1).max(3);

        if max_psf_width_x >= img_cols || max_psf_width_y >= img_rows {
            log::warn!(
                "Maximum PSF dimensions ({}x{}) are too large for image dimensions ({}x{}). Skipping deconvolution.",
                max_psf_width_x, max_psf_width_y, img_rows, img_cols
            );
            if let Ok(mut p) = progress_lock.write() {
                *p = None;
            }
            return input_data.clone();
        }

        // Error flag to track if any errors are encountered
        let error_flag = Arc::new(AtomicBool::new(false));
        let error_flag_clone = Arc::clone(&error_flag);

        let processed_data = par_for_each_cancellable_reduce(
            filters.outer_iter().enumerate().par_bridge(),
            &abort_flag,
            |(i, filter_coeffs)| {
                let error_flag = Arc::clone(&error_flag_clone);
                if let Ok(mut p) = progress_lock.write() {
                    if if let Some(p_old) = *p {
                        p_old < (i as f32) / (self.n_filters as f32)
                    } else {
                        false
                    } {
                        *p = Some((i as f32) / (self.n_filters as f32));
                    }
                }

                // Evaluate PSF parameters at this filter's center frequency
                let center_freq = center_frequencies[i];
                let wx = gui_settings.psf.wx_spline.eval_single(center_freq);
                let wy = gui_settings.psf.wy_spline.eval_single(center_freq);
                let x0 = gui_settings.psf.x0_spline.eval_single(center_freq);
                let y0 = gui_settings.psf.y0_spline.eval_single(center_freq);

                // Calculating the range for the PSF
                let range_max_x = self.range_max_min((wx + x0.abs()) * 3.0, 2.5);
                let range_max_y = self.range_max_min((wy + y0.abs()) * 3.0, 2.5);

                let range_max_x = (range_max_x / dx).floor() * dx + dx;
                let range_max_y = (range_max_y / dy).floor() * dy + dy;

                // Constrain PSF size to be smaller than image dimensions
                let max_allowed_x = (img_cols as f32 - 2.0) * dx / 2.0;
                let max_allowed_y = (img_rows as f32 - 2.0) * dy / 2.0;
                let constrained_range_x = range_max_x.min(max_allowed_x);
                let constrained_range_y = range_max_y.min(max_allowed_y);

                if constrained_range_x != range_max_x || constrained_range_y != range_max_y {
                    log::warn!(
                        "PSF range constrained from ({:.2}, {:.2}) to ({:.2}, {:.2}) for filter {} at {:.3} THz",
                        range_max_x,
                        range_max_y,
                        constrained_range_x,
                        constrained_range_y,
                        i,
                        center_freq
                    );
                }

                let x: Vec<f32> = (-((constrained_range_x / dx).floor() as isize)
                    ..=((constrained_range_x / dx).floor() as isize))
                    .map(|i| i as f32 * dx)
                    .collect();
                let y: Vec<f32> = (-((constrained_range_y / dy).floor() as isize)
                    ..=((constrained_range_y / dy).floor() as isize))
                    .map(|i| i as f32 * dy)
                    .collect();

                // Create Gaussian parameters: [center, width]
                let popt_x = &[x0, wx];
                let popt_y = &[y0, wy];

                let gaussian_x = gaussian(&arr1(&x), popt_x).to_vec();
                let gaussian_y = gaussian(&arr1(&y), popt_y).to_vec();

                let psf_2d = create_psf_2d(gaussian_x, gaussian_y, x, y, dx, dy);

                // Filter scan data
                let mut filtered_data = self.filter_scan(&input_data, &filter_coeffs.to_owned());

                // Compute filtered image
                let filtered_image = filtered_data.mapv(|x| x * x).sum_axis(Axis(2));

                // Calculate number of iterations based on beam width
                let n_iter = (((wx - w_min) / (w_max - w_min) * (self.n_iterations as f32 - 1.0)
                    + 1.0)
                    .floor()) as usize;

                let deconvolved_image = match self.richardson_lucy(&filtered_image, &psf_2d, n_iter)
                {
                    Ok(image) => image.mapv(|x| x.max(0.0)),
                    Err(e) => {
                        log::error!(
                            "Richardson-Lucy deconvolution failed for filter {i} at {:.3} THz: {e}",
                            center_freq
                        );
                        error_flag.store(true, std::sync::atomic::Ordering::Relaxed);
                        return None;
                    }
                };

                let mut deconvolution_gains =
                    Array2::zeros((output_data.data.dim().0, output_data.data.dim().1));

                // Compute deconvolution gains
                Zip::from(&deconvolved_image)
                    .and(&filtered_image)
                    .and(&mut deconvolution_gains)
                    .for_each(|&u, &d, g| *g = (u / d).sqrt());

                // Apply deconvolution gains in-place
                let shape = filtered_data.dim();
                for i in 0..shape.0 {
                    for j in 0..shape.1 {
                        let gain = deconvolution_gains[[i, j]];
                        filtered_data
                            .slice_mut(s![i, j, ..])
                            .mapv_inplace(|x| x * gain);
                    }
                }

                Some(filtered_data)
            },
            |mut acc, data| {
                acc += &data;
                acc
            },
            Array3::zeros(input_data.data.dim()), // initial accumulator
        );

        // Check if any errors occurred during processing
        if error_flag.load(std::sync::atomic::Ordering::Relaxed) {
            log::error!(
                "Deconvolution failed due to non-contiguous data rows. Returning original data."
            );
            if let Ok(mut p) = progress_lock.write() {
                *p = None;
            }
            return input_data.clone();
        }
        // Update the progress lock to indicate the completion of the filtering process
        if let Ok(mut p) = progress_lock.write() {
            *p = Some(1.0);
        }

        output_data.data = processed_data;
        output_data.img = output_data.data.mapv(|x| x * x).sum_axis(Axis(2));

        log::info!("\nDeconvolution filter completed.");

        // Reset the progress lock to indicate that the process has finished
        if let Ok(mut p) = progress_lock.write() {
            *p = None;
        }

        output_data
    }

    fn ui(
        &mut self,
        ui: &mut Ui,
        _thread_communication: &mut ThreadCommunication,
        _panel_width: f32,
    ) -> egui::Response {
        let mut clicked = false;

        let mut response = ui
            .vertical(|ui| {
                ui.label("Filter Bank Parameters:");

                ui.horizontal(|ui| {
                    ui.label("Number of filters:");
                    ui.add(egui::Slider::new(&mut self.n_filters, 10..=200));
                });

                ui.horizontal(|ui| {
                    ui.label("Start frequency (THz):");
                    ui.add(
                        egui::DragValue::new(&mut self.start_freq)
                            .speed(0.01)
                            .range(0.1..=2.0),
                    );
                });

                ui.horizontal(|ui| {
                    ui.label("End frequency (THz):");
                    ui.add(
                        egui::DragValue::new(&mut self.end_freq)
                            .speed(0.1)
                            .range(2.0..=10.0),
                    );
                });

                ui.horizontal(|ui| {
                    ui.label("Transition width (THz):");
                    ui.add(
                        egui::DragValue::new(&mut self.win_width)
                            .speed(0.01)
                            .range(0.1..=2.0),
                    );
                });

                ui.separator();

                let button_response = ui.add(egui::Button::new("Apply"));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::load_psf;
    use ndarray::{Array1, Array3};
    use std::path::Path;
    use std::sync::atomic::AtomicBool;
    use std::sync::{Arc, RwLock};

    #[test]
    fn test_shape_preservation() {
        let n = 64usize;
        let dt = 0.05f32;
        let width = 2usize;
        let height = 2usize;
        let impulse_idx = 12usize;

        let mut data = Array3::<f32>::zeros((width, height, n));
        data[[1, 1, impulse_idx]] = 1.0;

        let time = Array1::linspace(0.0, dt * (n as f32 - 1.0), n);

        let mut input = ScannedImageFilterData::default();
        input.time = time;
        input.data = data;

        input.dx = Some(1.0);
        input.dy = Some(1.0);

        let mut filter = Deconvolution {
            n_iterations: 10,
            n_filters: 20,
            start_freq: 0.25,
            end_freq: 4.0,
            win_width: 0.5,
        };

        let mut gui_settings = GuiSettingsContainer::new();
        gui_settings.psf = load_psf(&Path::new("sample_data/psf.npz").to_path_buf()).unwrap();

        let mut progress = Arc::new(RwLock::new(None));
        let abort = Arc::new(AtomicBool::new(false));

        let output = filter.filter(&input, &mut gui_settings, &mut progress, &abort);

        assert_eq!(output.time.len(), n);
        assert_eq!(output.data.shape(), input.data.shape());
    }
}
