use std::f32::consts::E;
use std::sync::mpsc::Sender;
use std::sync::{Arc, RwLock};

use eframe::egui;
use eframe::egui::panel::Side;
use eframe::egui::{vec2, DragValue, Stroke, Vec2, Visuals};
use egui_double_slider::DoubleSlider;
use egui_plot::{Line, LineStyle, Plot, PlotPoints, VLine};
use itertools_num::linspace;
use ndarray::Array1;

use crate::config::Config;
use crate::math_tools::apply_fft_window;
use crate::gui::matrix_plot::SelectedPixel;
use crate::toggle::toggle;
use crate::{DataPoint, GuiSettingsContainer};

#[allow(clippy::too_many_arguments)]
pub fn right_panel(
    ctx: &egui::Context,
    right_panel_width: &f32,
    gui_conf: &mut GuiSettingsContainer,
    filter_bounds: &mut [f32; 2],
    fft_bounds: &mut [f32; 2],
    time_window: &mut [f32; 2],
    pixel_selected: &mut SelectedPixel,
    pixel_lock: &Arc<RwLock<SelectedPixel>>,
    config_tx: &Sender<Config>,
    data_lock: &Arc<RwLock<DataPoint>>,
    scaling_lock: &Arc<RwLock<u8>>,
    wp: egui::Image,
) {
    let mut data = DataPoint::default();
    if let Ok(read_guard) = data_lock.read() {
        data = read_guard.clone();
    }

    egui::SidePanel::new(Side::Right, "Right Panel Settings")
        .min_width(*right_panel_width)
        .max_width(*right_panel_width)
        .resizable(false)
        .show(ctx, |ui| {
            ui.add_enabled_ui(true, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("Analysis");
                });
                ui.separator();

                egui::Grid::new("upper")
                    .num_columns(2)
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label("Log Mode: ");
                        if ui.add(toggle(&mut gui_conf.log_plot)).changed() {
                            config_tx
                                .send(Config::SetFFTLogPlot(gui_conf.log_plot))
                                .expect("unable to send config");
                        }
                        ui.end_row();

                        ui.label("Normalize FFT: ");
                        if ui.add(toggle(&mut gui_conf.normalize_fft)).changed() {
                            config_tx
                                .send(Config::SetFFTNormalization(gui_conf.normalize_fft))
                                .expect("unable to send config");
                        }

                        ui.end_row();
                        ui.label("Down scaling:");

                        ui.style_mut().spacing.slider_width = 320.0;

                        if ui
                            .add(egui::Slider::new(&mut gui_conf.down_scaling, 1..=10))
                            .changed()
                        {
                            pixel_selected.rect = vec![
                                [
                                    (pixel_selected.x as f64) / gui_conf.down_scaling as f64,
                                    (pixel_selected.y as f64) / gui_conf.down_scaling as f64,
                                ],
                                [
                                    (pixel_selected.x as f64) / gui_conf.down_scaling as f64 + 1.0,
                                    (pixel_selected.y as f64) / gui_conf.down_scaling as f64,
                                ],
                                [
                                    (pixel_selected.x as f64) / gui_conf.down_scaling as f64 + 1.0,
                                    (pixel_selected.y as f64) / gui_conf.down_scaling as f64 + 1.0,
                                ],
                                [
                                    (pixel_selected.x as f64) / gui_conf.down_scaling as f64,
                                    (pixel_selected.y as f64) / gui_conf.down_scaling as f64 + 1.0,
                                ],
                                [
                                    (pixel_selected.x as f64) / gui_conf.down_scaling as f64,
                                    (pixel_selected.y as f64) / gui_conf.down_scaling as f64,
                                ],
                            ];
                            if let Ok(mut s) = scaling_lock.write() {
                                *s = gui_conf.down_scaling as u8;
                            }
                            if let Ok(mut write_guard) = pixel_lock.write() {
                                *write_guard = pixel_selected.clone();
                            }
                            config_tx
                                .send(Config::SetDownScaling)
                                .expect("unable to send config");
                        }
                    });

                ui.separator();
                ui.heading("I. FFT window bounds: ");

                // TODO: implement different windows

                let mut window_vals: Vec<[f64; 2]> = Vec::new();
                let mut p = Array1::from_vec(vec![1.0; data.time.len()]);
                let t: Array1<f32> = linspace::<f32>(
                    data.hk.t_begin,
                    data.hk.t_begin + data.hk.range,
                    data.time.len(),
                )
                .collect();

                apply_fft_window(&mut p.view_mut(), &t, &fft_bounds[0], &fft_bounds[1]);

                for i in 0..t.len() {
                    window_vals.push([t[i] as f64, p[i] as f64]);
                }
                let fft_window_plot = Plot::new("FFT Window")
                    .include_x(data.hk.t_begin)
                    .include_x(data.hk.t_begin + data.hk.range)
                    .include_y(0.0)
                    .include_y(1.0)
                    .allow_drag(false)
                    .allow_zoom(false)
                    .allow_scroll(false)
                    .set_margin_fraction(Vec2 { x: 0.0, y: 0.05 })
                    .height(100.0)
                    .width(right_panel_width * 0.9);
                ui.vertical_centered(|ui| {
                    fft_window_plot.show(ui, |window_plot_ui| {
                        window_plot_ui.line(
                            Line::new(PlotPoints::from(window_vals))
                                .color(egui::Color32::RED)
                                .style(LineStyle::Solid)
                                .name("Blackman Window"),
                        );
                        window_plot_ui.vline(
                            VLine::new(data.hk.t_begin + fft_bounds[0])
                                .stroke(Stroke::new(1.0, egui::Color32::GRAY))
                                .name("Lower Bound"),
                        );
                        window_plot_ui.vline(
                            VLine::new(data.hk.t_begin + data.hk.range - fft_bounds[1])
                                .stroke(Stroke::new(1.0, egui::Color32::GRAY))
                                .name("Upper Bound"),
                        );
                    });
                });

                let slider_changed = ui.horizontal(|ui| {
                    let right_offset = 0.09 * right_panel_width;
                    let left_offset = 0.01 * right_panel_width;
                    ui.add_space(left_offset);
                    // Display slider, linked to the same range as the plot
                    let mut fft_lower_bound = fft_bounds[0];
                    let mut fft_upper_bound = data.hk.range - fft_bounds[1];

                    let slider_changed = ui
                        .add(
                            DoubleSlider::new(
                                &mut fft_lower_bound,
                                &mut fft_upper_bound,
                                0.0..=data.hk.range,
                            )
                            .zoom_factor(2.0)
                            .scroll_factor(0.005)
                            .separation_distance(2.0)
                            .invert_highlighting(true)
                            .width(right_panel_width - left_offset - right_offset),
                        )
                        .on_hover_text(egui::RichText::new(format!(
                            "{} Scroll and Zoom to adjust the sliders.",
                            egui_phosphor::regular::INFO
                        )))
                        .changed();
                    *fft_bounds = [fft_lower_bound, data.hk.range - fft_upper_bound];
                    slider_changed
                });

                ui.horizontal(|ui| {
                    let val1_changed = ui.add(DragValue::new(&mut fft_bounds[0])).changed();

                    ui.add_space(0.75 * right_panel_width);

                    let val2_changed = ui.add(DragValue::new(&mut fft_bounds[1])).changed();

                    if slider_changed.inner || val1_changed || val2_changed {
                        config_tx
                            .send(Config::SetFFTWindowLow(fft_bounds[0]))
                            .unwrap();
                        config_tx
                            .send(Config::SetFFTWindowHigh(fft_bounds[1]))
                            .unwrap();
                    }
                });

                ui.add_space(10.0);

                ui.separator();
                ui.heading("II. FFT Filter: ");

                // TODO: implement different windows

                let spectrum_vals: Vec<[f64; 2]> = data
                    .frequencies
                    .iter()
                    .zip(data.signal_1_fft.iter())
                    .map(|(x, y)| {
                        let mut fft;
                        if gui_conf.log_plot {
                            fft = (*y + 1.0).log(E);
                        } else {
                            fft = *y;
                        }
                        if fft < 0.0 {
                            fft = 0.0;
                        }
                        [*x as f64, fft as f64]
                    })
                    .collect();
                let max = spectrum_vals
                    .iter()
                    .fold(f64::NEG_INFINITY, |ai, &bi| ai.max(bi[1]));

                let mut filter_vals: Vec<[f64; 2]> = Vec::new();
                let filter_f: Vec<f64> = linspace::<f64>(0.0, 10.0, data.time.len()).collect();
                for fi in filter_f {
                    let a = if fi >= filter_bounds[0] as f64 && fi <= filter_bounds[1] as f64 {
                        max
                    } else {
                        0.0
                    };
                    filter_vals.push([fi, a]);
                }

                let window_plot = Plot::new("FFT Filter")
                    .include_x(0.0)
                    .include_x(10.0)
                    .include_y(0.0)
                    .allow_drag(false)
                    .allow_zoom(false)
                    .allow_scroll(false)
                    .set_margin_fraction(Vec2 { x: 0.0, y: 0.05 })
                    .height(100.0)
                    .width(right_panel_width * 0.9);
                ui.vertical_centered(|ui| {
                    window_plot.show(ui, |window_plot_ui| {
                        window_plot_ui.line(
                            Line::new(PlotPoints::from(spectrum_vals))
                                .color(egui::Color32::RED)
                                .style(LineStyle::Solid)
                                .name("Spectrum"),
                        );
                        window_plot_ui.line(
                            Line::new(PlotPoints::from(filter_vals))
                                .color(egui::Color32::BLUE)
                                .style(LineStyle::Solid)
                                .name("Filter"),
                        );
                        window_plot_ui.vline(
                            VLine::new(filter_bounds[0])
                                .stroke(Stroke::new(1.0, egui::Color32::GRAY))
                                .name("Filter Lower Bound"),
                        );
                        window_plot_ui.vline(
                            VLine::new(filter_bounds[1])
                                .stroke(Stroke::new(1.0, egui::Color32::GRAY))
                                .name("Filter Upper Bound"),
                        );
                    });
                });

                let slider_changed = ui.horizontal(|ui| {
                    let right_offset = 0.09 * right_panel_width;
                    let left_offset = 0.01 * right_panel_width;
                    ui.add_space(left_offset);
                    // Display slider, linked to the same range as the plot
                    let mut filter_lower_bound = filter_bounds[0];
                    let mut filter_upper_bound = filter_bounds[1];

                    let slider_changed = ui
                        .add(
                            DoubleSlider::new(
                                &mut filter_lower_bound,
                                &mut filter_upper_bound,
                                0.0..=10.0,
                            )
                            .zoom_factor(2.0)
                            .scroll_factor(0.005)
                            .separation_distance(0.05)
                            .width(right_panel_width - left_offset - right_offset),
                        )
                        .on_hover_text(egui::RichText::new(format!(
                            "{} Scroll and Zoom to adjust the sliders.",
                            egui_phosphor::regular::INFO
                        )))
                        .changed();
                    *filter_bounds = [filter_lower_bound, filter_upper_bound];
                    slider_changed
                });

                ui.horizontal(|ui| {
                    let val1_changed = ui.add(DragValue::new(&mut filter_bounds[0])).changed();

                    ui.add_space(0.75 * right_panel_width);

                    let val2_changed = ui.add(DragValue::new(&mut filter_bounds[1])).changed();

                    if slider_changed.inner || val1_changed || val2_changed {
                        config_tx
                            .send(Config::SetFFTFilterLow(filter_bounds[0]))
                            .unwrap();
                        config_tx
                            .send(Config::SetFFTFilterHigh(filter_bounds[1]))
                            .unwrap();
                    }
                });

                ui.add_space(10.0);

                ui.separator();
                ui.heading("III. Time Filter: ");

                let zoom_factor = 5.0;
                let scroll_factor = 0.01;

                let mut window_vals: Vec<[f64; 2]> = Vec::new();
                for i in 0..data.time.len() {
                    window_vals.push([data.time[i] as f64, data.signal_1[i] as f64]);
                }
                let time_window_plot = Plot::new("Time Window")
                    .allow_drag(false)
                    .set_margin_fraction(Vec2 { x: 0.0, y: 0.05 })
                    .height(100.0)
                    .allow_scroll(false)
                    .allow_zoom(false)
                    .width(right_panel_width * 0.9);
                let ui_response = ui.vertical_centered(|ui| {
                    time_window_plot.show(ui, |window_plot_ui| {
                        window_plot_ui.line(
                            Line::new(PlotPoints::from(window_vals))
                                .color(egui::Color32::RED)
                                .style(LineStyle::Solid)
                                .name("Pulse"),
                        );
                        window_plot_ui.vline(
                            // TODO: adjust this
                            VLine::new(time_window[0])
                                .stroke(Stroke::new(1.0, egui::Color32::GRAY))
                                .name("Lower Bound"),
                        );
                        window_plot_ui.vline(
                            VLine::new(time_window[1])
                                .stroke(Stroke::new(1.0, egui::Color32::GRAY))
                                .name("Upper Bound"),
                        );
                    })
                });

                ui_response
                    .response
                    .on_hover_text(egui::RichText::new(format!(
                        "{} Scroll and Zoom to adjust the sliders.",
                        egui_phosphor::regular::INFO
                    )));
                let plot_response = ui_response.inner;

                let slider_changed = ui.horizontal(|ui| {
                    let right_offset = 0.09 * right_panel_width;
                    let left_offset = 0.01 * right_panel_width;
                    ui.add_space(left_offset);
                    // Display slider, linked to the same range as the plot
                    let mut time_window_lower_bound = time_window[0];
                    let mut time_window_upper_bound = time_window[1];
                    let lower = data.time.first().unwrap_or(&1000.0);
                    let upper = data.time.last().unwrap_or(&1050.0);
                    let slider_changed = ui
                        .add(
                            DoubleSlider::new(
                                &mut time_window_lower_bound,
                                &mut time_window_upper_bound,
                                *lower..=*upper,
                            )
                            .zoom_factor(zoom_factor)
                            .separation_distance(2.0)
                            .width(right_panel_width - left_offset - right_offset),
                        )
                        .on_hover_text(egui::RichText::new(format!(
                            "{} Scroll and Zoom to adjust the sliders.",
                            egui_phosphor::regular::INFO
                        )))
                        .changed();
                    *time_window = [time_window_lower_bound, time_window_upper_bound];
                    slider_changed
                });

                ui.horizontal(|ui| {
                    let val1_changed = ui.add(DragValue::new(&mut time_window[0])).changed();

                    ui.add_space(0.75 * right_panel_width);

                    let val2_changed = ui.add(DragValue::new(&mut time_window[1])).changed();

                    if slider_changed.inner || val1_changed || val2_changed {
                        if time_window[0] == time_window[1] {
                            time_window[0] = *data.time.first().unwrap_or(&1000.0);
                            time_window[1] = *data.time.last().unwrap_or(&1050.0);
                        }
                        config_tx.send(Config::SetTimeWindow(*time_window)).unwrap();
                    }
                });

                let width = time_window[1] - time_window[0];
                let first = *data.time.first().unwrap_or(&1000.0);
                let last = *data.time.last().unwrap_or(&1050.0);

                if ui.input(|i| i.key_pressed(egui::Key::ArrowRight)) && time_window[1] < last {
                    time_window[0] += 1.0;
                    time_window[1] = width + time_window[0];
                    config_tx.send(Config::SetTimeWindow(*time_window)).unwrap();
                }

                if ui.input(|i| i.key_pressed(egui::Key::ArrowLeft)) && time_window[0] > first {
                    time_window[0] -= 1.0;
                    time_window[1] = width + time_window[0];
                    config_tx.send(Config::SetTimeWindow(*time_window)).unwrap();
                }

                // scroll through time axis
                if plot_response.response.hovered() {
                    let scroll_delta = ctx.input(|i| i.smooth_scroll_delta);
                    time_window[1] += scroll_delta.x * scroll_factor;
                    time_window[0] += scroll_delta.x * scroll_factor;

                    time_window[1] += scroll_delta.y * scroll_factor;
                    time_window[0] += scroll_delta.y * scroll_factor;
                    let zoom_delta = ctx.input(|i| i.zoom_delta() - 1.0);

                    time_window[1] += zoom_delta * zoom_factor;
                    time_window[0] -= zoom_delta * zoom_factor;

                    if scroll_delta != Vec2::ZERO || zoom_delta != 0.0 {
                        config_tx.send(Config::SetTimeWindow(*time_window)).unwrap();
                    }
                }

                gui_conf.dark_mode = ui.visuals() == &Visuals::dark();

                ui.separator();
                ui.collapsing("Debug logs:", |ui| {
                    ui.set_height(175.0);
                    egui_logger::logger_ui().show(ui);
                });

                // let mut task_open = false;
                // if task_open {
                //     ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Wait);
                // } else {
                //     ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Default);
                // }

                ui.add_space(5.0);
                ui.separator();

                let height = ui.available_size().y - 38.0 - 20.0;
                ui.add_space(height);
                ui.centered_and_justified(|ui| {
                    ui.add(wp.fit_to_exact_size(vec2(80.0, 38.0)));
                });
            });
        });
}
