use crate::config::ThreadCommunication;
use crate::data_container::ScannedImageFilterData;
use crate::filters::filter::{Filter, FilterConfig, FilterDomain};
use crate::gui::application::GuiSettingsContainer;
use bevy_egui::egui::{self, Ui};
use bevy_egui::egui::{DragValue, Stroke, Vec2};
use egui_double_slider::DoubleSlider;
use egui_plot::{Line, LineStyle, Plot, PlotPoints, VLine};
use filter_macros::register_filter;
use ndarray::s;
use num_complex::Complex;
use num_traits::Float;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::{Arc, RwLock};

#[derive(Debug)]
#[register_filter]
#[derive(Clone)]
pub struct FrequencyDomainBandPass {
    pub low: f64,  // Low cutoff frequency
    pub high: f64, // High cutoff frequency
    pub window_width: f64,
    pub freq_axis: Vec<f32>,
    signal_axis: Vec<f32>,
}

impl Filter for FrequencyDomainBandPass {
    fn new() -> Self
    where
        Self: Sized,
    {
        FrequencyDomainBandPass {
            low: 0.5,
            high: 5.0,
            window_width: 2.0,
            freq_axis: vec![],
            signal_axis: vec![],
        }
    }

    fn config(&self) -> FilterConfig {
        FilterConfig {
            name: "Frequency Band Pass".to_string(),
            domain: FilterDomain::Frequency,
        }
    }

    fn filter(
        &mut self,
        input_data: &ScannedImageFilterData,
        _gui_settings: &mut GuiSettingsContainer,
        progress_lock: &mut Arc<RwLock<Option<f32>>>,
        abort_flag: &Arc<AtomicBool>,
    ) -> ScannedImageFilterData {
        if let Ok(mut p) = progress_lock.write() {
            *p = Some(0.0);
        }

        let mut output_data = input_data.clone();
        let shape = output_data.data.dim();

        // Store the frequency axis for UI visualization
        self.freq_axis = output_data.frequency.to_vec();

        // Get pixel data for visualization
        let pixel = input_data.pixel_selected;

        // Transform all signals to frequency domain
        for i in 0..shape.0 {
            if abort_flag.load(Relaxed) {
                break;
            }

            for j in 0..shape.1 {
                // Get the time domain signal for this pixel
                let mut signal = output_data.data.slice(s![i, j, ..]).to_vec();

                // Transform to frequency domain
                let mut spectrum = vec![Complex::new(0.0, 0.0); signal.len() / 2 + 1];
                if let Some(r2c) = &output_data.r2c {
                    r2c.process(&mut signal, &mut spectrum).unwrap_or_default();

                    // Apply frequency domain bandpass filter
                    let safe_low = self.low.max(0.0) as f32;
                    let safe_high = self.high.min(output_data.frequency.len() as f64) as f32;

                    // Zero out frequencies outside the passband
                    for (k, freq) in output_data.frequency.iter().enumerate() {
                        if *freq < safe_low || *freq > safe_high {
                            spectrum[k] = Complex::new(0.0, 0.0);
                        }
                    }

                    // Transform back to time domain
                    if let Some(c2r) = &output_data.c2r {
                        c2r.process(&mut spectrum, &mut signal).unwrap_or_default();

                        // Normalize (IFFT scaling)
                        let scale = 1.0 / signal.len() as f32;
                        signal.iter_mut().for_each(|v| *v *= scale);

                        // Update the data
                        for (k, v) in signal.iter().enumerate() {
                            output_data.data[[i, j, k]] = *v;
                        }
                    }
                }
            }

            // Update progress
            if let Ok(mut p) = progress_lock.write() {
                *p = Some((i as f32) / (shape.0 as f32));
            }
        }

        // Get the spectrum of the selected pixel for visualization
        if pixel[0] < shape.0 && pixel[1] < shape.1 {
            let mut signal = output_data.data.slice(s![pixel[0], pixel[1], ..]).to_vec();
            let mut spectrum = vec![Complex::new(0.0, 0.0); signal.len() / 2 + 1];

            if let Some(r2c) = &output_data.r2c {
                r2c.process(&mut signal, &mut spectrum).unwrap_or_default();
                self.signal_axis = spectrum.iter().map(|c| c.norm()).collect();
            } else {
                self.signal_axis = vec![0.0; output_data.frequency.len()];
            }
        } else {
            self.signal_axis = vec![0.0; output_data.frequency.len()];
        }

        if let Ok(mut p) = progress_lock.write() {
            *p = None;
        }

        output_data
    }

    fn ui(
        &mut self,
        ui: &mut Ui,
        thread_communication: &mut ThreadCommunication,
        panel_width: f32,
    ) -> egui::Response {
        let mut final_response = ui.allocate_response(Vec2::ZERO, egui::Sense::hover());

        let zoom_factor = 5.0;
        let scroll_factor = 0.01;

        // Create frequency spectrum visualization
        let mut spectrum_vals: Vec<[f64; 2]> = Vec::new();
        for (i, freq) in self.freq_axis.iter().enumerate() {
            if i < self.signal_axis.len() {
                let amplitude = self.signal_axis[i];
                spectrum_vals.push([*freq as f64, amplitude as f64]);
            }
        }

        // Calculate max for scaling
        let max = spectrum_vals.iter().fold(0.0, |acc, &[_, y]| acc.max(y));

        // Create filter visualization
        let mut filter_vals: Vec<[f64; 2]> = Vec::new();
        for (i, freq) in self.freq_axis.iter().enumerate() {
            let amplitude = if *freq as f64 >= self.low && *freq as f64 <= self.high {
                max
            } else {
                0.0
            };
            filter_vals.push([*freq as f64, amplitude]);
        }

        // Frequency domain plot
        let freq_plot = Plot::new("Frequency Domain")
            .allow_drag(false)
            .allow_zoom(false)
            .allow_scroll(false)
            .set_margin_fraction(Vec2 { x: 0.0, y: 0.05 })
            .height(100.0)
            .width(panel_width * 0.9);

        let ui_response = ui.vertical_centered(|ui| {
            freq_plot.show(ui, |plot_ui| {
                // Plot the spectrum
                plot_ui.line(
                    Line::new(PlotPoints::from(spectrum_vals))
                        .color(egui::Color32::RED)
                        .style(LineStyle::Solid)
                        .name("Spectrum"),
                );

                // Plot the filter shape
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
            })
        });

        ui_response.response.on_hover_text(
            "Frequency domain bandpass filter. Adjust the sliders to set cutoff frequencies.",
        );
        let plot_response = ui_response.inner;

        // Frequency sliders
        let slider_changed = ui.horizontal(|ui| {
            let right_offset = 0.09 * panel_width;
            let left_offset = 0.01 * panel_width;
            ui.add_space(left_offset);

            let mut freq_lower_bound = self.low;
            let mut freq_upper_bound = self.high;
            let max_freq = *self.freq_axis.last().unwrap_or(&10.0) as f64;

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
            if slider.double_clicked() {
                freq_lower_bound = 0.5;
                freq_upper_bound = max_freq * 0.5;
            }

            self.low = freq_lower_bound;
            self.high = freq_upper_bound;
            slider_changed
        });

        // Numeric input for precise values
        ui.horizontal(|ui| {
            ui.label("Low cutoff:");
            let val1_changed = ui
                .add(
                    DragValue::new(&mut self.low)
                        .speed(0.01)
                        .clamp_range(0.0..=self.high),
                )
                .changed();

            ui.add_space(0.5 * panel_width);

            ui.label("High cutoff:");
            let val2_changed = ui
                .add(
                    DragValue::new(&mut self.high)
                        .speed(0.01)
                        .clamp_range(self.low..=*self.freq_axis.last().unwrap_or(&10.0) as f64),
                )
                .changed();

            if slider_changed.inner || val1_changed || val2_changed {
                if self.low == self.high {
                    self.low = self.low.max(0.1);
                    self.high = self.high + 0.1;
                }
                final_response.mark_changed();
            }
        });

        // Keyboard controls
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

        // Mouse wheel controls
        if plot_response.response.hovered() {
            let scroll_delta = ui.ctx().input(|i| i.smooth_scroll_delta);
            self.high += scroll_delta.x as f64 * scroll_factor as f64;
            self.low += scroll_delta.x as f64 * scroll_factor as f64;

            let zoom_delta = ui.ctx().input(|i| i.zoom_delta() - 1.0);
            self.high += zoom_delta as f64 * zoom_factor as f64 * 0.1;
            self.low -= zoom_delta as f64 * zoom_factor as f64 * 0.1;

            if scroll_delta.x != 0.0 || zoom_delta != 0.0 {
                final_response.mark_changed();
            }
        }

        // Combine responses
        final_response |= plot_response.response.clone();
        final_response |= slider_changed.response.clone();

        // Mark changed if interaction happened
        if plot_response.response.changed() || slider_changed.inner {
            final_response.mark_changed();
        }

        final_response
    }
}
