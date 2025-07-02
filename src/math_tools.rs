//! Mathematical tools for terahertz time-domain spectroscopy signal processing.
//!
//! This module provides various windowing functions for signal processing, including Blackman, Hanning, Hamming,
//! and Flat Top windows. It features specialized implementations like the adapted Blackman window that selectively
//! applies windowing to signal edges. Additionally, it includes utilities for spectral analysis, such as
//! Fast Fourier Transform (FFT) operations and phase unwrapping for periodic signals.
//!
//! # Signal Processing Functions
//!
//! * **Windowing Functions**: Used to reduce spectral leakage during FFT operations by tapering
//!   signal edges. Various window types offer different trade-offs between spectral resolution
//!   and amplitude accuracy.
//!
//! * **FFT Operations**: Parallel implementations of forward and inverse FFT operations
//!   with support for different windowing methods and automatic phase unwrapping.
//!
//! * **Phase Unwrapping**: Tools to remove 2π discontinuities in phase data, producing
//!   continuous phase information across the spectrum.

use crate::config::ConfigContainer;
use crate::data_container::ScannedImageFilterData;
use ndarray::{Array1, Array3, ArrayView1, ArrayViewMut, Axis, Ix1, Zip};
use rayon::iter::IntoParallelIterator;
use rayon::iter::ParallelIterator;
use realfft::num_complex::Complex32;
use std::cmp::{max, min};
use std::f32::consts::PI;
use std::fmt::{Display, Formatter};

/// Enum representing the different types of FFT window functions supported.
///
/// These window functions can be applied to signals for spectral analysis.
/// The type determines the nature of the windowing used during FFT computation.
#[derive(PartialEq, Clone, Copy, Debug)]
pub enum FftWindowType {
    /// Adapted Blackman window with only the beginning and ending being altered.
    AdaptedBlackman,
    /// Original Blackman window
    Blackman,
    /// Hanning window
    Hanning,
    /// Hamming window
    Hamming,
    /// FlatTop
    FlatTop,
}

impl Display for FftWindowType {
    /// Provides a user-friendly string representation of each window type.
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            FftWindowType::AdaptedBlackman => {
                write!(f, "Adapted Blackman")
            }
            FftWindowType::Blackman => {
                write!(f, "Blackman")
            }
            FftWindowType::Hanning => {
                write!(f, "Hanning")
            }
            FftWindowType::Hamming => {
                write!(f, "Haming")
            }
            FftWindowType::FlatTop => {
                write!(f, "Flat Top")
            }
        }
    }
}

/// Computes the Blackman window value for a given sample.
///
/// The implementation follows the mathematical definition as used by Python's numpy library.
///
/// # Arguments
/// - `n`: The current time or sample index.
/// - `m`: The total length of the signal.
///
/// # Returns
/// The computed value of the Blackman window. It automatically clamps the value in the range [0.0, 1.0].
fn blackman_window(n: f32, m: f32) -> f32 {
    // blackman window as implemented by numpy (python)
    let res = 0.42 - 0.5 * (2.0 * std::f32::consts::PI * n / m).cos()
        + 0.08 * (4.0 * std::f32::consts::PI * n / m).cos();
    if res.is_nan() {
        1.0
    } else {
        res.clamp(0.0, 1.0)
    }
}

/// Applies the adapted Blackman window to a signal's time series.
///
/// The window is adjusted with specific lower and upper bounds. It attenuates
/// the starting and ending portions of the signal's amplitude using Blackman window values.
///
/// # Arguments
/// - `signal`: A mutable view of the signal to which the window will be applied.
/// - `time`: The corresponding time values for the signal.
/// - `lower_bound`: The lower bound for the Blackman window.
/// - `upper_bound`: The upper bound for the Blackman window.
pub fn apply_adapted_blackman_window(
    signal: &mut ArrayViewMut<f32, Ix1>,
    time: &Array1<f32>,
    lower_bound: &f32,
    upper_bound: &f32,
) {
    for (s, t) in signal.iter_mut().zip(time.iter()) {
        if *t <= lower_bound + time[0] {
            // first half of blackman
            let bw = blackman_window(t - time[0], 2.0 * lower_bound);
            *s *= bw;
        } else if *t >= time[time.len() - 1] - upper_bound {
            // second half of blackman
            let bw = blackman_window(
                t - (time[time.len() - 1] - upper_bound * 2.0),
                2.0 * upper_bound,
            );
            *s *= bw;
        }
    }
}

/// Normalizes a time array to the range [0, 1].
///
/// # Arguments
/// - `time`: The input time array.
///
/// # Returns
/// The normalized array, where all values are scaled between 0 and 1.
fn normalize_time(time: &Array1<f32>) -> Array1<f32> {
    let min = time.iter().fold(f32::INFINITY, |a, &b| a.min(b)); // Find the minimum value
    let max = time.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b)); // Find the maximum value
    time.mapv(|t| (t - min) / (max - min))
}

/// Applies the Hamming window to the given signal.
///
/// Attenuates the signal with the Hamming window formula, which is well-suited
/// for reducing spectral leakage.
///
/// # Arguments
/// - `signal`: A mutable view of the signal to modify.
/// - `time`: A time array corresponding to the signal.
pub fn apply_hamming(signal: &mut ArrayViewMut<f32, ndarray::Ix1>, time: &Array1<f32>) {
    let normalized_time = normalize_time(time);
    Zip::from(signal).and(&normalized_time).for_each(|s, t| {
        *s *= 0.54 - 0.46 * (2.0 * std::f32::consts::PI * t).cos();
    });
}

/// Applies the Hanning (Hann) window to the given signal.
///
/// Well-suited for periodic signals, the Hanning window reduces discontinuities
/// at the boundaries of signal segments.
///
/// # Arguments
/// - `signal`: A mutable view of the signal to modify.
/// - `time`: A time array corresponding to the signal.
pub fn apply_hanning(signal: &mut ArrayViewMut<f32, ndarray::Ix1>, time: &Array1<f32>) {
    let normalized_time = normalize_time(time);
    Zip::from(signal).and(&normalized_time).for_each(|s, t| {
        *s *= 0.5 * (1.0 - (2.0 * std::f32::consts::PI * t).cos());
    });
}

/// Applies the Blackman window to the given signal.
///
/// Reduces spectral leakage by applying the Blackman formula to the signal amplitudes.
///
/// # Arguments
/// - `signal`: A mutable view of the signal to modify.
/// - `time`: A time array corresponding to the signal.
pub fn apply_blackman(signal: &mut ArrayViewMut<f32, ndarray::Ix1>, time: &Array1<f32>) {
    let normalized_time = normalize_time(time);
    Zip::from(signal).and(&normalized_time).for_each(|s, t| {
        *s *= 0.42 - 0.5 * (2.0 * std::f32::consts::PI * t).cos()
            + 0.08 * (4.0 * std::f32::consts::PI * t).cos();
    });
}

/// Applies the Flat Top window to the given signal.
///
/// The Flat Top window is used to provide a very flat passband response
/// for use in measurement applications.
///
/// # Arguments
/// - `signal`: A mutable view of the signal to modify.
/// - `time`: A time array corresponding to the signal.
pub fn apply_flat_top(signal: &mut ArrayViewMut<f32, ndarray::Ix1>, time: &Array1<f32>) {
    let normalized_time = normalize_time(time);
    Zip::from(signal).and(&normalized_time).for_each(|s, t| {
        *s *= 1.0 - 1.93 * (2.0 * std::f32::consts::PI * t).cos()
            + 1.29 * (4.0 * std::f32::consts::PI * t).cos()
            - 0.388 * (6.0 * std::f32::consts::PI * t).cos()
            + 0.028 * (8.0 * std::f32::consts::PI * t).cos();
    });
}

/// Unwraps a periodic signal's values based on the provided period.
///
/// Removes phase discontinuities by adjusting the signal's values to fall within
/// a continuous range.
///
/// # Arguments
/// - `x`: The input signal as an array of values.
/// - `period`: Optional period of the signal. If not provided, it is estimated from the signal.
///
/// # Returns
/// A vector containing the unwrapped signal values.
pub fn numpy_unwrap(x: &[f32], period: Option<f32>) -> Vec<f32> {
    // this was generated by ChatGPT

    let period = period.unwrap_or_else(|| {
        let diff = x[1..]
            .iter()
            .zip(x.iter())
            .map(|(&a, &b)| a - b)
            .collect::<Vec<f32>>();
        let diff_mean = diff.iter().sum::<f32>() / diff.len() as f32;
        2.0 * std::f32::consts::PI / diff_mean
    });
    let mut unwrapped = x.to_owned();
    let mut prev_val = x[0];
    let mut prev_unwrapped = x[0];
    for i in 1..x.len() {
        let val = x[i];
        let mut diff = val - prev_val;
        if diff > period / 2.0 {
            diff -= period;
        } else if diff < -period / 2.0 {
            diff += period;
        }
        let unwrapped_val = prev_unwrapped + diff;
        prev_val = val;
        prev_unwrapped = unwrapped_val;
        unwrapped[i] = unwrapped_val;
    }
    unwrapped
}

pub fn scaling(input: &ScannedImageFilterData, config: &ConfigContainer) -> ScannedImageFilterData {
    let scaling = config.scale_factor;
    if scaling <= 1 {
        return input.clone();
    }

    let mut output = input.clone();

    let new_width = input.width / scaling;
    let new_height = input.height / scaling;

    if new_width == 0 || new_height == 0 {
        // Scaling is too large, return original data
        return input.clone();
    }

    // Update metadata for scaled data, but keep original width/height for display
    output.width = new_width;
    output.height = new_height;
    output.scaling = scaling;
    if let Some(dx) = output.dx {
        output.dx = Some(dx * scaling as f32);
    }
    if let Some(dy) = output.dy {
        output.dy = Some(dy * scaling as f32);
    }

    output.pixel_selected[0] /= scaling;
    output.pixel_selected[1] /= scaling;

    // Helper function to scale a 3D array by averaging blocks
    fn scale_3d<T: Clone + Default + std::ops::AddAssign + std::ops::Div<f32, Output = T>>(
        data: &Array3<T>,
        new_width: usize,
        new_height: usize,
        scaling: usize,
    ) -> Array3<T> {
        let z_len = data.shape()[2];
        let mut scaled_data = Array3::<T>::default((new_width, new_height, z_len));
        let scaling_factor = (scaling * scaling) as f32;

        for nx in 0..new_width {
            for ny in 0..new_height {
                for z in 0..z_len {
                    let mut sum = T::default();
                    for i in 0..scaling {
                        for j in 0..scaling {
                            let ox = nx * scaling + i;
                            let oy = ny * scaling + j;
                            if ox < data.shape()[0] && oy < data.shape()[1] {
                                sum += data[[ox, oy, z]].clone();
                            }
                        }
                    }
                    scaled_data[[nx, ny, z]] = sum / scaling_factor;
                }
            }
        }
        scaled_data
    }

    // Scale 3D data arrays for faster backend processing
    output.data = scale_3d(&input.data, new_width, new_height, scaling);
    output.amplitudes = scale_3d(&input.amplitudes, new_width, new_height, scaling);
    output.phases = scale_3d(&input.phases, new_width, new_height, scaling);
    output.fft = scale_3d(&input.fft, new_width, new_height, scaling);

    output
}

/// Performs Fast Fourier Transform (FFT) on time-domain data.
///
/// This function applies the selected windowing function to the time-domain data before
/// transforming it to the frequency domain. It processes the data in parallel across
/// spatial dimensions using Rayon's parallel iterator capabilities.
///
/// The function:
/// 1. Applies the configured window function to the time-domain data
/// 2. Performs the forward FFT transformation
/// 3. Calculates amplitude and phase information from the complex spectrum
/// 4. Unwraps the phase data to remove 2π discontinuities
///
/// # Arguments
/// * `output` - The data container holding time-domain data to transform
/// * `config` - Configuration settings including window type and parameters
///
/// # Returns
/// A new `ScannedImageFilterData` instance with the FFT results
pub fn fft(input: &ScannedImageFilterData, config: &ConfigContainer) -> ScannedImageFilterData {
    let mut output = input.clone();
    if let Some(r2c) = &output.r2c {
        (
            output.data.axis_iter_mut(Axis(0)),
            output.phases.axis_iter_mut(Axis(0)),
            output.amplitudes.axis_iter_mut(Axis(0)),
            output.fft.axis_iter_mut(Axis(0)),
        )
            .into_par_iter()
            .for_each(
                |(
                    mut data_columns,
                    mut output_phases_columns,
                    mut output_amplitude_columns,
                    mut output_fft_columns,
                )| {
                    let mut input_vec = vec![0.0; output.time.len()];
                    let mut spectrum = r2c.make_output_vec();
                    for (((mut input_data, mut phases), mut amplitudes), mut fft) in data_columns
                        .axis_iter_mut(Axis(0))
                        .zip(output_phases_columns.axis_iter_mut(Axis(0)))
                        .zip(output_amplitude_columns.axis_iter_mut(Axis(0)))
                        .zip(output_fft_columns.axis_iter_mut(Axis(0)))
                    {
                        // Apply the selected window function to the time domain data
                        match config.fft_window_type {
                            FftWindowType::AdaptedBlackman => {
                                apply_adapted_blackman_window(
                                    &mut input_data,
                                    &output.time,
                                    &config.fft_window[0],
                                    &config.fft_window[1],
                                );
                            }
                            FftWindowType::Blackman => {
                                apply_blackman(&mut input_data, &output.time)
                            }
                            FftWindowType::Hanning => apply_hanning(&mut input_data, &output.time),
                            FftWindowType::Hamming => apply_hamming(&mut input_data, &output.time),
                            FftWindowType::FlatTop => apply_flat_top(&mut input_data, &output.time),
                        }

                        // Forward transform the input data
                        input_vec.clone_from_slice(input_data.as_slice().unwrap());
                        r2c.process(&mut input_vec, &mut spectrum).unwrap();

                        // Assign spectrum to fft
                        fft.assign(&Array1::from_vec(spectrum.clone()));

                        // Assign amplitudes
                        amplitudes
                            .iter_mut()
                            .zip(spectrum.iter())
                            .for_each(|(a, s)| *a = s.norm());

                        // Assign phases (unwrap)
                        let phase: Vec<f32> = spectrum.iter().map(|s| s.arg()).collect();
                        let unwrapped = numpy_unwrap(&phase, Some(2.0 * PI));
                        phases
                            .iter_mut()
                            .zip(unwrapped.iter())
                            .for_each(|(p, v)| *p = *v);
                    }
                },
            );
    };
    output
}

/// Performs Inverse Fast Fourier Transform (IFFT) on frequency-domain data.
///
/// This function transforms frequency-domain data back to the time domain using
/// parallel processing. It operates on the complex spectral data stored in the FFT
/// field of the input container.
///
/// The function:
/// 1. Retrieves the complex spectrum for each pixel
/// 2. Performs the inverse FFT operation
/// 3. Normalizes the resulting time-domain signal
/// 4. Updates the time-domain data field in the container
///
/// # Arguments
/// * `output` - The data container holding frequency-domain data to transform
/// * `config` - Configuration settings (unused in this function)
///
/// # Returns
/// A new `ScannedImageFilterData` instance with the IFFT results
pub fn ifft(input: &ScannedImageFilterData, config: &ConfigContainer) -> ScannedImageFilterData {
    let mut output = input.clone();

    output.avg_fft = output
        .fft
        .mean_axis(Axis(0))
        .expect("Axis 2 mean failed")
        .mean_axis(Axis(0))
        .expect("Axis 1 mean failed");

    output.avg_signal_fft = output
        .amplitudes
        .mean_axis(Axis(0))
        .expect("Axis 2 mean failed")
        .mean_axis(Axis(0))
        .expect("Axis 1 mean failed");

    output.avg_phase_fft = output
        .phases
        .mean_axis(Axis(0))
        .expect("Axis 2 mean failed")
        .mean_axis(Axis(0))
        .expect("Axis 1 mean failed");

    if config.avg_in_fourier_space {
        // println!("[FFT] Performing IFFT on average amplitude and phase data");
        // Reconstruct complex spectrum from average amplitude and phase
        if let Some(c2r) = &output.c2r {
            // Create a complex spectrum from the averaged amplitude and phase
            let mut spectrum = vec![Complex32::new(0.0, 0.0); output.frequency.len()];

            for (i, (&amp, &phase)) in output
                .avg_signal_fft
                .iter()
                .zip(output.avg_phase_fft.iter())
                .enumerate()
            {
                // Convert from polar form (amplitude, phase) to complex
                spectrum[i] = Complex32::from_polar(amp, phase);
            }

            let mut real_output = vec![0.0; output.time.len()];

            // Perform inverse FFT on the reconstructed spectrum
            c2r.process(&mut spectrum, &mut real_output).unwrap();

            // Normalize the result
            let length = real_output.len();
            let normalized: Vec<f32> = real_output.iter().map(|&v| v / length as f32).collect();

            // Store the result in output.avg_data
            output.avg_data = Array1::from_vec(normalized);
        }
    }

    // Process all ROIs after handling the average signal
    for (roi_uuid, (roi_name, polygon)) in &input.rois {
        // Time domain ROI processing (direct spatial averaging)
        if !config.avg_in_fourier_space {
            let roi_signal = average_polygon_roi(&input.data, polygon, input.scaling);
            output
                .roi_data
                .insert(roi_uuid.clone(), (roi_name.clone(), roi_signal));
        }

        // Frequency domain processing (for visualization)
        let roi_signal_fft = average_polygon_roi(&input.amplitudes, polygon, input.scaling);
        let roi_phase_fft = average_polygon_roi(&input.phases, polygon, input.scaling);

        // Store frequency domain results
        output
            .roi_signal_fft
            .insert(roi_uuid.clone(), (roi_name.clone(), roi_signal_fft.clone()));
        output
            .roi_phase_fft
            .insert(roi_uuid.clone(), (roi_name.clone(), roi_phase_fft.clone()));

        // In the ifft method where ROIs are processed:
        if config.avg_in_fourier_space {
            if let Some(c2r) = &output.c2r {
                // Create a complex spectrum from ROI amplitude and phase
                let mut spectrum = vec![Complex32::new(0.0, 0.0); input.frequency.len()];

                for (i, (&amp, &phase)) in
                    roi_signal_fft.iter().zip(roi_phase_fft.iter()).enumerate()
                {
                    // Convert from polar form (amplitude, phase) to complex
                    spectrum[i] = Complex32::from_polar(amp, phase);
                }

                // Enforce constraints for realfft compatibility:
                // 1. First element must have zero imaginary part
                if !spectrum.is_empty() {
                    spectrum[0] = Complex32::new(spectrum[0].re, 0.0);
                }

                let mut real_output = vec![0.0; input.time.len()];

                // Error handling instead of unwrap
                match c2r.process(&mut spectrum, &mut real_output) {
                    Ok(_) => {
                        // Normalize the result
                        let length = real_output.len();
                        let normalized: Vec<f32> =
                            real_output.iter().map(|&v| v / length as f32).collect();

                        // Store the reconstructed time domain signal
                        let roi_signal = Array1::from_vec(normalized);
                        output
                            .roi_data
                            .insert(roi_uuid.clone(), (roi_name.clone(), roi_signal));
                    }
                    Err(e) => {
                        println!("IFFT error for ROI {}: {}", roi_name, e);
                        // Fall back to time-domain averaging if IFFT fails
                        let roi_signal = average_polygon_roi(&input.data, polygon, input.scaling);
                        output
                            .roi_data
                            .insert(roi_uuid.clone(), (roi_name.clone(), roi_signal));
                    }
                }
            }
        }
    }

    if let Some(c2r) = &output.c2r {
        (
            output.fft.axis_iter(Axis(0)),
            output.data.axis_iter_mut(Axis(0)),
        )
            .into_par_iter()
            .for_each(|(fft_columns, mut data_columns)| {
                let mut spectrum = vec![Complex32::new(0.0, 0.0); output.frequency.len()];
                let mut real_output = vec![0.0; output.time.len()];
                for (fft, mut data) in fft_columns
                    .axis_iter(Axis(0))
                    .zip(data_columns.axis_iter_mut(Axis(0)))
                {
                    // Copy spectrum from fft view
                    spectrum.clone_from_slice(fft.as_slice().unwrap());
                    // Perform inverse FFT
                    c2r.process(&mut spectrum, &mut real_output).unwrap();
                    // Normalize if needed (realfft does not always normalize)
                    let length = real_output.len();
                    let normalized: Vec<f32> =
                        real_output.iter().map(|&v| v / length as f32).collect();
                    data.assign(&Array1::from_vec(normalized));
                }
            });
    }
    output
}

/// Check if a point is inside a polygon using the ray casting algorithm
fn point_in_polygon(x: usize, y: usize, polygon: &[(usize, usize)]) -> bool {
    let mut inside = false;
    let mut j = polygon.len() - 1;

    for i in 0..polygon.len() {
        let (xi, yi) = polygon[i];
        let (xj, yj) = polygon[j];

        let intersect = ((yi > y) != (yj > y)) && (x < (xj - xi) * (y - yi) / (yj - yi) + xi);

        if intersect {
            inside = !inside;
        }

        j = i;
    }
    inside
}

/// Average values in a 3D array within a polygon-defined ROI
///
/// * `data` - 3D array with dimensions [z, y, x]
/// * `polygon` - Vector of (x, y) coordinates defining the ROI boundary
///
/// Returns a 1D array with averages along the z-axis for the polygon region
pub fn average_polygon_roi(
    data: &Array3<f32>,
    polygon: &Vec<(usize, usize)>,
    scaling: usize,
) -> Array1<f32> {
    let mut polygon = polygon.clone();

    for (x, y) in polygon.iter_mut() {
        *x /= scaling;
        *y /= scaling;
    }
    let x_size = data.shape()[0];
    let y_size = data.shape()[1];
    let z_size = data.shape()[2];

    // Create output array
    let mut result = Array1::zeros(z_size);
    let mut pixel_counts = vec![0; z_size];

    // Find bounding box of polygon to reduce computation
    let mut x_min = usize::MAX;
    let mut y_min = usize::MAX;
    let mut x_max = 0;
    let mut y_max = 0;

    for (x, y) in polygon.iter() {
        x_min = min(x_min, *x);
        y_min = min(y_min, *y);
        x_max = max(x_max, *x);
        y_max = max(y_max, *y);
    }

    // Clamp to array bounds
    x_min = min(x_min, x_size - 1);
    y_min = min(y_min, y_size - 1);
    x_max = min(x_max, x_size - 1);
    y_max = min(y_max, y_size - 1);

    // For each pixel in the bounding box
    for y in y_min..=y_max {
        for x in x_min..=x_max {
            // Check if the pixel is inside the polygon
            if point_in_polygon(x, y, &polygon) {
                // Add the value to the average for each z-slice
                for z in 0..z_size {
                    result[z] += data[[x, y, z]];
                    pixel_counts[z] += 1;
                }
            }
        }
    }

    // Calculate average for each z-slice
    for z in 0..z_size {
        if pixel_counts[z] > 0 {
            result[z] /= pixel_counts[z] as f32;
        }
    }
    result
}

const C: f32 = 2.99792458e8_f32;

pub fn calculate_optical_properties(
    sample_amplitude: ArrayView1<f32>,
    sample_phase: ArrayView1<f32>,
    reference_amplitude: ArrayView1<f32>,
    reference_phase: ArrayView1<f32>,
    frequencies: ArrayView1<f32>,
    sample_thickness: f32,
) -> (Array1<f32>, Array1<f32>, Array1<f32>) {
    let mut refractive_index = Array1::zeros(frequencies.len());
    let mut absorption_coeff = Array1::zeros(frequencies.len());
    let mut extinction_coeff = Array1::zeros(frequencies.len());

    // Calculate for each frequency point
    for i in 0..frequencies.len() {
        // Convert frequency to Hz (from THz)
        let frequency_hz = frequencies[i] * 1.0e12;

        // Phase difference (may need unwrapping for discontinuities)
        let phase_diff = sample_phase[i] - reference_phase[i];

        // Refractive index: n = 1 + (c * Δφ) / (2π * f * d)
        let n = 1.0 + (C * phase_diff) / (2.0 * PI * frequency_hz * sample_thickness);

        // Absorption coefficient: α = -2 * ln(|T|) / d
        // where |T| = |E_sample| / |E_reference|
        let amplitude_ratio = sample_amplitude[i] / reference_amplitude[i];
        let alpha = -2.0 * amplitude_ratio.ln() / sample_thickness;

        // Extinction coefficient: κ = α * c / (4π * f)
        let kappa = alpha * C / (4.0 * PI * frequency_hz);

        refractive_index[i] = n;
        absorption_coeff[i] = alpha;
        extinction_coeff[i] = kappa;
    }

    (refractive_index, absorption_coeff, extinction_coeff)
}
