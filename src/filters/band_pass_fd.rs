//! Frequency domain bandpass filter implementation that operates on FFT data.
//!
//! This filter allows users to isolate specific frequency ranges by applying
//! an adaptive window function, effectively removing spectral components outside
//! the specified band while maintaining smooth transitions at the cutoffs.

use crate::config::ThreadCommunication;
use crate::data_container::ScannedImageFilterData;
use crate::filters::filter::{CopyStaticFieldsTrait, Filter, FilterConfig, FilterDomain};
use crate::gui::application::GuiSettingsContainer;
use crate::math_tools::apply_adapted_blackman_window;
use bevy_egui::egui::{self, Ui};
use bevy_egui::egui::{DragValue, Stroke, Vec2};
use cancellable_loops::par_for_each_cancellable;
use egui_double_slider::DoubleSlider;
use egui_plot::{Line, LineStyle, Plot, PlotPoints, VLine};
use filter_macros::{register_filter, CopyStaticFields};
use ndarray::{s, Array1};
use num_traits::Float;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, RwLock};

/// Frequency domain bandpass filter that operates on FFT data.
///
/// This filter allows selecting a specific frequency band in the spectrum,
/// zeroing out components outside this band and smoothing the edges
/// with a Blackman window to reduce spectral artifacts.
#[register_filter]
#[derive(Clone, Debug, CopyStaticFields)]
pub struct FrequencyDomainBandPass {
    /// Lower bound of the frequency window (low cutoff in frequency units)
    pub low: f64,
    /// Upper bound of the frequency window (high cutoff in frequency units)
    pub high: f64,
    /// Width parameter for the adaptive Blackman window applied at the edges
    pub window_width: f64,
    /// Frequency axis data for visualization in the UI
    #[static_field]
    freq_axis: Vec<f32>,
    /// Signal spectrum values for the currently selected pixel (after filtering)
    #[static_field]
    signal_axis: Vec<f32>,
}

impl Filter for FrequencyDomainBandPass {
    /// Creates a new instance of the frequency domain bandpass filter with default values
    fn new() -> Self
    where
        Self: Sized,
    {
        FrequencyDomainBandPass {
            low: 0.2,
            high: 5.0,
            window_width: 0.1,
            freq_axis: vec![],
            signal_axis: vec![],
        }
    }

    /// No special reset operation needed for this filter
    ///
    /// # Arguments
    /// * `_time` - The time axis array (unused in this implementation)
    /// * `_shape` - The shape of the data array (unused in this implementation)
    fn reset(&mut self, _time: &Array1<f32>, _shape: &[usize]) {
        // NOOP
    }

    /// Updates the filter's GUI data with new data
    ///
    /// # Arguments
    /// * `data` - The scanned image filter data containing the signal and time axis
    ///
    fn show_data(&mut self, data: &ScannedImageFilterData) {
        // Extract spectrum for the selected pixel (for visualization)

        let shape = data.fft.dim();

        if shape.0 == 0 || shape.1 == 0 || shape.2 == 0 {
            // Dataset is empty, return early or handle gracefully
            return;
        }

        let h = shape.0;
        let w = shape.1;

        // Store the frequency axis for UI visualization
        self.freq_axis = data.frequency.to_vec();

        let pixel = data.pixel_selected;
        if pixel[0] < h && pixel[1] < w {
            let spectrum = data.fft.slice(s![pixel[0], pixel[1], ..]);
            self.signal_axis = spectrum.iter().map(|c| c.norm()).collect();
        } else {
            self.signal_axis = vec![0.0; data.frequency.len()];
        }
    }

    /// Returns the filter's configuration and metadata
    fn config(&self) -> FilterConfig {
        FilterConfig {
            name: "Frequency Band Pass".to_string(),
            description: "Band Pass Filter in Frequency Domain.".to_string(),
            hyperlink: None,
            domain: FilterDomain::Frequency,
        }
    }

    /// Applies the frequency domain bandpass filter to the input data
    ///
    /// This function:
    /// 1. Extracts the frequency band defined by the cutoff frequencies
    /// 2. Applies a Blackman window to smooth transitions at band edges
    /// 3. Zero-pads the result to maintain the original frequency domain size
    /// 4. Updates visualization data for the UI
    ///
    /// # Arguments
    /// * `input_data` - The input data to filter
    /// * `_gui_settings` - Container for GUI settings (unused in this implementation)
    /// * `progress_lock` - Shared lock for reporting progress
    /// * `abort_flag` - Flag that can be used to abort processing
    fn filter(
        &mut self,
        input_data: &ScannedImageFilterData,
        _gui_settings: &mut GuiSettingsContainer,
        progress_lock: &mut Arc<RwLock<Option<f32>>>,
        abort_flag: &Arc<AtomicBool>,
    ) -> ScannedImageFilterData {
        let mut output_data = input_data.clone();
        let shape = output_data.fft.dim();
        let h = shape.0;
        let w = shape.1;

        // Ensure cutoff frequencies are within the valid range
        let safe_low = self.low.max(0.0) as f32;
        let safe_high = self
            .high
            .min(output_data.frequency.last().copied().unwrap_or(10.0) as f64)
            as f32;

        // Find indices corresponding to the frequency cutoffs
        let lower = output_data
            .frequency
            .iter()
            .position(|&f| f >= safe_low)
            .unwrap_or(0);
        let upper = output_data
            .frequency
            .iter()
            .rposition(|&f| f <= safe_high)
            .map(|i| i + 1) // rposition is inclusive, so add 1 for slicing
            .unwrap_or(output_data.frequency.len());

        // Extract the frequency band of interest
        output_data.fft = input_data.fft.slice(s![.., .., lower..upper]).to_owned();
        output_data.amplitudes = input_data
            .amplitudes
            .slice(s![.., .., lower..upper])
            .to_owned();

        // Create and apply window function to smooth transitions
        let mut freq_window = Array1::<f32>::ones(upper - lower);
        apply_adapted_blackman_window(
            &mut freq_window.view_mut(),
            &input_data.frequency.slice(s![lower..upper]).to_owned(),
            &(self.window_width as f32),
            &(self.window_width as f32),
        );

        // Wrap arrays in Mutex for thread-safe parallel processing
        let fft = Mutex::new(&mut output_data.fft);
        let amplitudes = Mutex::new(&mut output_data.amplitudes);

        // Apply window to all pixels in parallel with cancellation support
        par_for_each_cancellable(0..h, abort_flag, |i| {
            for j in 0..w {
                let mut fft_guard = fft.lock().unwrap();
                let mut spectrum = fft_guard.slice_mut(s![i, j, ..]);

                let mut amp_guard = amplitudes.lock().unwrap();
                let mut amplitudes_slice = amp_guard.slice_mut(s![i, j, ..]);

                // Apply window to both complex and amplitude data
                for k in 0..freq_window.len() {
                    spectrum[k] = spectrum[k] * freq_window[k];
                    amplitudes_slice[k] = amplitudes_slice[k] * freq_window[k];
                }
            }
        });

        let original_freq_len = input_data.frequency.len();

        // Zero-pad the filtered FFT back to the original frequency length
        let mut padded_fft = ndarray::Array3::zeros((h, w, original_freq_len));
        let mut padded_amplitudes = ndarray::Array3::zeros((h, w, original_freq_len));

        // Copy the filtered data to the appropriate position in the padded arrays
        for i in 0..h {
            for j in 0..w {
                let n_data = output_data.fft.shape()[2];
                let filtered = output_data.fft.slice(s![i, j, ..]);
                let filtered_amp = output_data.amplitudes.slice(s![i, j, ..]);
                padded_fft
                    .slice_mut(s![i, j, lower..n_data + lower])
                    .assign(&filtered);
                padded_amplitudes
                    .slice_mut(s![i, j, lower..n_data + lower])
                    .assign(&filtered_amp);
            }
        }
        output_data.fft = padded_fft;
        output_data.amplitudes = padded_amplitudes;

        // Clear progress indicator when complete
        if let Ok(mut p) = progress_lock.write() {
            *p = None;
        }

        output_data
    }

    /// Renders the filter's UI controls and visualization
    ///
    /// The UI includes:
    /// - A plot showing the frequency spectrum and filter window
    /// - A double-slider for adjusting the frequency band cutoffs
    /// - Numeric input fields for precise cutoff values
    /// - Interactive controls for keyboard and mouse navigation
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
        let mut final_response = ui.allocate_response(Vec2::ZERO, egui::Sense::hover());

        // Constants for UI interaction sensitivity
        let zoom_factor = 0.5;
        let scroll_factor = 0.005;

        // Create frequency spectrum visualization
        let mut spectrum_vals: Vec<[f64; 2]> = Vec::new();
        for (i, freq) in self.freq_axis.iter().enumerate() {
            if i < self.signal_axis.len() {
                let amplitude = self.signal_axis[i];
                spectrum_vals.push([*freq as f64, amplitude as f64]);
            }
        }

        // Calculate max amplitude for scaling
        let max = spectrum_vals.iter().fold(0.0, |acc, &[_, y]| acc.max(y));

        // Create filter band visualization (rectangular shape)
        let mut filter_vals: Vec<[f64; 2]> = Vec::new();
        for freq in self.freq_axis.iter() {
            let amplitude = if *freq as f64 >= self.low && *freq as f64 <= self.high {
                max
            } else {
                0.0
            };
            filter_vals.push([*freq as f64, amplitude]);
        }

        // Generate the frequency window for visualization (Blackman shape)
        let mut freq_window = ndarray::Array1::<f32>::zeros(self.freq_axis.len());
        let safe_low = self.low.max(0.0) as f32;
        let safe_high =
            self.high
                .min(self.freq_axis.last().copied().unwrap_or(10.0) as f64) as f32;
        let freq_axis_arr = ndarray::Array1::from(self.freq_axis.clone());
        apply_adapted_blackman_window(
            &mut freq_window.view_mut(),
            &freq_axis_arr,
            &(safe_low - self.window_width as f32),
            &(safe_high + self.window_width as f32),
        );

        // Scale the window to match the max amplitude for visualization
        let window_line: Vec<[f64; 2]> = self
            .freq_axis
            .iter()
            .zip(freq_window.iter())
            .map(|(&f, &w)| [f as f64, w as f64 * max])
            .collect();

        // Create and configure the frequency domain plot
        let freq_plot = Plot::new("Frequency Domain")
            .allow_drag(false)
            .allow_zoom(false)
            .allow_scroll(false)
            .include_x(0.0)
            .include_x(10.0)
            .set_margin_fraction(Vec2 { x: 0.0, y: 0.05 })
            .height(100.0)
            .width(panel_width * 0.9);

        // Render the plot with spectrum, filter shape and cutoff lines
        let ui_response = ui.vertical_centered(|ui| {
            freq_plot.show(ui, |plot_ui| {
                // Plot the spectrum
                plot_ui.line(
                    Line::new(PlotPoints::from(spectrum_vals))
                        .color(egui::Color32::RED)
                        .style(LineStyle::Solid)
                        .name("Spectrum"),
                );

                // Plot the rectangular filter shape
                plot_ui.line(
                    Line::new(PlotPoints::from(filter_vals))
                        .color(egui::Color32::BLUE)
                        .style(LineStyle::Solid)
                        .name("Filter"),
                );

                // Add vertical lines for cutoffs
                plot_ui.vline(
                    VLine::new(self.low)
                        .stroke(Stroke::new(1.0, egui::Color32::GRAY))
                        .name("Low Cutoff"),
                );
                plot_ui.vline(
                    VLine::new(self.high)
                        .stroke(Stroke::new(1.0, egui::Color32::GRAY))
                        .name("High Cutoff"),
                );

                // Plot the actual window function shape
                plot_ui.line(
                    Line::new(PlotPoints::from(window_line))
                        .style(LineStyle::Solid)
                        .stroke(Stroke::new(1.0, egui::Color32::WHITE))
                        .name("Window"),
                );
            })
        });

        // Add tooltip with usage instructions
        ui_response.response.on_hover_text(
            "Frequency domain bandpass filter. Adjust the sliders to set cutoff frequencies.",
        );
        let plot_response = ui_response.inner;

        // Add the double slider for adjusting frequency cutoffs
        let slider_changed = ui.horizontal(|ui| {
            let right_offset = 0.09 * panel_width;
            let left_offset = 0.01 * panel_width;
            ui.add_space(left_offset);

            // Get current slider values and range limits
            let mut freq_lower_bound = self.low;
            let mut freq_upper_bound = self.high;
            let max_freq = *self.freq_axis.last().unwrap_or(&10.0) as f64;

            // Add the double slider with proper configuration
            let slider = ui
                .add(
                    DoubleSlider::new(&mut freq_lower_bound, &mut freq_upper_bound, 0.0..=max_freq)
                        .vertical_scroll(false)
                        .zoom_factor(zoom_factor)
                        .scroll_factor(scroll_factor)
                        .separation_distance(0.1)
                        .width(panel_width - left_offset - right_offset),
                )
                .on_hover_text(
                    "Scroll and zoom to adjust the frequency range. Double-click to reset.",
                );

            let slider_changed = slider.changed();

            // Reset to default values on double-click
            if slider.double_clicked() {
                freq_lower_bound = 0.5;
                freq_upper_bound = max_freq * 0.5;
            }

            // Update filter parameters with slider values
            self.low = freq_lower_bound;
            self.high = freq_upper_bound;
            slider_changed
        });

        // Add numeric input fields for precise cutoff control
        ui.horizontal(|ui| {
            let val1_changed = ui
                .add(
                    DragValue::new(&mut self.low)
                        .suffix(" THz")
                        .speed(0.01)
                        .range(0.0..=self.high),
                )
                .changed();

            let val2_changed = ui
                .with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(20.0);
                    ui.add(
                        DragValue::new(&mut self.high)
                            .suffix(" THz")
                            .speed(0.01)
                            .range(self.low..=*self.freq_axis.last().unwrap_or(&10.0) as f64),
                    )
                    .changed()
                })
                .inner;

            // Ensure cutoffs don't overlap
            if slider_changed.inner || val1_changed || val2_changed {
                if self.low == self.high {
                    self.low = self.low.max(0.1);
                    self.high = self.high + 0.1;
                }
                final_response.mark_changed();
            }
        });

        // Implement keyboard and scroll navigation for the frequency band
        if plot_response.response.hovered() {
            // Arrow key navigation
            let max_freq = *self.freq_axis.last().unwrap_or(&10.0) as f64;
            if ui.input(|i| i.key_pressed(egui::Key::ArrowRight)) && self.high < max_freq {
                self.low += 0.1;
                self.high += 0.1;
                final_response.mark_changed();
            }

            if ui.input(|i| i.key_pressed(egui::Key::ArrowLeft)) && self.low > 0.0 {
                self.low -= 0.1;
                self.high -= 0.1;
                final_response.mark_changed();
            }

            // Mouse scroll for panning
            let scroll_delta = ui.ctx().input(|i| i.smooth_scroll_delta);
            self.high += scroll_delta.x as f64 * scroll_factor as f64;
            self.low += scroll_delta.x as f64 * scroll_factor as f64;

            // Mouse zoom for adjusting band width
            let zoom_delta = ui.ctx().input(|i| i.zoom_delta() - 1.0);
            self.high += zoom_delta as f64 * zoom_factor as f64 * 0.1;
            self.low -= zoom_delta as f64 * zoom_factor as f64 * 0.1;

            if scroll_delta.x != 0.0 || zoom_delta != 0.0 {
                final_response.mark_changed();
            }
        }

        // Combine responses for UI interactions
        final_response |= plot_response.response.clone();
        final_response |= slider_changed.response.clone();

        // Mark the response as changed if any interaction occurred
        if plot_response.response.changed() || slider_changed.inner {
            final_response.mark_changed();
        }

        final_response
    }
}
