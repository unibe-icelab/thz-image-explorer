//! This module provides various windowing functions for signal processing, including Blackman, Hanning, Hamming,
//! Flat Top windows, and an adapted Blackman window implementation. Additionally, it includes a utility for
//! unwrapping phase ranges in periodic signals.

use ndarray::{Array1, ArrayViewMut, Ix1, Zip};
use std::f32::consts::PI;
use std::fmt::{Display, Formatter};

/// Enum representing the different types of FFT window functions supported.
///
/// These window functions can be applied to signals for spectral analysis.
/// The type determines the nature of the windowing used during FFT computation.
#[derive(PartialEq, Clone, Copy)]
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
    let res = 0.42 - 0.5 * (2.0 * PI * n / m).cos() + 0.08 * (4.0 * PI * n / m).cos();
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
        *s *= 0.54 - 0.46 * (2.0 * PI * t).cos();
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
        *s *= 0.5 * (1.0 - (2.0 * PI * t).cos());
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
        *s *= 0.42 - 0.5 * (2.0 * PI * t).cos() + 0.08 * (4.0 * PI * t).cos();
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
        *s *= 1.0 - 1.93 * (2.0 * PI * t).cos() + 1.29 * (4.0 * PI * t).cos()
            - 0.388 * (6.0 * PI * t).cos()
            + 0.028 * (8.0 * PI * t).cos();
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
