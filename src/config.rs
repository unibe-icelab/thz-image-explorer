use std::path::PathBuf;

pub enum Config {
    OpenFile(PathBuf),
    SetFFTWindowLow(f32),
    SetFFTWindowHigh(f32),
    SetFFTFilterLow(f32),
    SetFFTFilterHigh(f32),
    SetTimeWindowLow(f32),
    SetTimeWindowHigh(f32),
    SetFFTLogPlot(bool),
    SetFFTNormalization(bool),
    SetFFTResolution(f32),
    SetDownScaling(usize),
    SetSelectedPixel([usize; 2]),
}

#[derive(Clone)]
pub struct ConfigContainer {
    pub fft_window: [f32; 2],
    pub fft_filter: [f32; 2],
    pub time_window: [f32; 2],
    pub down_scaling: usize,
    pub fft_log_plot: bool,
    pub normalize_fft: bool,
    pub fft_df: f32,
    pub selected_pixel: [usize; 2],
}

impl Default for ConfigContainer {
    fn default() -> Self {
        ConfigContainer {
            fft_window: [1.0, 7.0],
            fft_filter: [0.0, 10.0],
            time_window: [1000.0, 1050.0],
            down_scaling: 1,
            fft_log_plot: false,
            normalize_fft: false,
            fft_df: 1.0,
            selected_pixel: [0, 0],
        }
    }
}
