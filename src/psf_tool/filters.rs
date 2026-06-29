use ndarray::{Array1, Array2};
use num_complex::Complex64;
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

/// Spacing strategy for filter center frequencies
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FrequencySpacing {
    /// Logarithmic spacing (center frequencies spread on a log scale)
    Log,
    /// Linear spacing (center frequencies spread on a linear scale)
    Linear,
}

/// Parameters for filter creation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterParams {
    pub n_filters: usize,
    pub low_cut: f64,
    pub high_cut: f64,
    pub start_freq: f64,
    pub end_freq: f64,
    pub win_width: f64,
    pub frequency_spacing: FrequencySpacing,
}

impl Default for FilterParams {
    fn default() -> Self {
        Self {
            n_filters: 20,
            low_cut: 0.1,
            high_cut: 10.0,
            start_freq: 0.15,
            end_freq: 5.0,
            win_width: 0.5,
            frequency_spacing: FrequencySpacing::Log,
        }
    }
}

/// Computed FIR filters
#[derive(Debug, Clone)]
pub struct Filters {
    pub coefficients: Array2<f64>,
    pub center_frequencies: Vec<f64>,
    pub fs: f64, // sampling frequency in THz
}

/// Calculate Kaiser window attenuation
fn kaiser_atten(ntaps: usize, width_ratio: f64) -> f64 {
    let a = 2.285 * (ntaps as f64 - 1.0) * PI * width_ratio + 7.95;
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
    bessel_i(0.0, Complex64::new(x, 0.0), 40).re
}

fn factorial(n: usize) -> f64 {
    (1..=n).fold(1.0, |acc, i| acc * i as f64)
}

/// Approximate Gamma(x) (Lanczos) for real positive x.
fn gamma(x: f64) -> f64 {
    let g = 7.0;
    const COEFFS: [f64; 8] = [
        676.5203681218851,
        -1259.1392167224028,
        771.32342877765313,
        -176.61502916214059,
        12.507343278686905,
        -0.13857109526572012,
        9.9843695780195716e-6,
        1.5056327351493116e-7,
    ];
    if x < 0.5 {
        PI / ((PI * x).sin() * gamma(1.0 - x))
    } else {
        let x = x - 1.0;
        let mut a = 0.99999999999980993;
        for (i, coeff) in COEFFS.iter().enumerate() {
            a += coeff / (x + (i as f64) + 1.0);
        }
        let t = x + g + 0.5;
        ((2.0 * PI).sqrt()) * t.powf(x + 0.5) * (-t).exp() * a
    }
}

/// Compute modified Bessel I_v(z) via truncated series.
fn bessel_i(v: f64, z: Complex64, terms: usize) -> Complex64 {
    let mut sum = Complex64::new(0.0, 0.0);
    let half = Complex64::new(0.5, 0.0);
    for k in 0..terms {
        let kf = k as f64;
        let z_half = half * z;
        let exponent = 2.0 * kf + v;
        let numerator = z_half.powc(Complex64::new(exponent, 0.0));
        let denom = factorial(k) as f64 * gamma(v + kf + 1.0);
        sum += numerator / denom;
    }
    sum
}

/// Sinc function sin(x)/x (normalized, not sin(πx)/(πx)).
fn sinc(x: f64) -> f64 {
    if x == 0.0 {
        1.0
    } else {
        (x.sin()) / x
    }
}

/// Create a Kaiser window (individual coefficient)
fn kaiser_window_coeff(n: usize, n_taps: usize, beta: f64) -> f64 {
    let num =
        i0(beta * ((1.0 - (((2.0 * n as f64) / (n_taps as f64 - 1.0)) - 1.0).powi(2)).sqrt()));
    let denom = i0(beta);
    let out = if n == 0 || n == n_taps - 1 {
        0.0
    } else {
        num / denom
    };
    out
}

/// FIR low-pass design using time-domain Kaiser-windowed sinc.
fn firwin_kaiser_lowpass(n_taps: usize, cutoff_hz: f64, beta: f64, sampling_freq: f64) -> Vec<f64> {
    let adjusted_n_taps = if n_taps % 2 == 0 { n_taps - 1 } else { n_taps };
    let mid = (adjusted_n_taps - 1) as f64 / 2.0;
    let cutoff = cutoff_hz / sampling_freq;

    let mut filter: Vec<f64> = (0..adjusted_n_taps)
        .map(|n| {
            let sinc_val = sinc(2.0 * PI * cutoff * ((n as f64) - mid));
            let window_val = kaiser_window_coeff(n, adjusted_n_taps, beta);
            sinc_val * window_val
        })
        .collect();

    // Normalize filter for unitary gain at DC
    let sum_filter: f64 = filter.iter().sum();
    if sum_filter != 0.0 {
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
        *h = if i == mid as usize { 1.0 - *h } else { -(*h) };
    });

    if n_taps % 2 == 0 {
        filter.push(0.0);
    }

    filter
}

/// Design a Kaiser-windowed FIR bandpass filter (matches scipy.signal.firwin)
fn bandpass_kaiser(ntaps: usize, lowcut: f64, highcut: f64, fs: f64, width: f64) -> Array1<f64> {
    let width_ratio = width / (0.5 * fs);
    let atten = kaiser_atten(ntaps, width_ratio);
    let beta = kaiser_beta(atten);

    // Determine filter type and cutoffs
    let filter_vec: Vec<f64> = if lowcut <= 0.0 {
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
    };

    Array1::from_vec(filter_vec)
}

/// Create logarithmically spaced bandpass filters
pub fn create_filters(params: &FilterParams, times: &[f64]) -> Filters {
    // Determine FIR length (Python: ntaps = len(times) // 5)
    let mut ntaps = 499;
    if ntaps % 2 == 0 {
        ntaps += 1;
    }

    // Calculate sampling frequency
    let dt = times[1] - times[0];
    let fs = 1.0 / dt; // in THz (since times are in ps, fs is in THz)

    // Center frequencies depending on spacing mode
    let center_frequencies: Vec<f64> = match params.frequency_spacing {
        FrequencySpacing::Log => {
            let log_start = params.start_freq.ln();
            let log_end = params.end_freq.ln();
            let log_step = (log_end - log_start) / ((params.n_filters - 1) as f64);
            (0..params.n_filters)
                .map(|i| (log_start + i as f64 * log_step).exp())
                .collect()
        }
        FrequencySpacing::Linear => {
            let step = (params.end_freq - params.start_freq) / ((params.n_filters - 1) as f64);
            (0..params.n_filters)
                .map(|i| params.start_freq + i as f64 * step)
                .collect()
        }
    };

    // Create filters
    let mut coefficients = Array2::zeros((params.n_filters, ntaps));

    for (i, &center_freq) in center_frequencies.iter().enumerate() {
        // Calculate lowcut and highcut for this filter
        let lowcut = if i == 0 {
            params.low_cut
        } else {
            (center_frequencies[i - 1] * center_freq).sqrt()
        };

        let highcut = if i == params.n_filters - 1 {
            params.high_cut
        } else {
            (center_freq * center_frequencies[i + 1]).sqrt()
        };

        // Design the filter
        let filter_coeffs = bandpass_kaiser(ntaps, lowcut, highcut, fs, params.win_width);

        // Store in array
        for (j, &coeff) in filter_coeffs.iter().enumerate() {
            coefficients[[i, j]] = coeff;
        }
    }

    Filters {
        coefficients,
        center_frequencies,
        fs,
    }
}

/// Compute frequency response of a filter
pub fn frequency_response(filter_coeffs: &[f64], n_points: usize, fs: f64) -> (Vec<f64>, Vec<f64>) {
    let mut frequencies = Vec::with_capacity(n_points);
    let mut magnitudes = Vec::with_capacity(n_points);

    for k in 0..n_points {
        let freq = k as f64 * fs / (2.0 * n_points as f64);
        frequencies.push(freq);

        let omega = 2.0 * PI * freq / fs;

        let mut real = 0.0;
        let mut imag = 0.0;

        for (n, &h_n) in filter_coeffs.iter().enumerate() {
            let arg = -omega * n as f64;
            real += h_n * arg.cos();
            imag += h_n * arg.sin();
        }

        let magnitude = (real * real + imag * imag).sqrt();
        magnitudes.push(magnitude);
    }

    (frequencies, magnitudes)
}
