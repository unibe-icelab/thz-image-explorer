//! This module implements a custom filter named `Deconvolution`, which operates on scanned images
//! and performs a deconvolution operation in the frequency domain.
//!
//! The implementation includes a Richardson-Lucy deconvolution algorithm placeholder,
//! allowing for further customization and parameterization.

use crate::data_container::ScannedImage;
use crate::filters::filter::{Filter, FilterConfig, FilterDomain, FilterParameter, ParameterKind};
use crate::gui::application::GuiSettingsContainer;
use filter_macros::register_filter;

/// Represents a `Deconvolution` filter.
///
/// This filter is designed to perform deconvolution using a configurable number of iterations
/// and a defined frequency range. It is implemented to work in the frequency domain.
///
/// Fields:
/// - `filter_number`: A placeholder for selecting predefined filters within the algorithm.
/// - `start_frequency`: The starting range for the frequency domain.
/// - `end_frequency`: The ending range for the frequency domain.
/// - `n_iterations`: The number of iterations for performing the deconvolution.
#[derive(Debug)]
#[register_filter]
pub struct Deconvolution {
    pub filter_number: usize,
    pub start_frequency: f64,
    pub end_frequency: f64,
    pub n_iterations: usize,
}

impl Filter for Deconvolution {
    /// Creates a new `Deconvolution` filter with default settings.
    ///
    /// Default values:
    /// - `n_iterations`: 10
    /// - `filter_number`: 10
    /// - `start_frequency`: 0.0
    /// - `end_frequency`: 10.0
    fn new() -> Self {
        Deconvolution {
            n_iterations: 10,
            filter_number: 10,
            start_frequency: 0.0,
            end_frequency: 10.0,
        }
    }

    /// Applies the deconvolution algorithm to a scanned image.
    ///
    /// # Arguments:
    /// - `_scan`: Mutable reference to the scanned image to be processed.
    /// - `_gui_settings`: Mutable reference to the GUI settings associated with the filter.
    ///
    /// # Notes:
    /// This method currently contains a placeholder for the Richardson-Lucy algorithm.
    fn filter(&self, _scan: &mut ScannedImage, _gui_settings: &mut GuiSettingsContainer) {
        // Implement your Richardson-Lucy algorithm here
    }

    /// Returns the configuration details of the `Deconvolution` filter.
    ///
    /// The configuration specifies:
    /// - Name: `"Deconvolution"`
    /// - Domain: `Frequency`
    /// - Parameters:
    ///     - `"Iterations"`: A positive integer representing the number of iterations.
    ///
    /// # Returns:
    /// A `FilterConfig` struct describing the filter's properties and parameters.
    fn config(&self) -> FilterConfig {
        FilterConfig {
            name: "Deconvolution".to_string(),
            domain: FilterDomain::Frequency,
            parameters: vec![FilterParameter {
                name: "Iterations".to_string(),
                kind: ParameterKind::UInt(self.n_iterations),
            }],
            // not used for now
            // vec![
            //     FilterParameter {
            //         name: "Filter Number".to_string(),
            //         kind: ParameterKind::UInt(self.filter_number),
            //     },
            //     FilterParameter {
            //         name: "Frequencies".to_string(),
            //         kind: ParameterKind::DoubleSlider {
            //             values: [self.start_frequency, self.end_frequency],
            //             show_in_plot: true,
            //             minimum_separation: 0.1,
            //             inverted: true,
            //             min: 0.0,
            //             max: 10.0,
            //         },
            //     },
            // ],        }
        }
    }
}
