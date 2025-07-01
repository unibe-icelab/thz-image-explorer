//! Data structures and functionality for managing meta-information, housekeeping data,
//! scanned images, and related operations for image and signal processing tasks.
//!
//! This module provides:
//! - `HouseKeeping`: Metadata for scan/experiment conditions.
//! - `DataPoint`: A single scan/experiment result with time, frequency, and ROI data.
//! - `ScannedImageFilterData`: Multi-dimensional dataset for 2D scans with time/frequency domain data.

use crate::gui::matrix_plot::ROI;
use ndarray::{Array1, Array2, Array3};
use realfft::num_complex::Complex32;
use realfft::{ComplexToReal, RealToComplex};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Metadata describing the conditions and parameters of a scan or experiment.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct HouseKeeping {
    /// Resolution in the x direction (e.g., spatial step size).
    pub dx: f32,
    /// Range of x values: `[min, max]`.
    pub x_range: [f32; 2],
    /// Resolution in the y direction (e.g., spatial step size).
    pub dy: f32,
    /// Range of y values: `[min, max]`.
    pub y_range: [f32; 2],
    /// Start time of the scan or measurement.
    pub t_begin: f32,
    /// Total measurement range (e.g., duration or distance).
    pub range: f32,
    /// Ambient temperature during the scan (in °C).
    pub ambient_temperature: f64,
    /// Ambient pressure during the scan (in hPa).
    pub ambient_pressure: f64,
    /// Ambient humidity during the scan (in %).
    pub ambient_humidity: f64,
    /// Temperature of the sample being measured (in °C).
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

/// A single scan or experiment result, including time/frequency domain data and ROI information.
#[derive(Clone, Default, Debug)]
pub struct DataPoint {
    /// Housekeeping metadata for this data point.
    pub hk: HouseKeeping,
    /// List of available reference names.
    pub available_references: Vec<String>,
    /// List of available sample names.
    pub available_samples: Vec<String>,
    /// Thickness of the sample (in µm or mm).
    pub sample_thickness: f32,
    /// Time axis data (raw).
    pub time: Vec<f32>,
    /// Filtered time axis data.
    pub filtered_time: Vec<f32>,
    /// Primary signal data (time domain).
    pub signal: Vec<f32>,
    /// Filtered signal data (time domain).
    pub filtered_signal: Vec<f32>,
    /// Averaged signal data (time domain).
    pub avg_signal: Vec<f32>,
    /// Signal data for each ROI (region of interest).
    pub roi_signal: HashMap<String, Vec<f32>>,
    /// Frequency axis data (raw).
    pub frequencies: Vec<f32>,
    /// Filtered frequency axis data.
    pub filtered_frequencies: Vec<f32>,
    /// Absorption coefficient spectrum.
    pub absorption_coefficient: Vec<f32>,
    /// Refractive index spectrum.
    pub refractive_index: Vec<f32>,
    /// Extinction coefficient spectrum.
    pub extinction_coefficient: Vec<f32>,
    /// FFT amplitude of the signal.
    pub signal_fft: Vec<f32>,
    /// FFT phase of the signal.
    pub phase_fft: Vec<f32>,
    /// FFT amplitude of the filtered signal.
    pub filtered_signal_fft: Vec<f32>,
    /// FFT phase of the filtered signal.
    pub filtered_phase_fft: Vec<f32>,
    /// FFT amplitude of the averaged signal.
    pub avg_signal_fft: Vec<f32>,
    /// FFT phase of the averaged signal.
    pub avg_phase_fft: Vec<f32>,
    /// FFT amplitude for each ROI.
    pub roi_signal_fft: HashMap<String, Vec<f32>>,
    /// Phase spectrum for each ROI.
    pub roi_phase: HashMap<String, Vec<f32>>,
    /// List of regions of interest.
    pub rois: Vec<ROI>,
}

/// Multi-dimensional dataset for 2D spectroscopic imaging, including time and frequency domain data.
#[derive(Clone)]
pub struct ScannedImageFilterData {
    /// Minimum x value (if known).
    pub x_min: Option<f32>,
    /// Step size in x direction (if known).
    pub dx: Option<f32>,
    /// Minimum y value (if known).
    pub y_min: Option<f32>,
    /// Step size in y direction (if known).
    pub dy: Option<f32>,
    /// Height of the scan (number of pixels in y).
    pub height: usize,
    /// Width of the scan (number of pixels in x).
    pub width: usize,
    /// Scaling factor for visualization.
    pub scaling: usize,
    /// Currently selected pixel `[x, y]` for detailed analysis.
    pub pixel_selected: [usize; 2],
    /// FFT planner for real-to-complex transforms.
    pub r2c: Option<Arc<dyn RealToComplex<f32>>>,
    /// FFT planner for complex-to-real transforms.
    pub c2r: Option<Arc<dyn ComplexToReal<f32>>>,
    /// Map of ROI names to pixel coordinates.
    pub rois: HashMap<String, Vec<(usize, usize)>>,
    /// Time axis data for the scan.
    pub time: Array1<f32>,
    /// 2D intensity image derived from the scan.
    pub img: Array2<f32>,
    /// 3D array: (x, y, time) time-domain data.
    pub data: Array3<f32>,
    /// Averaged time-domain data across all pixels.
    pub avg_data: Array1<f32>,
    /// Additional datasets for specific references or samples.
    pub datasets: HashMap<String, Array1<f32>>,
    /// ROI-averaged time-domain data.
    pub roi_data: HashMap<String, Array1<f32>>,
    /// Frequency axis data for the scan.
    pub frequency: Array1<f32>,
    /// 3D array: (x, y, frequency) complex FFT results.
    pub fft: Array3<Complex32>,
    /// 3D array: (x, y, frequency) FFT amplitude.
    pub amplitudes: Array3<f32>,
    /// 3D array: (x, y, frequency) FFT phase.
    pub phases: Array3<f32>,
    /// Averaged FFT (complex) across all pixels.
    pub avg_fft: Array1<Complex32>,
    /// Averaged FFT amplitude across all pixels.
    pub avg_signal_fft: Array1<f32>,
    /// Averaged FFT phase across all pixels.
    pub avg_phase_fft: Array1<f32>,
    /// ROI-averaged FFT amplitude.
    pub roi_signal_fft: HashMap<String, Array1<f32>>,
    /// ROI-averaged FFT phase.
    pub roi_phase_fft: HashMap<String, Array1<f32>>,
}

impl Default for ScannedImageFilterData {
    fn default() -> Self {
        Self {
            x_min: None,
            dx: None,
            y_min: None,
            dy: None,
            height: 0,
            width: 0,
            scaling: 1, // Set the default value here
            pixel_selected: [0, 0],
            r2c: None,
            c2r: None,
            rois: HashMap::new(),
            time: Array1::zeros(0),
            img: Array2::zeros((0, 0)),
            data: Array3::zeros((0, 0, 0)),
            avg_data: Array1::zeros(0),
            datasets: HashMap::new(),
            roi_data: HashMap::new(),
            frequency: Array1::zeros(0),
            fft: Array3::zeros((0, 0, 0)),
            amplitudes: Array3::zeros((0, 0, 0)),
            phases: Array3::zeros((0, 0, 0)),
            avg_fft: Array1::zeros(0),
            avg_signal_fft: Array1::zeros(0),
            avg_phase_fft: Array1::zeros(0),
            roi_signal_fft: HashMap::new(),
            roi_phase_fft: HashMap::new(),
        }
    }
}
