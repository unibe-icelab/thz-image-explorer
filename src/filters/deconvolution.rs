use crate::data_container::ScannedImage;
use crate::filters::filter::{Filter, FilterConfig, FilterDomain, FilterParameter, ParameterKind};

pub struct Deconvolution {
    // the number of filters (i.e. the frequency resolution)
    pub filter_number: usize,
    // the start frequency (the first filter is a low pass filter averaging all frequencies below the cutoff)
    pub start_frequency: f64,
    // the end frequency (the last filter is a high pass filter)
    pub end_frequency: f64,
    // the number of iterations of the Richardson-Lucy algorithm
    pub n_iterations: usize,
}

impl Filter for Deconvolution {
    const DOMAIN: FilterDomain = FilterDomain::Frequency;

    fn filter(&self, _t: &mut ScannedImage) {
        todo!()
    }

    fn config(&self) -> FilterConfig {
        FilterConfig {
            name: "Deconvolution".to_string(),
            parameters: vec![
                FilterParameter {
                    name: "Filter Number".to_string(),
                    kind: ParameterKind::UInt(self.filter_number),
                },
                FilterParameter {
                    name: "Iterations".to_string(),
                    kind: ParameterKind::UInt(self.n_iterations),
                },
                FilterParameter {
                    name: "Frequencies".to_string(),
                    kind: ParameterKind::DoubleSlider {
                        values: [self.start_frequency, self.end_frequency],
                        show_in_plot: true,
                        minimum_separation: 0.1,
                        inverted: true,
                        min: 0.0,
                        max: 10.0,
                    },
                },
            ],
        }
    }
}
