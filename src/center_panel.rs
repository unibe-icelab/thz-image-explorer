use std::f64::consts::E;
use std::sync::{Arc, RwLock};
use std::ops::RangeInclusive;
use eframe::egui;
use eframe::egui::{Checkbox, Stroke, DragValue};
use eframe::egui::plot::{Line, LineStyle, Plot, PlotPoint, PlotPoints, VLine};
use crate::{GuiSettingsContainer, vec2};
use crate::data::DataContainer;
use crate::toggle::toggle;

pub fn center_panel(ctx: &egui::Context,
                    right_panel_width: &f32,
                    left_panel_width: &f32,
                    gui_conf: &mut GuiSettingsContainer,
                    data: &mut DataContainer,
                    df_lock: &Arc<RwLock<f64>>,
                    data_lock: &Arc<RwLock<DataContainer>>,
                    water_vapour_lines: &Vec<f64>,
) {
    egui::CentralPanel::default().show(ctx, |ui| {
        let height = ui.available_size().y * 0.45;
        let spacing = (ui.available_size().y - 2.0 * height) / 3.0 - 10.0;
        let width = ui.available_size().x - 40.0 - *left_panel_width - *right_panel_width;
        let mut plot_color = egui::Color32::YELLOW;
        if !gui_conf.dark_mode {
            plot_color = egui::Color32::BLUE;
        }
        ui.add_space(spacing);
        ui.horizontal(|ui| {
            ui.add_space(*left_panel_width + 20.0);
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.add_space(50.0);
                    ui.add(Checkbox::new(&mut gui_conf.signal_1_visible, ""));
                    ui.colored_label(egui::Color32::RED, "— ");
                    ui.label("Signal 1");
                    ui.add_space(50.0);
                    ui.add(Checkbox::new(&mut gui_conf.filtered_signal_1_visible, ""));
                    ui.colored_label(egui::Color32::BLUE, "— ");
                    ui.label("Filtered Signal 1");
                    ui.add_space(50.0);
                    ui.add(Checkbox::new(&mut gui_conf.ref_1_visible, ""));
                    ui.colored_label(egui::Color32::RED, "--- ");
                    ui.label("Ref 1");
                });

                if let Ok(read_guard) = data_lock.read() {
                    *data = read_guard.clone();
                    // self.data.time = linspace::<f64>(self.tera_flash_conf.t_begin as f64,
                    //                                  (self.tera_flash_conf.t_begin + self.tera_flash_conf.range) as f64, NUM_PULSE_LINES).collect();
                }

                let mut signal_1: Vec<[f64; 2]> = Vec::new();
                let mut filtered_signal_1: Vec<[f64; 2]> = Vec::new();
                let mut ref_1: Vec<[f64; 2]> = Vec::new();

                let mut axis_display_offset_signal_1 = f64::NEG_INFINITY;
                let mut axis_display_offset_filtered_signal_1 = f64::NEG_INFINITY;
                let mut axis_display_offset_ref_1 = f64::NEG_INFINITY;

                if gui_conf.signal_1_visible {
                    axis_display_offset_signal_1 = data.signal_1.iter().fold(f64::INFINITY, |ai, &bi| ai.min(bi)).abs();
                }
                if gui_conf.filtered_signal_1_visible {
                    axis_display_offset_filtered_signal_1 = data.filtered_signal_1.iter().fold(f64::INFINITY, |ai, &bi| ai.min(bi)).abs();
                }
                if gui_conf.ref_1_visible {
                    axis_display_offset_ref_1 = data.ref_1.iter().fold(f64::INFINITY, |ai, &bi| ai.min(bi)).abs();
                }

                let axis_display_offset = vec![axis_display_offset_ref_1, axis_display_offset_signal_1, axis_display_offset_filtered_signal_1]
                    .iter()
                    .fold(f64::NEG_INFINITY, |ai, &bi| ai.max(bi)) * 1.05;

                for i in 0..data.time.len() {
                    signal_1.push([data.time[i] as f64, (data.signal_1[i] + axis_display_offset) as f64]);
                    ref_1.push([data.time[i] as f64, (data.ref_1[i] + axis_display_offset) as f64]);
                }

                for i in 0..data.time.len().min(data.filtered_signal_1.len()) {
                    filtered_signal_1.push([data.time[i] as f64, (data.filtered_signal_1[i] + axis_display_offset) as f64]);
                }

                let t_fmt = |x, _range: &RangeInclusive<f64>| {
                    format!("{:4.2} ps", x)
                };
                let axis_display_offset_2 = axis_display_offset.clone();
                let s_fmt = move |y, _range: &RangeInclusive<f64>| {
                    format!("{:4.2} a.u.", y - axis_display_offset as f64)
                };
                let label_fmt = move |s: &str, val: &PlotPoint| {
                    format!("{}\n{:4.2} ps\n{:4.2} a.u.", s, val.x, val.y - axis_display_offset_2 as f64)
                };

                let signal_plot = Plot::new("signal")
                    .height(height)
                    .width(width)
                    .y_axis_formatter(s_fmt)
                    .x_axis_formatter(t_fmt)
                    .label_formatter(label_fmt)
                    //.coordinates_formatter(Corner::LeftTop, position_fmt)
                    //.include_x(&self.tera_flash_conf.t_begin + &self.tera_flash_conf.range)
                    //.include_x(self.tera_flash_conf.t_begin)
                    .min_size(vec2(50.0, 100.0));


                signal_plot.show(ui, |signal_plot_ui| {
                    if gui_conf.signal_1_visible {
                        signal_plot_ui.line(Line::new(PlotPoints::from(signal_1))
                            .color(egui::Color32::RED)
                            .style(LineStyle::Solid)
                            .name("signal 1"));
                    }
                    if gui_conf.filtered_signal_1_visible {
                        signal_plot_ui.line(Line::new(PlotPoints::from(filtered_signal_1))
                            .color(egui::Color32::BLUE)
                            .style(LineStyle::Solid)
                            .name("filtered signal 1"));
                    }
                    if gui_conf.ref_1_visible {
                        signal_plot_ui.line(Line::new(PlotPoints::from(ref_1))
                            .color(egui::Color32::RED)
                            .style(LineStyle::Dashed { length: 10.0 })
                            .name("ref 1"));
                    }
                });

                ui.add_space(spacing);

                let f_fmt = |x, _range: &RangeInclusive<f64>| {
                    format!("{:4.2} THz", x)
                };
                let a_fmt = |y: f64, _range: &RangeInclusive<f64>| {
                    format!("{:4.2} a.u.", y.exp())
                };
                let fft_plot = Plot::new("fft")
                    .height(height)
                    .width(width)
                    .y_axis_formatter(a_fmt)
                    .x_axis_formatter(f_fmt)
                    .include_y(0.0)
                    .include_x(0.0)
                    .include_x(10.0);

                let signal_1_fft: Vec<[f64; 2]> = data.frequencies_fft.iter()
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
                let filtered_signal_1_fft: Vec<[f64; 2]> = data.frequencies_fft.iter()
                    .zip(data.filtered_signal_1_fft.iter())
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
                let ref_1_fft: Vec<[f64; 2]> = data.frequencies_fft.iter()
                    .zip(data.ref_1_fft.iter())
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

                let phase_1_fft: Vec<[f64; 2]> = data.frequencies_fft.iter()
                    .zip(data.phase_1_fft.iter())
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
                let ref_phase_1_fft: Vec<[f64; 2]> = data.frequencies_fft.iter()
                    .zip(data.ref_phase_1_fft.iter())
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

                fft_plot.show(ui, |fft_plot_ui| {
                    if gui_conf.signal_1_visible {
                        if !gui_conf.phases_visible {
                            fft_plot_ui.line(Line::new(PlotPoints::from(signal_1_fft))
                                .color(egui::Color32::RED)
                                .style(LineStyle::Solid)
                                .name("signal 1"));
                        } else {
                            fft_plot_ui.line(Line::new(PlotPoints::from(phase_1_fft))
                                .color(egui::Color32::RED)
                                .style(LineStyle::Solid)
                                .name("phase 1"));
                        }
                    }
                    if gui_conf.filtered_signal_1_visible {
                        fft_plot_ui.line(Line::new(PlotPoints::from(filtered_signal_1_fft))
                            .color(egui::Color32::BLUE)
                            .style(LineStyle::Solid)
                            .name("filtered signal 1"));
                    }
                    if gui_conf.ref_1_visible {
                        if !gui_conf.phases_visible {
                            fft_plot_ui.line(Line::new(PlotPoints::from(ref_1_fft))
                                .color(egui::Color32::RED)
                                .style(LineStyle::Dashed { length: 10.0 })
                                .name("ref 1"));
                        } else {
                            fft_plot_ui.line(Line::new(PlotPoints::from(ref_phase_1_fft))
                                .color(egui::Color32::RED)
                                .style(LineStyle::Dashed { length: 10.0 })
                                .name("ref phase 1"));
                        }
                    }

                    if gui_conf.water_lines_visible {
                        for line in water_vapour_lines.iter() {
                            fft_plot_ui.vline(VLine::new(*line)
                                .stroke(Stroke::new(1.0, egui::Color32::BLUE))
                                .name("water vapour"));
                        }
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Freq Res");
                    if ui.add(DragValue::new(&mut gui_conf.frequency_resolution_temp)
                        .min_decimals(4)
                        .max_decimals(4)
                        .suffix(" THz".to_string())
                    ).lost_focus() {
                        // TODO: get range from dataset!
                        if gui_conf.frequency_resolution_temp > 1.0 / data.hk.range {
                            gui_conf.frequency_resolution_temp = 1.0 / data.hk.range;
                        } else if gui_conf.frequency_resolution_temp < 0.0001 {
                            gui_conf.frequency_resolution_temp = 0.0001;
                        }
                        gui_conf.frequency_resolution = gui_conf.frequency_resolution_temp;
                        if let Ok(mut write_guard) = df_lock.write() {
                            *write_guard = gui_conf.frequency_resolution;
                        }
                    }
                    ui.add_space(50.0);
                    ui.label("FFT");
                    ui.add(toggle(&mut gui_conf.phases_visible));
                    ui.label("Phases");
                    ui.add_space(50.0);
                    ui.add(Checkbox::new(&mut gui_conf.water_lines_visible, ""));
                    ui.colored_label(egui::Color32::BLUE, "— ");
                    ui.label("Water Lines");
                });
            });
        });
        ctx.request_repaint()
    });
}