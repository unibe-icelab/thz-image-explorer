use itertools_num::linspace;
use serde::{Deserialize, Serialize};

pub const NUM_PULSE_LINES: usize = 1001;
pub const NUM_FFT_LINES: usize = 10001;

#[derive(Clone, Serialize, Deserialize)]
pub struct HouseKeeping {
    pub dx: f64,
    pub x_range: [f64; 2],
    pub dy: f64,
    pub y_range: [f64; 2],
    pub t_begin: f64,
    pub range: f64,
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

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct DataContainer {
    pub hk: HouseKeeping,
    pub cut_off: f64,
    pub valid: bool,
    pub time: Vec<f64>,
    pub signal_1: Vec<f64>,
    pub filtered_signal_1: Vec<f64>,
    pub ref_1: Vec<f64>,
    pub frequencies_fft: Vec<f64>,
    pub signal_1_fft: Vec<f64>,
    pub phase_1_fft: Vec<f64>,
    pub filtered_signal_1_fft: Vec<f64>,
    pub filtered_phase_1_fft: Vec<f64>,
    pub ref_1_fft: Vec<f64>,
    pub ref_phase_1_fft: Vec<f64>,
}
