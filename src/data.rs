use std::fmt;
use itertools_num::linspace;
use serde::{Serialize, Deserialize};

pub const NUM_PULSE_LINES: usize = 1001;
pub const NUM_FFT_LINES: usize = 10001;


#[derive(Clone, Serialize, Deserialize)]
pub struct DataContainer {
    pub valid: bool,
    pub time: Vec<f64>,
    pub signal_1: Vec<f64>,
    pub ref_1: Vec<f64>,
    pub frequencies_fft: Vec<f64>,
    pub signal_1_fft: Vec<f64>,
    pub phase_1_fft: Vec<f64>,
    pub ref_1_fft: Vec<f64>,
    pub ref_phase_1_fft: Vec<f64>,
}

impl Default for DataContainer {
    fn default() -> DataContainer {
        return DataContainer {
            valid: false,
            time: linspace::<f64>(0.0, 1000.0, NUM_PULSE_LINES).collect(),
            signal_1: vec![0.0; NUM_PULSE_LINES],
            ref_1: vec![0.0; NUM_PULSE_LINES],
            frequencies_fft: vec![0.0; NUM_FFT_LINES],
            signal_1_fft: vec![0.0; NUM_FFT_LINES],
            phase_1_fft: vec![0.0; NUM_FFT_LINES],
            ref_1_fft: vec![0.0; NUM_FFT_LINES],
            ref_phase_1_fft: vec![0.0; NUM_FFT_LINES],
        };
    }
}