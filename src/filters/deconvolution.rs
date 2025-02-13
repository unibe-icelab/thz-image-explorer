//! This module implements a custom filter named `Deconvolution`, which operates on scanned images
//! and performs a deconvolution operation in the frequency domain.
//!
//! The implementation includes a Richardson-Lucy deconvolution algorithm placeholder,
//! allowing for further customization and parameterization.

use crate::data_container::ScannedImage;
use crate::filters::filter::{Filter, FilterConfig, FilterDomain, ParameterKind};
use crate::gui::application::GuiSettingsContainer;
use filter_macros::register_filter;
use std::collections::HashMap;

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
    pub config: FilterConfig,
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
        let mut parameters = HashMap::new();
        parameters.insert("Iterations".to_string(), ParameterKind::UInt(10));

        let config = FilterConfig {
            name: "Deconvolution".to_string(),
            domain: FilterDomain::Frequency,
            parameters,
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
        };

        Deconvolution {
            filter_number: 10,
            start_frequency: 0.0,
            end_frequency: 10.0,
            config,
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
        // access the values, that can be updated from the GUI, like this for now.
        // This is not pretty, but it works with the GUI this way....
        // the other values (filter_number, start_frequency, end_frequency) are still
        // just fields of this struct.

        let mut iteration = 0;
        if let ParameterKind::UInt(value) = self.config.parameters.get("Iterations").unwrap() {
            iteration = *value;
        }

        // Implement your Richardson-Lucy algorithm here
        // Get the psf with _gui_settings.psf
        // Iterate over the frequencies contained in the psf
        // Compute range_max_x and range_max_y with (w_x + |x_0|) * 3 and (w_y + |y_0|) * 3
        // Create two vectors x and y with range_max_x and range_max_y using the dx and dy steps from the scan
        // Create the 2D PSF for the given frequency
        // Filter the scan with the FIR filter of the given frequency
        // Perform the deconvolution with the Richardson-Lucy algorithm
        // etc.
    }

    fn config(&self) -> &FilterConfig {
        &self.config
    }

    fn config_mut(&mut self) -> &mut FilterConfig {
        &mut self.config
    }
}
