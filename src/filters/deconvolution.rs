//! This module implements a custom filter named `Deconvolution`, which operates on scanned images
//! and performs a deconvolution operation in the frequency domain.
//!
//! The implementation includes a Richardson-Lucy deconvolution algorithm placeholder,
//! allowing for further customization and parameterization.

use crate::config::GuiThreadCommunication;
use crate::data_container::ScannedImage;
use crate::filters::filter::{Filter, FilterConfig, FilterDomain};
use crate::gui::application::GuiSettingsContainer;
use eframe::egui::{self, Ui};
use filter_macros::register_filter;
use crate::filters::psf::gaussian2;
use crate::filters::psf::create_psf_2d;
use ndarray_ndimage::{convolve, convolve1d};
use ndarray::{Array1, Array2, Array3, s, Zip, Axis};

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
    pub n_iterations: usize,
    pub filter_number: usize,
    pub start_frequency: f64,
    pub end_frequency: f64,
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

    fn config(&self) -> FilterConfig {
        FilterConfig {
            name: "Deconvolution".to_string(),
            domain: FilterDomain::Frequency,
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
    fn filter(&self, scan: &mut ScannedImage, gui_settings: &mut GuiSettingsContainer) {
        // Implement your Richardson-Lucy algorithm here
        // Iterate over the frequencies/filters contained in the psf
        for (i, &filter) in gui_settings.psf.filters.iter() {
            // Compute range_max_x and range_max_y with (w_x + |x_0|) * 3 and (w_y + |y_0|) * 3
            let mut range_max_x: f64 = (gui_settings.psf.popt_x[1] + gui_settings.psf.popt_x[0].abs()) * 3.0;
            let mut range_max_y: f64 = (gui_settings.psf.popt_y[1] + gui_settings.psf.popt_y[0].abs()) * 3.0;
            // Compute the minimum range_max_x and range_max_y
            // wmin is set to 2.5 to avoid deconvolution errors
            let wmin: f64 = 2.5;
            range_max_x = self.range_max_min(range_max_x, &wmin);
            range_max_y = self.range_max_min(range_max_y, &wmin);
            // Round the range_max_x and range_max_y to the nearest dx and dy steps
            range_max_x = (range_max_x / scan.dx).floor() * scan.dx + scan.dx;
            range_max_y = (range_max_y / scan.dy).floor() * scan.dy + scan.dy;
            // Create two vectors x and y with range_max_x and range_max_y using the dx and dy steps from the scan
            let x: Vec<f64> = (-((range_max_x / scan.dx).floor() as isize)..=((range_max_x / scan.dx).floor() as isize))
                                .map(|i| i as f64 * scan.dx)
                                .collect();
            let y: Vec<f64> = (-((range_max_y / scan.dy).floor() as isize)..=((range_max_y / scan.dy).floor() as isize))
                                .map(|i| i as f64 * scan.dy)
                                .collect();
            // Create the x and y psfs
            let gaussian_x: Vec<f64> = gaussian2(&x, &gui_settings.psf.popt_x);
            let gaussian_y: Vec<f64> = gaussian2(&y, &gui_settings.psf.popt_y);
            // Create the 2D PSF
            let psf_2d: Array2<f64> =  create_psf_2d(&gaussian_x, &gaussian_y, &x, &y, &scan.dx, &scan.dy);
            // Filter the scan with the FIR filter of the given frequency
            let filtered_data: Array3<f32> = self.filter_scan(scan, &psf_2d);
            // Computing the filtered image by summing the squared samples for each pixel
            let mut filtered_image: Array2<f32> = Array2::zeros((scan.raw_data.dim().0, scan.raw_data.dim().1));
            for i in 0..scan.raw_data.dim().0 {
                for j in 0..scan.raw_data.dim().1 {
                    filtered_image[(i, j)] = filtered_data.slice(s![i, j, ..]).iter().map(|x| x.powi(2)).sum();
                }
            }

        }

        // Perform the deconvolution with the Richardson-Lucy algorithm
        // etc.
    }
    fn ui(&mut self, ui: &mut Ui, _thread_communication: &mut GuiThreadCommunication) {
        // thread_communication can be used, but is not required. It contains the gui_settings GuiSettingsContainer
        // implement your GUI parameter handling here, for example like this:
        ui.horizontal(|ui| {
            ui.label("Iterations: ");
            ui.add(egui::Slider::new(&mut self.n_iterations, 0..=10));
        });
    }
}
