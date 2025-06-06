use crate::config::ThreadCommunication;
use crate::data_container::ScannedImageFilterData;
use crate::filters::filter::{Filter, FilterConfig, FilterDomain};
use crate::gui::application::GuiSettingsContainer;
use crate::math_tools::apply_adapted_blackman_window;
use bevy_egui::egui::{self, Ui};
use filter_macros::register_filter;
use ndarray::s;
use realfft::RealFftPlanner;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, RwLock};

#[derive(Debug)]
#[register_filter]
#[derive(Clone)]
pub struct BandPass {
    pub low: f64,
    pub high: f64,
    pub window_width: f64,
    pub time_axis: Vec<f32>,
}

impl Filter for BandPass {
    fn new() -> Self
    where
        Self: Sized,
    {
        BandPass {
            low: 0.0,
            high: 0.0,
            window_width: 2.0,
            time_axis: vec![],
        }
    }

    fn config(&self) -> FilterConfig {
        FilterConfig {
            name: "Band Pass".to_string(),
            domain: FilterDomain::TimeBeforeFFT,
        }
    }

    fn filter(
        &mut self,
        filter_data: &mut ScannedImageFilterData,
        gui_settings: &mut GuiSettingsContainer,
        progress_lock: &mut Arc<RwLock<Option<f32>>>,
        abort_flag: &Arc<AtomicBool>,
    ) {
        let lower = filter_data
            .time
            .iter()
            .position(|t| *t == self.low.round() as f32)
            .unwrap_or(0);
        let upper = filter_data
            .time
            .iter()
            .position(|t| *t == self.high.round() as f32)
            .unwrap_or(filter_data.time.len());

        // apply the bandpass filter to the signal
        for i in 0..filter_data.data.len() {
            // let mut signal = filter_data.data.slice_mut(s![i, lower..upper]);
            // apply_adapted_blackman_window(
            //     &mut signal,
            //     &filter_data.time,
            //     &(self.window_width as f32),
            //     &(self.window_width as f32),
            // );
        }

        // Update the time window in the filter data
        filter_data.time = filter_data.time.slice(s![lower..upper]).to_owned();

        self.time_axis = filter_data.time.to_vec();
        self.time_axis = filter_data.time.to_vec();

        let n = filter_data.time.len();
        let rng = filter_data.time.last().unwrap() - filter_data.time.first().unwrap();

        let mut real_planner = RealFftPlanner::<f32>::new();
        let r2c = real_planner.plan_fft_forward(n);
        let c2r = real_planner.plan_fft_inverse(n);
        let spectrum = r2c.make_output_vec();
        let freq = (0..spectrum.len()).map(|i| i as f32 / rng).collect();
        filter_data.frequency = freq;
        filter_data.r2c = Some(r2c);
        filter_data.c2r = Some(c2r);
    }

    fn ui(
        &mut self,
        ui: &mut Ui,
        _thread_communication: &mut ThreadCommunication,
        panel_width: f32,
    ) -> egui::Response {
        // let mut final_response = ui.allocate_response(egui::Vec2::ZERO, egui::Sense::hover());
        //
        // let zoom_factor = 5.0;
        // let scroll_factor = 0.01;
        //
        // let mut window_vals: Vec<[f64; 2]> = Vec::new();
        // for i in 0..self.time_axis.len() {
        //     window_vals.push([self.time_axis[i] as f64, self.signal_axis[i] as f64]);
        // }
        // let time_window_plot = Plot::new("Time Window")
        //     .allow_drag(false)
        //     .set_margin_fraction(Vec2 { x: 0.0, y: 0.05 })
        //     .height(100.0)
        //     .allow_scroll(false)
        //     .allow_zoom(false)
        //     .width(panel_width * 0.9);
        // let ui_response = ui.vertical_centered(|ui| {
        //     time_window_plot.show(ui, |window_plot_ui| {
        //         window_plot_ui.line(
        //             Line::new(PlotPoints::from(window_vals))
        //                 .color(egui::Color32::RED)
        //                 .style(LineStyle::Solid)
        //                 .name("Pulse"),
        //         );
        //         window_plot_ui.vline(
        //             // TODO: adjust this
        //             VLine::new(explorer.time_window[0])
        //                 .stroke(Stroke::new(1.0, egui::Color32::GRAY))
        //                 .name("Lower Bound"),
        //         );
        //         window_plot_ui.vline(
        //             VLine::new(explorer.time_window[1])
        //                 .stroke(Stroke::new(1.0, egui::Color32::GRAY))
        //                 .name("Upper Bound"),
        //         );
        //     })
        // });
        //
        // ui_response
        //     .response
        //     .on_hover_text(egui::RichText::new(format!(
        //         "{} Scroll and Zoom to adjust the sliders.",
        //         egui_phosphor::regular::INFO
        //     )));
        // let plot_response = ui_response.inner;
        //
        // let slider_changed = ui.horizontal(|ui| {
        //     let right_offset = 0.09 * panel_width;
        //     let left_offset = 0.01 * panel_width;
        //     ui.add_space(left_offset);
        //     // Display slider, linked to the same range as the plot
        //     let mut time_window_lower_bound = explorer.time_window[0];
        //     let mut time_window_upper_bound = explorer.time_window[1];
        //     let lower = data.time.first().unwrap_or(&1000.0);
        //     let upper = data.time.last().unwrap_or(&1050.0);
        //     let slider = ui
        //         .add(
        //             DoubleSlider::new(
        //                 &mut time_window_lower_bound,
        //                 &mut time_window_upper_bound,
        //                 *lower..=*upper,
        //             )
        //                 .zoom_factor(zoom_factor)
        //                 .separation_distance(1.0)
        //                 .width(panel_width - left_offset - right_offset),
        //         )
        //         .on_hover_text(egui::RichText::new(format!(
        //             "{} Scroll and Zoom to adjust the sliders. Double Click to reset.",
        //             egui_phosphor::regular::INFO
        //         )));
        //     let slider_changed = slider.changed();
        //     if slider.double_clicked() {
        //         time_window_lower_bound = *lower;
        //         time_window_upper_bound = *upper;
        //     }
        //     explorer.time_window = [time_window_lower_bound, time_window_upper_bound];
        //     slider_changed
        // });
        //
        // ui.horizontal(|ui| {
        //     let val1_changed = ui
        //         .add(DragValue::new(&mut explorer.time_window[0]))
        //         .changed();
        //
        //     ui.add_space(0.75 * panel_width);
        //
        //     let val2_changed = ui
        //         .add(DragValue::new(&mut explorer.time_window[1]))
        //         .changed();
        //
        //     if slider_changed.inner || val1_changed || val2_changed {
        //         if explorer.time_window[0] == explorer.time_window[1] {
        //             explorer.time_window[0] = *data.time.first().unwrap_or(&1000.0);
        //             explorer.time_window[1] = *data.time.last().unwrap_or(&1050.0);
        //         }
        //         thread_communication
        //             .config_tx
        //             .send(ConfigCommand::SetTimeWindow(explorer.time_window))
        //             .unwrap();
        //     }
        // });
        //
        // let width = explorer.time_window[1] - explorer.time_window[0];
        // let first = *data.time.first().unwrap_or(&1000.0);
        // let last = *data.time.last().unwrap_or(&1050.0);
        //
        // if ui.input(|i| i.key_pressed(egui::Key::ArrowRight))
        //     && explorer.time_window[1] < last
        // {
        //     explorer.time_window[0] += 1.0;
        //     explorer.time_window[1] = width + explorer.time_window[0];
        //     thread_communication
        //         .config_tx
        //         .send(ConfigCommand::SetTimeWindow(explorer.time_window))
        //         .unwrap();
        // }
        //
        // if ui.input(|i| i.key_pressed(egui::Key::ArrowLeft))
        //     && explorer.time_window[0] > first
        // {
        //     explorer.time_window[0] -= 1.0;
        //     explorer.time_window[1] = width + explorer.time_window[0];
        //     thread_communication
        //         .config_tx
        //         .send(ConfigCommand::SetTimeWindow(explorer.time_window))
        //         .unwrap();
        // }
        //
        // // scroll through time axis
        // if plot_response.response.hovered() {
        //     let scroll_delta = ctx.input(|i| i.smooth_scroll_delta);
        //     explorer.time_window[1] += scroll_delta.x * scroll_factor;
        //     explorer.time_window[0] += scroll_delta.x * scroll_factor;
        //
        //     explorer.time_window[1] += scroll_delta.y * scroll_factor;
        //     explorer.time_window[0] += scroll_delta.y * scroll_factor;
        //     let zoom_delta = ctx.input(|i| i.zoom_delta() - 1.0);
        //
        //     explorer.time_window[1] += zoom_delta * zoom_factor;
        //     explorer.time_window[0] -= zoom_delta * zoom_factor;
        //
        //     if scroll_delta != Vec2::ZERO || zoom_delta != 0.0 {
        //         thread_communication
        //             .config_tx
        //             .send(ConfigCommand::SetTimeWindow(explorer.time_window))
        //             .unwrap();
        //     }
        // }
        //
        // // Merge responses to track interactivity
        // final_response |= response_x.clone();
        // final_response |= response_y.clone();
        //
        // // Only mark changed if any slider was changed (not just hovered)
        // if response_x.changed() || response_y.changed() {
        //     final_response.mark_changed();
        // }
        //
        // final_response
        ui.label("WIP")
    }
}
