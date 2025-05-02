use crate::config::GuiThreadCommunication;
use crate::data_container::ScannedImage;
use crate::filters::filter::{Filter, FilterConfig, FilterDomain};
use crate::gui::application::GuiSettingsContainer;
use crate::math_tools::apply_adapted_blackman_window;
use eframe::egui::{self, Ui};
//use filter_macros::register_filter;
use ndarray::{concatenate, s, Array1, Array3, Axis};
use realfft::RealFftPlanner;
use std::f32::consts::PI;

#[derive(Debug)]
//#[register_filter]
pub struct TiltCompensation {
    pub tilt_x: f64,
    pub tilt_y: f64,
}

impl Filter for TiltCompensation {
    fn new() -> Self
    where
        Self: Sized,
    {
        TiltCompensation {
            tilt_x: 0.0,
            tilt_y: 0.0,
        }
    }

    fn config(&self) -> FilterConfig {
        FilterConfig {
            name: "Tilt Compensation".to_string(),
            domain: FilterDomain::Frequency,
        }
    }

    fn filter(&self, scan: &mut ScannedImage, _gui_settings: &mut GuiSettingsContainer) {
        // only rotation around the center are implemented, offset rotations are still to be done.
        let time_shift_x = self.tilt_x as f32 / 180.0 * PI;
        let time_shift_y = self.tilt_y as f32 / 180.0 * PI;

        if let (Some(dx), Some(dy)) = (scan.dx, scan.dy) {
            let (width, height, time_samples) = scan.raw_data.dim();
            let center_x = width as f32 / 2.0 * dx;
            let center_y = height as f32 / 2.0 * dy;
            let c = 0.299792458_f64; // mm/ps

            let dt = 0.05;

            // Compute extension and round it to the nearest step
            let max_offset_x = (center_x as f64 * time_shift_x.abs() as f64 / c) as f32;
            let max_offset_y = (center_y as f64 * time_shift_y.abs() as f64 / c) as f32;
            let extension = (max_offset_x + max_offset_y) / dt;
            let extension = extension.floor() * dt;

            // Clone the original time array
            let original_time = scan.time.clone();

            // Get first and last values
            let first_value = *original_time.first().unwrap();
            let last_value = *original_time.last().unwrap();

            // Compute number of steps for extension
            let num_steps = (extension / dt).round() as usize;
            let extended_samples = original_time.len() + num_steps * 2; // Extra steps on both sides

            // Generate extended time array
            let front_array =
                Array1::linspace(first_value - extension, first_value - dt, num_steps);
            let back_array = Array1::linspace(last_value + dt, last_value + extension, num_steps);
            scan.filtered_time = concatenate![Axis(0), front_array, original_time, back_array];

            // Create new filtered_data with extra time samples
            let mut new_filtered_data = Array3::zeros((width, height, extended_samples));
            for i in 0..width {
                for j in 0..height {
                    let x_offset = (((i as f32 - width as f32 / 2.0) * dx) as f64
                        * time_shift_x as f64
                        / c) as f32;
                    let y_offset = (((j as f32 - height as f32 / 2.0) * dy) as f64
                        * time_shift_y as f64
                        / c) as f32;
                    let delta = x_offset + y_offset;

                    let delta_steps = (delta / dt).floor() as isize;

                    let raw_trace = scan.raw_data.slice_mut(s![i, j, ..]);
                    let mut extended_trace = Array1::zeros(extended_samples);

                    let insert_index = (num_steps as isize + delta_steps).max(0) as usize; // Ensure non-negative index

                    // Fill before the trace with first value
                    extended_trace
                        .slice_mut(s![..insert_index])
                        .fill(*raw_trace.first().unwrap());

                    // Insert original trace
                    let end_index = (insert_index + time_samples).min(extended_samples);

                    let mut raw_trace_copy = raw_trace.to_owned(); // Create a mutable copy
                    let mut data_view = raw_trace_copy.view_mut(); // Obtain a mutable view

                    apply_adapted_blackman_window(
                        &mut data_view,
                        &original_time,
                        &0.0,
                        &7.0,
                    );
                    extended_trace
                        .slice_mut(s![insert_index..end_index])
                        .assign(&data_view.slice(s![..(end_index - insert_index)]));

                    // Fill after the trace with last value
                    extended_trace.slice_mut(s![end_index..]).fill(0.0);

                    // Assign extended trace to the new data
                    new_filtered_data
                        .slice_mut(s![i, j, ..])
                        .assign(&extended_trace);
                }
            }
            let n = scan.filtered_time.len();
            let rng = scan.filtered_time.last().unwrap() - scan.filtered_time.first().unwrap();

            let mut real_planner = RealFftPlanner::<f32>::new();
            let r2c = real_planner.plan_fft_forward(n);
            let c2r = real_planner.plan_fft_inverse(n);
            let spectrum = r2c.make_output_vec();
            let freq = (0..spectrum.len()).map(|i| i as f32 / rng).collect();
            scan.filtered_frequencies = freq;
            scan.filtered_r2c = Some(r2c);
            scan.filtered_c2r = Some(c2r);

            scan.filtered_data = new_filtered_data;

            dbg!(&scan.filtered_time.len());
            dbg!(&scan.filtered_data.shape());
        }
    }

    fn ui(
        &mut self,
        ui: &mut Ui,
        _thread_communication: &mut GuiThreadCommunication,
    ) -> egui::Response {
        let mut final_response = ui.allocate_response(egui::Vec2::ZERO, egui::Sense::hover());

        let response_x = ui
            .horizontal(|ui| {
                ui.label("Tilt X: ");
                ui.add(egui::Slider::new(&mut self.tilt_x, -15.0..=15.0).suffix(" deg"))
            })
            .inner; // Get the slider's response

        let response_y = ui
            .horizontal(|ui| {
                ui.label("Tilt Y: ");
                ui.add(egui::Slider::new(&mut self.tilt_y, -15.0..=15.0).suffix(" deg"))
            })
            .inner; // Get the slider's response

        // Merge responses to track interactivity
        final_response |= response_x.clone();
        final_response |= response_y.clone();

        // Only mark changed if any slider was changed (not just hovered)
        if response_x.changed() || response_y.changed() {
            final_response.mark_changed();
        }

        final_response
    }
}
