//! Time domain bandpass filter implementation that operates on data after iFFT processing.
//!
//! This filter allows users to zero out signal components outside a specified time window,
//! applying an adapted Blackman window to reduce spectral leakage at the window edges.

use crate::config::ThreadCommunication;
use crate::data_container::ScannedImageFilterData;
use crate::filters::filter::{CopyStaticFieldsTrait, Filter, FilterConfig, FilterDomain};
use crate::gui::application::GuiSettingsContainer;
use crate::math_tools::apply_adapted_blackman_window;
use bevy_egui::egui::{self, Ui};
use bevy_egui::egui::{DragValue, Stroke, Vec2};
use egui_double_slider::DoubleSlider;
use egui_plot::{Line, LineStyle, Plot, PlotPoints, VLine};
use filter_macros::{register_filter, CopyStaticFields};
use ndarray::{s, Array1};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, RwLock};

/// Time domain bandpass filter that operates after iFFT processing.
///
/// This filter allows selecting a specific time window in the signal,
/// zeroing out components outside this window and smoothing the edges
/// with a Blackman window to reduce spectral artifacts.
#[register_filter]
#[derive(Clone, Debug, CopyStaticFields)]
pub struct TimeDomainBandPassAfterFFT {
    /// Lower bound of the time window (start time)
    pub low: f64,
    /// Upper bound of the time window (end time)
    pub high: f64,
    /// Width parameter for the adaptive Blackman window applied at the edges
    pub window_width: f64,
    /// Time axis data for visualization in the UI
    #[static_field]
    time_axis: Vec<f32>,
    /// Output signal values for the currently selected pixel (after filtering)
    #[static_field]
    signal_axis: Vec<f32>,
    /// Input signal values for the currently selected pixel (before filtering)
    #[static_field]
    input_signal_axis: Vec<f32>,
}

impl Filter for TimeDomainBandPassAfterFFT {
    /// Creates a new instance of the time domain bandpass filter with default values
    fn new() -> Self
    where
        Self: Sized,
    {
        TimeDomainBandPassAfterFFT {
            low: 0.0,
            high: 0.0,
            window_width: 0.1,
            time_axis: vec![],
            signal_axis: vec![],
            input_signal_axis: vec![],
        }
    }

    /// Resets the filter state with new time axis data
    ///
    /// # Arguments
    /// * `time` - The time axis array
    /// * `_shape` - The shape of the data array (unused in this implementation)
    fn reset(&mut self, time: &Array1<f32>, _shape: &[usize]) {
        self.time_axis = time.to_vec();
        self.signal_axis = vec![0.0; self.time_axis.len()];
        self.input_signal_axis = vec![0.0; self.time_axis.len()];
        self.low = *time.first().unwrap_or(&0.0) as f64;
        self.high = *time.last().unwrap_or(&0.0) as f64;
    }

    /// Returns the filter's configuration and metadata
    fn config(&self) -> FilterConfig {
        FilterConfig {
            name: "Time Band Pass".to_string(),
            description: "Band-Pass Filter in Time Domain after the iFFT.".to_string(),
            hyperlink: None,
            domain: FilterDomain::TimeAfterFFT,
        }
    }

    /// Applies the bandpass filter to the input data
    ///
    /// This function:
    /// 1. Zeroes out signal components outside the specified time window
    /// 2. Applies a Blackman window to smooth transitions at window edges
    /// 3. Updates visualization data for the UI
    ///
    /// # Arguments
    /// * `input_data` - The input data to filter
    /// * `_gui_settings` - Container for GUI settings (unused in this implementation)
    /// * `progress_lock` - Shared lock for reporting progress
    /// * `_abort_flag` - Flag that can be used to abort processing (unused in this implementation)
    fn filter(
        &mut self,
        input_data: &ScannedImageFilterData,
        _gui_settings: &mut GuiSettingsContainer,
        progress_lock: &mut Arc<RwLock<Option<f32>>>,
        _abort_flag: &Arc<AtomicBool>,
    ) -> ScannedImageFilterData {
        let mut output_data = input_data.clone();
        let shape = output_data.data.dim();

        // Ensure high and low values are within the actual time range
        let min_time = *input_data.time.first().unwrap_or(&0.0);
        let max_time = *input_data.time.last().unwrap_or(&0.0);
        self.low = self.low.max(min_time as f64);
        self.high = self.high.min(max_time as f64);

        // Find indices corresponding to the frequency cutoffs (with bounds checking)
        let lower = input_data
            .time
            .iter()
            .position(|t| *t >= self.low as f32)
            .unwrap_or(0);
        let upper = input_data
            .time
            .iter()
            .position(|t| *t >= self.high as f32)
            .unwrap_or_else(|| input_data.time.len().saturating_sub(1));
        // Ensure upper is greater than lower and within bounds
        let upper = upper.max(lower + 1).min(input_data.time.len());

        // Apply the bandpass filter to the signal
        for i in 0..shape.0 {
            for j in 0..shape.1 {
                // Zero values before the lower bound
                if lower > 0 {
                    output_data.data.slice_mut(s![i, j, 0..lower]).fill(0.0);
                }
                // Zero values after the upper bound
                if upper < output_data.data.shape()[2] {
                    output_data.data.slice_mut(s![i, j, upper..]).fill(0.0);
                }

                let mut signal = output_data.data.slice_mut(s![i, j, lower..upper]);

                apply_adapted_blackman_window(
                    &mut signal,
                    &input_data.time.slice(s![lower..upper]).to_owned(),
                    &(self.window_width as f32),
                    &(self.window_width as f32),
                );
            }
        }

        // Store the time axis for UI visualization
        self.time_axis = output_data.time.to_vec();

        // Safely get the signal for the selected pixel with bounds checking
        let pixel = input_data.pixel_selected;
        if pixel[0] < shape.0 && pixel[1] < shape.1 {
            self.signal_axis = output_data.data.slice(s![pixel[0], pixel[1], ..]).to_vec();
        } else {
            // Use default values if pixel is out of bounds
            self.signal_axis = vec![0.0; output_data.time.len()];
        }

        self.input_signal_axis = input_data.data.slice(s![pixel[0], pixel[1], ..]).to_vec();

        if let Ok(mut p) = progress_lock.write() {
            *p = None;
        }

        output_data
    }

    /// Renders the filter's UI controls and visualization
    ///
    /// The UI includes:
    /// - A plot showing the original and filtered signals
    /// - A double-slider for adjusting the time window bounds
    /// - Numeric input fields for precise boundary values
    /// - Keyboard and mouse controls for interacting with the filter
    ///
    /// # Arguments
    /// * `ui` - The egui UI context to render into
    /// * `_thread_communication` - Communication channel with processing threads (unused here)
    /// * `panel_width` - Width of the panel in pixels, used for layout calculations
    fn ui(
        &mut self,
        ui: &mut Ui,
        _thread_communication: &mut ThreadCommunication,
        panel_width: f32,
    ) -> egui::Response {
        let mut final_response = ui.allocate_response(egui::Vec2::ZERO, egui::Sense::hover());

        // Constants for UI interaction sensitivity
        let zoom_factor = 5.0;
        let scroll_factor = 0.01;

        // Prepare data points for the signal plot
        let mut window_vals: Vec<[f64; 2]> = Vec::new();
        for i in 0..self.time_axis.len() {
            window_vals.push([self.time_axis[i] as f64, self.signal_axis[i] as f64]);
        }
        let mut input_signal: Vec<[f64; 2]> = Vec::new();
        for i in 0..self.time_axis.len() {
            input_signal.push([self.time_axis[i] as f64, self.input_signal_axis[i] as f64]);
        }

        // Create and configure the plot widget
        let time_window_plot = Plot::new("Time Window")
            .allow_drag(false)
            .set_margin_fraction(Vec2 { x: 0.0, y: 0.05 })
            .height(100.0)
            .allow_scroll(false)
            .allow_zoom(false)
            .width(panel_width * 0.9);

        // Show the plot with signal data and window bounds
        let ui_response = ui.vertical_centered(|ui| {
            time_window_plot.show(ui, |window_plot_ui| {
                // Plot the original input signal
                window_plot_ui.line(
                    Line::new(PlotPoints::from(input_signal))
                        .color(egui::Color32::RED)
                        .style(LineStyle::Solid)
                        .name("Input Pulse"),
                );
                // Plot the filtered signal
                window_plot_ui.line(
                    Line::new(PlotPoints::from(window_vals))
                        .color(egui::Color32::BLUE)
                        .style(LineStyle::Solid)
                        .name("Filtered Pulse"),
                );
                // Add vertical lines for the window bounds
                window_plot_ui.vline(
                    // TODO: adjust this
                    VLine::new(self.low)
                        .stroke(Stroke::new(1.0, egui::Color32::GRAY))
                        .name("Lower Bound"),
                );
                window_plot_ui.vline(
                    VLine::new(self.high)
                        .stroke(Stroke::new(1.0, egui::Color32::GRAY))
                        .name("Upper Bound"),
                );
            })
        });

        // Add hover text with usage instructions
        ui_response
            .response
            .on_hover_text(egui::RichText::new(format!(
                "{} Scroll and Zoom to adjust the sliders.",
                egui_phosphor::regular::INFO
            )));
        let plot_response = ui_response.inner;

        // Add the double slider for adjusting window bounds
        let slider_changed = ui.horizontal(|ui| {
            let right_offset = 0.09 * panel_width;
            let left_offset = 0.01 * panel_width;
            ui.add_space(left_offset);

            // Get current slider values and range limits
            let mut time_window_lower_bound = self.low;
            let mut time_window_upper_bound = self.high;
            let lower = *self.time_axis.first().unwrap_or(&1000.0) as f64;
            let upper = *self.time_axis.last().unwrap_or(&1050.0) as f64;

            // Add the double slider with proper configuration
            let slider = ui
                .add(
                    DoubleSlider::new(
                        &mut time_window_lower_bound,
                        &mut time_window_upper_bound,
                        lower..=upper,
                    )
                    .vertical_scroll(false)
                    .zoom_factor(zoom_factor)
                    .separation_distance(1.0)
                    .width(panel_width - left_offset - right_offset),
                )
                .on_hover_text(egui::RichText::new(format!(
                    "{} Scroll and Zoom to adjust the sliders. Double Click to reset.",
                    egui_phosphor::regular::INFO
                )));

            let slider_changed = slider.changed();

            // Reset to full range on double-click
            if slider.double_clicked() {
                time_window_lower_bound = lower;
                time_window_upper_bound = upper;
            }

            // Update filter parameters with slider values
            self.low = time_window_lower_bound;
            self.high = time_window_upper_bound;
            slider_changed
        });

        // Add numeric input fields for precise boundary control
        ui.horizontal(|ui| {
            let val1_changed = ui.add(DragValue::new(&mut self.low)).changed();

            ui.add_space(0.75 * panel_width);

            let val2_changed = ui.add(DragValue::new(&mut self.high)).changed();

            // Reset if values are invalid (equal)
            if slider_changed.inner || val1_changed || val2_changed {
                if self.low == self.high {
                    self.low = *self.time_axis.first().unwrap_or(&1000.0) as f64;
                    self.high = *self.time_axis.last().unwrap_or(&1050.0) as f64;
                }
                final_response.mark_changed();
            }
        });

        // Implement keyboard and scroll navigation for the time window
        if plot_response.response.hovered() {
            let width = self.high - self.low;
            let first = *self.time_axis.first().unwrap_or(&1000.0) as f64;
            let last = *self.time_axis.last().unwrap_or(&1050.0) as f64;

            // Arrow key navigation
            if ui.input(|i| i.key_pressed(egui::Key::ArrowRight)) && self.high < last {
                self.low += 1.0;
                self.high = width + self.low;
                final_response.mark_changed();
            }

            if ui.input(|i| i.key_pressed(egui::Key::ArrowLeft)) && self.low > first {
                self.low -= 1.0;
                self.high = width + self.low;
                final_response.mark_changed();
            }

            // Mouse scroll for panning
            let scroll_delta = ui.ctx().input(|i| i.smooth_scroll_delta);
            self.high += scroll_delta.x as f64 * scroll_factor as f64;
            self.low += scroll_delta.x as f64 * scroll_factor as f64;

            // Mouse zoom for adjusting window width
            let zoom_delta = ui.ctx().input(|i| i.zoom_delta() - 1.0);
            self.high += zoom_delta as f64 * zoom_factor as f64;
            self.low -= zoom_delta as f64 * zoom_factor as f64;

            if scroll_delta.x != 0.0 || zoom_delta != 0.0 {
                final_response.mark_changed();
            }
        }

        // Combine responses
        final_response |= plot_response.response.clone();
        final_response |= slider_changed.response.clone();

        // Mark the response as changed if any interaction occurred
        if plot_response.response.changed() || slider_changed.inner {
            final_response.mark_changed();
        }

        final_response
    }
}
