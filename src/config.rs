pub enum Config {
    SetFFTWindowLow(f32),
    SetFFTWindowHigh(f32),
    SetFFTFilterLow(f32),
    SetFFTFilterHigh(f32),
    SetTimeWindowLow(f32),
    SetTimeWindowHigh(f32),
    SetFFTLogPlot(bool),
    SetFFTNormalization(bool),
    SetFFTResolution(f32),
    SetSelectedPixel([usize; 2]),
}

#[derive(Clone, Default)]
pub struct ConfigContainer {
    pub fft_window: [f32; 2],
    pub fft_filter: [f32; 2],
    pub time_window: [f32; 2],
    pub fft_log_plot: bool,
    pub normalize_fft: bool,
    pub fft_df: f32,
    pub selected_pixel: [usize; 2],
}
