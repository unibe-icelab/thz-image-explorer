use crate::data_container::ScannedImage;
use crate::filters::filter::{Filter, FilterConfig, FilterDomain, FilterParameter, ParameterKind};
use filter_macros::register_filter;

#[derive(Debug)]
#[register_filter]
pub struct Deconvolution {
    pub filter_number: usize,
    pub start_frequency: f64,
    pub end_frequency: f64,
    pub n_iterations: usize,
}

impl Filter for Deconvolution {
    fn new() -> Self {
        Deconvolution {
            filter_number: 10,
            start_frequency: 0.0,
            end_frequency: 10.0,
            n_iterations: 10,
        }
    }

    fn filter(&self, _t: &mut ScannedImage) {
        // Implement your Richardson-Lucy algorithm here
    }

    fn config(&self) -> FilterConfig {
        FilterConfig {
            name: "Deconvolution".to_string(),
            domain: FilterDomain::Frequency,
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
