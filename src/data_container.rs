//! This module defines data structures and functionality for managing meta-information, housekeeping data,
//! scanned images, and related operations for image and signal processing tasks.

use ndarray::{Array1, Array2, Array3};
use realfft::num_complex::Complex32;
use realfft::{ComplexToReal, RealToComplex};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

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
/// - `signal`: Primary signal data.
/// - `filtered_signal`: Filtered signal data.
/// - `avg_signal`: Averaged signal data.
/// - `frequencies`: Frequency axis data.
/// - `signal_fft`, `phase_fft`: Raw FFT and phase data for `signal`.
/// - `filtered_signal_fft`, `filtered_phase_fft`: FFT and phase data for the filtered signal.
/// - `avg_signal_fft`, `avg_phase_fft`: Averaged FFT and phase data.
#[derive(Clone, Default, Debug)]
pub struct DataPoint {
    pub hk: HouseKeeping,
    pub time: Vec<f32>,
    pub filtered_time: Vec<f32>,
    pub signal: Vec<f32>,
    pub filtered_signal: Vec<f32>,
    pub avg_signal: Vec<f32>,
    pub frequencies: Vec<f32>,
    pub filtered_frequencies: Vec<f32>,
    pub signal_fft: Vec<f32>,
    pub phase_fft: Vec<f32>,
    pub filtered_signal_fft: Vec<f32>,
    pub filtered_phase_fft: Vec<f32>,
    pub avg_signal_fft: Vec<f32>,
    pub avg_phase_fft: Vec<f32>,
}

/// Represents a multi-dimensional dataset for spectroscopic imaging with both time and frequency domain data.
///
/// This structure contains the complete representation of a 2D scan with time-resolved measurements
/// at each spatial position, along with derived frequency-domain data obtained through FFT. It
/// maintains both the raw data and processed results (amplitudes, phases) for analysis and visualization.
///
/// # Fields
/// - `x_min`, `dx`: Starting position and step size in the x-direction.
/// - `y_min`, `dy`: Starting position and step size in the y-direction.
/// - `height`, `width`: Dimensions of the 2D scan in pixels.
/// - `scaling`: Scaling factor for visualization purposes.
/// - `pixel_selected`: Currently selected pixel coordinates for detailed analysis.
/// - `r2c`: FFT planner for real-to-complex transforms.
/// - `c2r`: FFT planner for complex-to-real transforms.
/// - `time`: Time axis data as a 1D array.
/// - `img`: 2D array representing an intensity image derived from the data.
/// - `data`: 3D array containing time-domain data for all spatial positions (x, y, time).
/// - `frequency`: Frequency axis data as a 1D array.
/// - `fft`: 3D array of complex FFT results for all spatial positions (x, y, frequency).
/// - `amplitudes`: 3D array of FFT amplitude data (x, y, frequency).
/// - `phases`: 3D array of phase information (x, y, frequency).
#[derive(Default, Clone)]
pub struct ScannedImageFilterData {
    pub x_min: Option<f32>,
    pub dx: Option<f32>,
    pub y_min: Option<f32>,
    pub dy: Option<f32>,
    pub height: usize,
    pub width: usize,
    pub scaling: usize,
    pub pixel_selected: [usize; 2],
    pub r2c: Option<Arc<dyn RealToComplex<f32>>>,
    pub c2r: Option<Arc<dyn ComplexToReal<f32>>>,
    pub time: Array1<f32>,
    pub img: Array2<f32>,
    pub data: Array3<f32>,
    pub avg_data: Array1<f32>,
    pub frequency: Array1<f32>,
    pub fft: Array3<Complex32>,
    pub amplitudes: Array3<f32>,
    pub phases: Array3<f32>,
    pub avg_fft: Array1<Complex32>,
    pub avg_signal_fft: Array1<f32>,
    pub avg_phase_fft: Array1<f32>,
}
