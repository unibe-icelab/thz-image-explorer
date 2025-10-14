//! Tilt compensation filter for correcting sample misalignment.
//!
//! This filter corrects for physical misalignment of samples by applying position-dependent
//! time shifts to the data. When samples are tilted relative to the scanning plane,
//! different parts of the sample are at different optical path lengths, causing
//! timing offsets in the measured signals. This filter compensates for these effects
//! by calculating and applying the appropriate time shifts based on specified tilt angles.

use crate::config::ThreadCommunication;
use crate::data_container::ScannedImageFilterData;
use crate::filters::filter::{CopyStaticFieldsTrait, Filter, FilterConfig, FilterDomain};
use crate::gui::application::GuiSettingsContainer;
use crate::math_tools::apply_adapted_blackman_window;
use bevy_egui::egui::{self, Ui};
use filter_macros::{register_filter, CopyStaticFields};
use ndarray::{concatenate, s, Array1, Array3, Axis};
use realfft::RealFftPlanner;
use std::f32::consts::PI;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, RwLock};

/// Tilt compensation filter for correcting sample misalignment along X and Y axes.
///
/// This filter applies position-dependent time shifts to compensate for sample tilt.
/// When a sample is tilted, different positions on the sample are at different distances
/// from the detector, causing time delays in the signal. This filter corrects these
/// delays based on specified tilt angles.
#[register_filter]
#[derive(Clone, Debug, CopyStaticFields)]
pub struct TiltCompensation {
    /// Tilt angle around the X axis in degrees (-15 to 15 degrees)
    pub tilt_x: f64,
    /// Tilt angle around the Y axis in degrees (-15 to 15 degrees)
    pub tilt_y: f64,
}

impl Filter for TiltCompensation {
    /// Creates a new instance of the tilt compensation filter with default values (no tilt)
    fn new() -> Self
    where
        Self: Sized,
    {
        TiltCompensation {
            tilt_x: 0.0,
            tilt_y: 0.0,
        }
    }

    /// No special reset operation needed for this filter. Not used in this implementation.
    ///
    /// # Arguments
    /// * `_time` - The time axis array (unused in this implementation)
    /// * `_shape` - The shape of the data array (unused in this implementation)
    fn reset(&mut self, _time: &Array1<f32>, _shape: &[usize]) {
        // NOOP
    }

    /// Updates the filter's GUI data with new data. Not used in this implementation.
    ///
    /// # Arguments
    /// * `_data` - The scanned image filter data containing the signal and time axis
    ///
    fn show_data(&mut self, _data: &ScannedImageFilterData) {
        // NOOP
    }

    /// Returns the filter's configuration and metadata
    fn config(&self) -> FilterConfig {
        FilterConfig {
            name: "Tilt Compensation".to_string(),
            description: "Compensate any misalignment of the sample along x and y axis."
                .to_string(),
            hyperlink: None,
            domain: FilterDomain::TimeBeforeFFTPrioFirst,
        }
    }

    /// Applies tilt compensation to the input data
    ///
    /// This function:
    /// 1. Converts tilt angles to radians
    /// 2. Calculates position-dependent time shifts for each pixel
    /// 3. Extends the time range to accommodate the shifted data
    /// 4. Applies appropriate time shifts to each pixel's time trace
    /// 5. Updates FFT parameters for the extended data
    ///
    /// The time shift for each pixel is calculated based on:
    /// - Its distance from the center of the scan area
    /// - The tilt angles specified for X and Y axes
    /// - The speed of light (c = 0.3 mm/ps)
    ///
    /// # Arguments
    /// * `input_data` - The input data to process
    /// * `_gui_settings` - Container for GUI settings (unused in this implementation)
    /// * `_progress_lock` - Progress reporting lock (unused in this implementation)
    /// * `_abort_flag` - Flag for aborting processing (unused in this implementation)
    fn filter(
        &mut self,
        input_data: &ScannedImageFilterData,
        _gui_settings: &mut GuiSettingsContainer,
        _progress_lock: &mut Arc<RwLock<Option<f32>>>,
        _abort_flag: &Arc<AtomicBool>,
    ) -> ScannedImageFilterData {
        // Convert tilt angles from degrees to radians
        let time_shift_x = self.tilt_x as f32 / 180.0 * PI;
        let time_shift_y = self.tilt_y as f32 / 180.0 * PI;

        let mut output_data = input_data.clone();

        // Only proceed if we have spatial resolution information
        if let (Some(dx), Some(dy)) = (input_data.dx, input_data.dy) {
            let (width, height, time_samples) = input_data.data.dim();

            // Calculate center position for reference
            let center_x = width as f32 / 2.0 * dx;
            let center_y = height as f32 / 2.0 * dy;

            // Speed of light in mm/ps
            let c = 0.299792458_f64;

            // Time step for calculations
            let dt = 0.05;

            // Calculate maximum time offsets based on the sample dimensions and tilt angles
            let max_offset_x = (center_x as f64 * time_shift_x.abs() as f64 / c) as f32;
            let max_offset_y = (center_y as f64 * time_shift_y.abs() as f64 / c) as f32;
            let extension = (max_offset_x + max_offset_y) / dt;
            let extension = extension.floor() * dt;

            // Get the original time array
            let original_time = input_data.time.clone();

            if original_time.is_empty() {
                log::warn!("scan time is empty, cannot update voxel plot instances");
                return output_data;
            }

            // Get boundary values for time extension
            let first_value = *original_time.first().unwrap();
            let last_value = *original_time.last().unwrap();

            // Calculate the number of additional time steps needed
            let num_steps = (extension / dt).round() as usize;
            let extended_samples = original_time.len() + num_steps * 2; // Extend both before and after

            // Create extended time array by concatenating time segments
            let front_array =
                Array1::linspace(first_value - extension, first_value - dt, num_steps);
            let back_array = Array1::linspace(last_value + dt, last_value + extension, num_steps);
            output_data.time = concatenate![Axis(0), front_array, original_time, back_array];

            // Create new data array with the extended time dimension
            let mut new_filtered_data = Array3::zeros((width, height, extended_samples));

            // Process each pixel
            for i in 0..width {
                for j in 0..height {
                    // Calculate the position-dependent time shift for this pixel
                    let x_offset = (((i as f32 - width as f32 / 2.0) * dx) as f64
                        * time_shift_x as f64
                        / c) as f32;
                    let y_offset = (((j as f32 - height as f32 / 2.0) * dy) as f64
                        * time_shift_y as f64
                        / c) as f32;
                    let delta = x_offset + y_offset;

                    // Convert time shift to discrete steps
                    let delta_steps = (delta / dt).floor() as isize;

                    // Get the original time trace for this pixel
                    let raw_trace = output_data.data.slice_mut(s![i, j, ..]);
                    let mut extended_trace = Array1::zeros(extended_samples);

                    // Calculate where to insert the original trace in the extended array
                    let insert_index = (num_steps as isize + delta_steps).max(0) as usize;

                    // Fill the beginning of the extended trace
                    extended_trace
                        .slice_mut(s![..insert_index])
                        .fill(*raw_trace.first().unwrap());

                    // Calculate end index, ensuring we don't exceed array bounds
                    let end_index = (insert_index + time_samples).min(extended_samples);

                    // Apply Blackman window to smooth the signal and reduce artifacts
                    let mut raw_trace_copy = raw_trace.to_owned();
                    let mut data_view = raw_trace_copy.view_mut();
                    apply_adapted_blackman_window(&mut data_view, &original_time, &0.0, &7.0);

                    // Insert the windowed trace into the extended array
                    extended_trace
                        .slice_mut(s![insert_index..end_index])
                        .assign(&data_view.slice(s![..(end_index - insert_index)]));

                    // Fill the end of the extended trace with zeros
                    extended_trace.slice_mut(s![end_index..]).fill(0.0);

                    // Assign the extended trace to the new data array
                    new_filtered_data
                        .slice_mut(s![i, j, ..])
                        .assign(&extended_trace);
                }
            }

            // Update FFT parameters for the extended data
            let n = output_data.time.len();
            let rng = output_data.time.last().unwrap() - output_data.time.first().unwrap();

            // Create new FFT planner and transforms for the extended data
            let mut real_planner = RealFftPlanner::<f32>::new();
            let r2c = real_planner.plan_fft_forward(n);
            let c2r = real_planner.plan_fft_inverse(n);
            let spectrum = r2c.make_output_vec();

            // Calculate new frequency axis
            let freq = (0..spectrum.len()).map(|i| i as f32 / rng).collect();
            output_data.frequency = freq;
            output_data.r2c = Some(r2c);
            output_data.c2r = Some(c2r);

            // Assign the processed data to the output
            output_data.data = new_filtered_data;
        }

        output_data
    }

    /// Renders the filter's UI controls
    ///
    /// Provides sliders for adjusting the X and Y tilt angles, with a range of
    /// -15 to 15 degrees for each axis.
    ///
    /// # Arguments
    /// * `ui` - The egui UI context to render into
    /// * `_thread_communication` - Communication channel with processing threads (unused here)
    /// * `_panel_width` - Width of the panel in pixels (unused in this implementation)
    fn ui(
        &mut self,
        ui: &mut Ui,
        _thread_communication: &mut ThreadCommunication,
        _panel_width: f32,
    ) -> egui::Response {
        let mut final_response = ui.allocate_response(egui::Vec2::ZERO, egui::Sense::hover());

        // Slider for X-axis tilt adjustment
        let response_x = ui
            .horizontal(|ui| {
                ui.label("Tilt X: ");
                ui.add(egui::Slider::new(&mut self.tilt_x, -15.0..=15.0).suffix(" deg"))
            })
            .inner;

        // Slider for Y-axis tilt adjustment
        let response_y = ui
            .horizontal(|ui| {
                ui.label("Tilt Y: ");
                ui.add(egui::Slider::new(&mut self.tilt_y, -15.0..=15.0).suffix(" deg"))
            })
            .inner;

        // Combine responses for UI interactions
        final_response |= response_x.clone();
        final_response |= response_y.clone();

        // Mark as changed only if slider values actually changed
        if response_x.changed() || response_y.changed() {
            final_response.mark_changed();
        }

        final_response
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::{s, Array1, Array3};
    use std::sync::atomic::AtomicBool;
    use std::sync::{Arc, RwLock};

    fn compute_expected_num_steps(
        width: usize,
        height: usize,
        dx: f32,
        dy: f32,
        tilt_x: f64,
        tilt_y: f64,
    ) -> usize {
        let center_x = width as f32 / 2.0 * dx;
        let center_y = height as f32 / 2.0 * dy;
        let time_shift_x = tilt_x as f32 / 180.0 * std::f32::consts::PI;
        let time_shift_y = tilt_y as f32 / 180.0 * std::f32::consts::PI;
        let c = 0.299_792_458_f64;
        let dt = 0.05_f32;

        let max_offset_x = (center_x as f64 * time_shift_x.abs() as f64 / c) as f32;
        let max_offset_y = (center_y as f64 * time_shift_y.abs() as f64 / c) as f32;
        let extension = ((max_offset_x + max_offset_y) / dt).floor() * dt;
        ((extension / dt).round()) as usize
    }

    #[test]
    fn test_tilt_compensation_filter_extends_time_and_shifts_center_trace() {
        // 2x2 image; center pixel is (1,1). Single impulse at t = 10.
        let n = 64usize;
        let dt = 0.05f32;
        let width = 2usize;
        let height = 2usize;
        let impulse_idx = 10usize;

        let mut data = Array3::<f32>::zeros((width, height, n));
        data[[1, 1, impulse_idx]] = 1.0;

        let time = Array1::linspace(0.0, dt * (n as f32 - 1.0), n);

        let mut input = ScannedImageFilterData::default();
        input.time = time;
        input.data = data;

        input.dx = Some(1.0);
        input.dy = Some(1.0);

        let mut filt = TiltCompensation {
            tilt_x: 10.0,
            tilt_y: 0.0,
        };
        let mut gui = GuiSettingsContainer::new();
        let mut progress = Arc::new(RwLock::new(None));
        let abort = Arc::new(AtomicBool::new(false));

        let output = filt.filter(&input, &mut gui, &mut progress, &abort);

        // Expect extension and corresponding shift for the center pixel
        let expected_steps = compute_expected_num_steps(width, height, 1.0, 1.0, 10.0, 0.0);
        assert_eq!(output.time.len(), n + 2 * expected_steps);

        // Center pixel has zero geometric shift; impulse moves by +expected_steps
        let center_trace = output.data.slice(s![1, 1, ..]);
        let peak_idx = center_trace
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .map(|(i, _)| i)
            .unwrap();
        assert_eq!(peak_idx, impulse_idx + expected_steps);
    }

    #[test]
    fn test_tilt_compensation_filter_no_tilt_no_extension() {
        let n = 64usize;
        let dt = 0.05f32;
        let width = 2usize;
        let height = 2usize;
        let impulse_idx = 12usize;

        let mut data = Array3::<f32>::zeros((width, height, n));
        data[[1, 1, impulse_idx]] = 1.0;

        let time = Array1::linspace(0.0, dt * (n as f32 - 1.0), n);

        let mut input = ScannedImageFilterData::default();
        input.time = time;
        input.data = data;

        input.dx = Some(1.0);
        input.dy = Some(1.0);

        let mut filt = TiltCompensation {
            tilt_x: 0.0,
            tilt_y: 0.0,
        };
        let mut gui = GuiSettingsContainer::new();
        let mut progress = Arc::new(RwLock::new(None));
        let abort = Arc::new(AtomicBool::new(false));

        let output = filt.filter(&input, &mut gui, &mut progress, &abort);

        // No tilt => no extension and impulse index unchanged
        assert_eq!(output.time.len(), n);

        let center_trace = output.data.slice(s![1, 1, ..]);
        let peak_idx = center_trace
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .map(|(i, _)| i)
            .unwrap();
        assert_eq!(peak_idx, impulse_idx);
    }
}
