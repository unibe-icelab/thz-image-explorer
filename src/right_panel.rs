use std::f64::consts::E;
use std::f64::NEG_INFINITY;
use std::sync::{Arc, RwLock};
use std::sync::mpsc::Sender;

use eframe::egui;
use eframe::egui::{FontFamily, FontId, global_dark_light_mode_buttons, RichText, Stroke, Vec2, Visuals};
use eframe::egui::panel::Side;
use eframe::egui::plot::{Line, LineStyle, Plot, PlotPoints, VLine};
use egui_extras::RetainedImage;
use itertools_num::linspace;

use crate::{DataContainer, GuiSettingsContainer, Print};
use crate::data::NUM_PULSE_LINES;
use crate::math_tools::apply_fft_window;
use crate::plot_slider::{filter, windowing};
use crate::toggle::toggle;

pub fn right_panel(ctx: &egui::Context,
                   right_panel_width: &f32,
                   gui_conf: &mut GuiSettingsContainer,
                   console: &mut Vec<Print>,
                   picked_path: &mut String,
                   filter_bounds: &mut [f64; 2],
                   fft_bounds: &mut [f64; 2],
                   save_tx: &Sender<String>,
                   data_lock: &Arc<RwLock<DataContainer>>,
                   print_lock: &Arc<RwLock<Vec<Print>>>,
                   log_mode_lock: &Arc<RwLock<bool>>,
                   normalize_fft_lock: &Arc<RwLock<bool>>,
                   fft_bounds_lock: &Arc<RwLock<[f64; 2]>>,
                   fft_filter_bounds_lock: &Arc<RwLock<[f64; 2]>>,
                   hacktica_dark: &RetainedImage,
                   hacktica_light: &RetainedImage,
                   wp: &RetainedImage,
) {
    let mut data = DataContainer::default();
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
                    ui.heading("Map");
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
                    });

                ui.label("FFT window bounds: ");

                // TODO: implement different windows

                let mut window_vals: Vec<[f64; 2]> = Vec::new();
                let mut p = vec![1.0; NUM_PULSE_LINES];
                let t: Vec<f64> = linspace::<f64>(data.hk.t_begin,
                                                  data.hk.t_begin + data.hk.range, NUM_PULSE_LINES).collect();
                apply_fft_window(&mut p, &t, &fft_bounds[0], &fft_bounds[1]);

                for i in 0..t.len() {
                    window_vals.push([t[i], p[i]]);
                }
                let window_plot = Plot::new("Window")
                    .include_x(data.hk.t_begin)
                    .include_x(data.hk.t_begin + data.hk.range)
                    .include_y(0.0)
                    .include_y(1.0)
                    .allow_drag(false)
                    .set_margin_fraction(Vec2 { x: 0.0, y: 0.05 })
                    .height(100.0)
                    .width(right_panel_width * 0.9);
                ui.vertical_centered(|ui| {
                    window_plot.show(ui, |window_plot_ui| {
                        window_plot_ui.line(Line::new(PlotPoints::from(window_vals))
                            .color(egui::Color32::RED)
                            .style(LineStyle::Solid)
                            .name("Blackman Window"));
                        window_plot_ui.vline(VLine::new(data.hk.t_begin + fft_bounds[0])
                            .stroke(Stroke::new(1.0, egui::Color32::GRAY))
                            .name("Lower Bound"));
                        window_plot_ui.vline(VLine::new(data.hk.t_begin + data.hk.range - fft_bounds[1])
                            .stroke(Stroke::new(1.0, egui::Color32::GRAY))
                            .name("Upper Bound"));
                    });
                });

                ui.vertical_centered(|ui| {
                    ui.add(windowing(&(*right_panel_width as f64 * 0.9), &100.0, &data.hk.range, fft_bounds));
                });
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


                if let Ok(mut write_guard) = fft_bounds_lock.write() {
                    *write_guard = fft_bounds.clone();
                }

                ui.add_space(10.0);

                ui.label("FFT Filter: ");

                // TODO: implement different windows

                let spectrum_vals: Vec<[f64; 2]> = data.frequencies_fft.iter()
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
                        [*x as f64, fft]
                    }).collect();
                let max = spectrum_vals.iter().fold(NEG_INFINITY, |ai, &bi| ai.max(bi[1]));

                let mut filter_vals: Vec<[f64; 2]> = Vec::new();
                let filter_f: Vec<f64> = linspace::<f64>(0.0, 10.0, NUM_PULSE_LINES).collect();
                for i in 0..filter_f.len() {
                    let a: f64;
                    if filter_f[i] >= filter_bounds[0] && filter_f[i] <= filter_bounds[1] {
                        a = max;
                    } else {
                        a = 0.0;
                    }
                    filter_vals.push([filter_f[i], a]);
                }

                let window_plot = Plot::new("Filter")
                    .include_x(0.0)
                    .include_x(10.0)
                    .include_y(0.0)
                    .allow_drag(false)
                    .set_margin_fraction(Vec2 { x: 0.0, y: 0.05 })
                    .height(100.0)
                    .width(right_panel_width * 0.9);
                ui.vertical_centered(|ui| {
                    window_plot.show(ui, |window_plot_ui| {
                        window_plot_ui.line(Line::new(PlotPoints::from(spectrum_vals))
                            .color(egui::Color32::RED)
                            .style(LineStyle::Solid)
                            .name("Spectrum"));
                        window_plot_ui.line(Line::new(PlotPoints::from(filter_vals))
                            .color(egui::Color32::BLUE)
                            .style(LineStyle::Solid)
                            .name("Filter"));
                        window_plot_ui.vline(VLine::new(filter_bounds[0])
                            .stroke(Stroke::new(1.0, egui::Color32::GRAY))
                            .name("Filter Lower Bound"));
                        window_plot_ui.vline(VLine::new(filter_bounds[1])
                            .stroke(Stroke::new(1.0, egui::Color32::GRAY))
                            .name("Filter Upper Bound"));
                    });
                });

                ui.vertical_centered(|ui| {
                    if ui.add(filter(&(*right_panel_width as f64 * 0.9), &100.0, &10.0, filter_bounds)).changed() {
                        if let Ok(mut write_guard) = fft_filter_bounds_lock.write() {
                            *write_guard = filter_bounds.clone();
                        }
                    };
                });

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
                    .show_rows(ui, row_height, num_rows,
                               |ui, row_range| {
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
                                                   ui.label(RichText::new(text).color(color).font(
                                                       FontId::new(14.0, FontFamily::Monospace)));
                                                   let text = format!("{}", s);
                                                   ui.label(RichText::new(text).font(
                                                       FontId::new(14.0, FontFamily::Monospace)));
                                               });
                                           }
                                           Print::ERROR(s) => {
                                               ui.horizontal_wrapped(|ui| {
                                                   let text = "[ERR] ".to_string();
                                                   ui.label(RichText::new(text).color(egui::Color32::RED).font(
                                                       FontId::new(14.0, FontFamily::Monospace)));
                                                   let text = format!("{}", s);
                                                   ui.label(RichText::new(text).font(
                                                       FontId::new(14.0, FontFamily::Monospace)));
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
                                                       ui.label(RichText::new(text).color(color).font(
                                                           FontId::new(14.0, FontFamily::Monospace)));
                                                       let text = format!("{}", s);
                                                       ui.label(RichText::new(text).font(
                                                           FontId::new(14.0, FontFamily::Monospace)));
                                                   });
                                               }
                                           }
                                           Print::TASK(s) => {
                                               task_open = true;
                                               ui.horizontal_wrapped(|ui| {
                                                   let text = "[  ] ".to_string();
                                                   ui.label(RichText::new(text).color(egui::Color32::WHITE).font(
                                                       FontId::new(14.0, FontFamily::Monospace)));
                                                   let text = format!("{}", s);
                                                   ui.label(RichText::new(text).font(
                                                       FontId::new(14.0, FontFamily::Monospace)));
                                               });
                                           }
                                           Print::OK(s) => {
                                               task_open = false;
                                               ui.horizontal_wrapped(|ui| {
                                                   let text = "[OK] ".to_string();
                                                   ui.label(RichText::new(text).color(egui::Color32::GREEN).font(
                                                       FontId::new(14.0, FontFamily::Monospace)));
                                                   let text = format!("{}", s);
                                                   ui.label(RichText::new(text).font(
                                                       FontId::new(14.0, FontFamily::Monospace)));
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
                        ui.add(egui::Image::new(hacktica_dark.texture_id(ctx), [96.0, 38.0]));
                    } else {
                        ui.add(egui::Image::new(hacktica_light.texture_id(ctx), [96.0, 38.0]));
                    }
                    ui.add_space(50.0);
                    ui.add(egui::Image::new(wp.texture_id(ctx), [80.0, 38.0]));
                });
            });
        });
}