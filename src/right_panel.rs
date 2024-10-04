use std::f32::consts::E;
use std::f64::NEG_INFINITY;
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::sync::{Arc, RwLock};

use eframe::egui;
use eframe::egui::panel::Side;
use eframe::egui::plot::{Line, LineStyle, Plot, PlotPoints, VLine};
use eframe::egui::{
    global_dark_light_mode_buttons, FontFamily, FontId, RichText, Slider, Stroke, Vec2, Visuals,
};
use egui_extras::RetainedImage;
use itertools_num::linspace;
use ndarray::Array1;

use crate::config::Config;
use crate::math_tools::apply_fft_window;
use crate::plot_slider::{fft_filter, time_filter, windowing};
use crate::toggle::toggle;
use crate::{DataPoint, GuiSettingsContainer, Print};

pub fn right_panel(
    ctx: &egui::Context,
    right_panel_width: &f32,
    gui_conf: &mut GuiSettingsContainer,
    console: &mut Vec<Print>,
    picked_path: &mut String,
    filter_bounds: &mut [f32; 2],
    fft_bounds: &mut [f32; 2],
    time_window: &mut [f32; 2],
    config_tx: &Sender<Config>,
    data_lock: &Arc<RwLock<DataPoint>>,
    print_lock: &Arc<RwLock<Vec<Print>>>,
    log_mode_lock: &Arc<RwLock<bool>>,
    normalize_fft_lock: &Arc<RwLock<bool>>,
    fft_bounds_lock: &Arc<RwLock<[f32; 2]>>,
    fft_filter_bounds_lock: &Arc<RwLock<[f32; 2]>>,
    hacktica_dark: &RetainedImage,
    hacktica_light: &RetainedImage,
    wp: &RetainedImage,
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
                ui.set_visible(true);
                ui.horizontal(|ui| {
                    ui.heading("Analysis");
                });
                ui.separator();

                egui::Grid::new("upper")
                    .num_columns(2)
                    //.spacing([40.0, 4.0])
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label("Log Mode: ");
                        ui.add(toggle(&mut gui_conf.log_plot));
                        ui.end_row();

                        ui.label("Normalize FFT: ");
                        ui.add(toggle(&mut gui_conf.normalize_fft));
                        if let Ok(mut write_guard) = normalize_fft_lock.write() {
                            *write_guard = gui_conf.normalize_fft.clone();
                        }

                        ui.end_row();
                        ui.label("Down scaling:");
                        if ui
                            .add(egui::Slider::new(&mut gui_conf.down_scaling, 1..=10))
                            .changed()
                        {
                            config_tx.send(Config::SetDownScaling(gui_conf.down_scaling));
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

                ui.vertical_centered(|ui| {
                    if ui
                        .add(windowing(
                            &(*right_panel_width * 0.9),
                            &100.0,
                            &data.hk.range,
                            fft_bounds,
                        ))
                        .changed()
                    {
                        if fft_bounds[0] < 0.0 {
                            fft_bounds[0] = 0.0;
                        }

                        if fft_bounds[1] < 0.0 {
                            fft_bounds[1] = 0.0;
                        }

                        if fft_bounds[0] > data.hk.range {
                            fft_bounds[0] = data.hk.range;
                        }

                        if fft_bounds[1] > data.hk.range {
                            fft_bounds[1] = data.hk.range;
                        }
                        config_tx
                            .send(Config::SetFFTWindowLow(fft_bounds[0]))
                            .unwrap();
                        config_tx
                            .send(Config::SetFFTWindowHigh(fft_bounds[1]))
                            .unwrap();
                    };
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
                    .fold(NEG_INFINITY, |ai, &bi| (ai as f64).max(bi[1]));

                let mut filter_vals: Vec<[f64; 2]> = Vec::new();
                let filter_f: Vec<f64> = linspace::<f64>(0.0, 10.0, data.time.len()).collect();
                for i in 0..filter_f.len() {
                    let a: f64;
                    if filter_f[i] >= filter_bounds[0] as f64
                        && filter_f[i] <= filter_bounds[1] as f64
                    {
                        a = max;
                    } else {
                        a = 0.0;
                    }
                    filter_vals.push([filter_f[i], a]);
                }

                let window_plot = Plot::new("FFT Filter")
                    .include_x(0.0)
                    .include_x(10.0)
                    .include_y(0.0)
                    .allow_drag(false)
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

                ui.vertical_centered(|ui| {
                    if ui
                        .add(fft_filter(
                            &(*right_panel_width * 0.9),
                            &100.0,
                            &10.0,
                            filter_bounds,
                        ))
                        .changed()
                    {
                        config_tx
                            .send(Config::SetFFTFilterLow(filter_bounds[0]))
                            .unwrap();
                        config_tx
                            .send(Config::SetFFTFilterHigh(filter_bounds[1]))
                            .unwrap();
                    };
                });

                ui.add_space(10.0);

                ui.separator();
                ui.heading("III. Time Filter: ");

                let mut window_vals: Vec<[f64; 2]> = Vec::new();
                for i in 0..data.time.len() {
                    window_vals.push([data.time[i] as f64, data.signal_1[i] as f64]);
                }
                let time_window_plot = Plot::new("Time Window")
                    .allow_drag(false)
                    .set_margin_fraction(Vec2 { x: 0.0, y: 0.05 })
                    .height(100.0)
                    .width(right_panel_width * 0.9);
                ui.vertical_centered(|ui| {
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
                    });
                });

                ui.vertical_centered(|ui| {
                    if ui
                        .add(time_filter(
                            &(*right_panel_width * 0.9),
                            &100.0,
                            &data.time.first().unwrap_or(&1000.0),
                            &data.time.last().unwrap_or(&1050.0),
                            time_window,
                        ))
                        .changed()
                    {
                        if time_window[0] == time_window[1] {
                            time_window[0] = *data.time.first().unwrap_or(&1000.0);
                            time_window[1] = *data.time.last().unwrap_or(&1050.0);
                        }
                        config_tx
                            .send(Config::SetTimeWindow(time_window.clone()))
                            .unwrap();
                    };
                });

                let mut width = time_window[1] - time_window[0];
                let first = *data.time.first().unwrap_or(&1000.0);
                let last = *data.time.last().unwrap_or(&1050.0);
                if ui
                    .add(Slider::new(&mut width, 0.5..=last - first))
                    .changed()
                {
                    if time_window[0] == time_window[1] {
                        time_window[0] = *data.time.first().unwrap_or(&1000.0);
                        time_window[1] = *data.time.last().unwrap_or(&1050.0);
                    }
                    config_tx
                        .send(Config::SetTimeWindow(time_window.clone()))
                        .unwrap();
                }
                time_window[1] = width + time_window[0];
                if ui
                    .add(Slider::new(&mut time_window[0], first..=last - width))
                    .changed()
                {
                    if time_window[0] == time_window[1] {
                        time_window[0] = *data.time.first().unwrap_or(&1000.0);
                        time_window[1] = *data.time.last().unwrap_or(&1050.0);
                    }
                    time_window[1] = width + time_window[0];
                    config_tx
                        .send(Config::SetTimeWindow(time_window.clone()))
                        .unwrap();
                }

                if ui.input(|i| i.key_pressed(egui::Key::ArrowRight)) && time_window[1] < last {
                    time_window[0] += 1.0;
                    time_window[1] = width + time_window[0];
                    config_tx
                        .send(Config::SetTimeWindow(time_window.clone()))
                        .unwrap();
                }

                if ui.input(|i| i.key_pressed(egui::Key::ArrowLeft)) && time_window[0] > first {
                    time_window[0] -= 1.0;
                    time_window[1] = width + time_window[0];
                    config_tx
                        .send(Config::SetTimeWindow(time_window.clone()))
                        .unwrap();
                }

                ui.add_space(40.0);
                ui.separator();

                global_dark_light_mode_buttons(ui);

                gui_conf.dark_mode = ui.visuals() == &Visuals::dark();

                let text_style = egui::TextStyle::Body;
                let row_height = ui.text_style_height(&text_style);
                let num_rows = console.len();
                ui.separator();
                let mut task_open = false;
                egui::ScrollArea::vertical()
                    .id_source("console_scroll_area")
                    .auto_shrink([false; 2])
                    .stick_to_bottom(true)
                    .max_height(row_height * 5.20)
                    .show_rows(ui, row_height, num_rows, |ui, row_range| {
                        for row in row_range {
                            match console[row].clone() {
                                Print::EMPTY => {}
                                Print::MESSAGE(s) => {
                                    let text = "[MSG] ".to_string();
                                    ui.horizontal_wrapped(|ui| {
                                        let color: egui::Color32;
                                        if gui_conf.dark_mode {
                                            color = egui::Color32::WHITE;
                                        } else {
                                            color = egui::Color32::BLACK;
                                        }
                                        ui.label(
                                            RichText::new(text)
                                                .color(color)
                                                .font(FontId::new(14.0, FontFamily::Monospace)),
                                        );
                                        let text = format!("{}", s);
                                        ui.label(
                                            RichText::new(text)
                                                .font(FontId::new(14.0, FontFamily::Monospace)),
                                        );
                                    });
                                }
                                Print::ERROR(s) => {
                                    ui.horizontal_wrapped(|ui| {
                                        let text = "[ERR] ".to_string();
                                        ui.label(
                                            RichText::new(text)
                                                .color(egui::Color32::RED)
                                                .font(FontId::new(14.0, FontFamily::Monospace)),
                                        );
                                        let text = format!("{}", s);
                                        ui.label(
                                            RichText::new(text)
                                                .font(FontId::new(14.0, FontFamily::Monospace)),
                                        );
                                    });
                                }
                                Print::DEBUG(s) => {
                                    if gui_conf.debug {
                                        let color: egui::Color32;
                                        if gui_conf.dark_mode {
                                            color = egui::Color32::YELLOW;
                                        } else {
                                            color = egui::Color32::LIGHT_RED;
                                        }
                                        ui.horizontal_wrapped(|ui| {
                                            let text = "[DBG] ".to_string();
                                            ui.label(
                                                RichText::new(text)
                                                    .color(color)
                                                    .font(FontId::new(14.0, FontFamily::Monospace)),
                                            );
                                            let text = format!("{}", s);
                                            ui.label(
                                                RichText::new(text)
                                                    .font(FontId::new(14.0, FontFamily::Monospace)),
                                            );
                                        });
                                    }
                                }
                                Print::TASK(s) => {
                                    task_open = true;
                                    ui.horizontal_wrapped(|ui| {
                                        let text = "[  ] ".to_string();
                                        ui.label(
                                            RichText::new(text)
                                                .color(egui::Color32::WHITE)
                                                .font(FontId::new(14.0, FontFamily::Monospace)),
                                        );
                                        let text = format!("{}", s);
                                        ui.label(
                                            RichText::new(text)
                                                .font(FontId::new(14.0, FontFamily::Monospace)),
                                        );
                                    });
                                }
                                Print::OK(s) => {
                                    task_open = false;
                                    ui.horizontal_wrapped(|ui| {
                                        let text = "[OK] ".to_string();
                                        ui.label(
                                            RichText::new(text)
                                                .color(egui::Color32::GREEN)
                                                .font(FontId::new(14.0, FontFamily::Monospace)),
                                        );
                                        let text = format!("{}", s);
                                        ui.label(
                                            RichText::new(text)
                                                .font(FontId::new(14.0, FontFamily::Monospace)),
                                        );
                                    });
                                }
                            }
                        }
                    });
                if task_open {
                    ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Wait);
                } else {
                    ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Default);
                }
                ui.add_space(5.0);
                ui.separator();

                let height = ui.available_size().y - 38.0 - 20.0;
                ui.add_space(height);
                ui.horizontal_wrapped(|ui| {
                    let width = (ui.available_size().x - 96.0 - 96.0) / 3.0;
                    ui.add_space(width);
                    if gui_conf.dark_mode == true {
                        ui.add(egui::Image::new(
                            hacktica_dark.texture_id(ctx),
                            [96.0, 38.0],
                        ));
                    } else {
                        ui.add(egui::Image::new(
                            hacktica_light.texture_id(ctx),
                            [96.0, 38.0],
                        ));
                    }
                    ui.add_space(50.0);
                    ui.add(egui::Image::new(wp.texture_id(ctx), [80.0, 38.0]));
                });
            });
        });
}
