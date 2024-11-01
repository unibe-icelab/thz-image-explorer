pub struct Deconvolution {
    // the number of filters (i.e. the frequency resolution)
    pub filter_number: usize,
    // the start frequency (the first filter is a low pass filter averaging all frequencies below the cutoff)
    pub start_frequency: f32,
    // the end frequency (the last filter is a high pass filter)
    pub end_frequency: f32,
    // the number of iterations of the Richardson-Lucy algorithm
    pub n_iterations: usize,
}
