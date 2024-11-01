use eframe::egui;
use eframe::egui::{Checkbox, DragValue, Stroke};
use egui_plot::{GridMark, Line, LineStyle, Plot, PlotPoint, PlotPoints, VLine};
use std::ops::RangeInclusive;
use std::sync::mpsc::Sender;
use std::sync::{Arc, RwLock};

use crate::config::Config;
use crate::data::DataPoint;
use crate::toggle::toggle;
use crate::{vec2, GuiSettingsContainer};

#[allow(clippy::too_many_arguments)]
pub fn center_panel(
    ctx: &egui::Context,
    right_panel_width: &f32,
    left_panel_width: &f32,
    gui_conf: &mut GuiSettingsContainer,
    data: &mut DataPoint,
    data_lock: &Arc<RwLock<DataPoint>>,
    config_tx: &Sender<Config>,
    water_vapour_lines: &Vec<f64>,
) {
    egui::CentralPanel::default().show(ctx, |ui| {
        let height = ui.available_size().y * 0.45;
        let spacing = (ui.available_size().y - 2.0 * height) / 3.0 - 10.0;
        let width = ui.available_size().x - 40.0 - *left_panel_width - *right_panel_width;
        let mut plot_color;
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
                let ref_1: Vec<[f64; 2]> = Vec::new();

                let mut axis_display_offset_signal_1 = f64::NEG_INFINITY;
                let mut axis_display_offset_filtered_signal_1 = f64::NEG_INFINITY;

                if gui_conf.signal_1_visible {
                    axis_display_offset_signal_1 = data
                        .signal_1
                        .iter()
                        .fold(f64::INFINITY, |ai, &bi| ai.min(bi as f64))
                        .abs();
                }
                if gui_conf.filtered_signal_1_visible {
                    axis_display_offset_filtered_signal_1 = data
                        .filtered_signal_1
                        .iter()
                        .fold(f64::INFINITY, |ai, &bi| ai.min(bi as f64))
                        .abs();
                }

                let axis_display_offset = [
                    axis_display_offset_signal_1,
                    axis_display_offset_filtered_signal_1,
                ]
                .iter()
                .fold(f64::NEG_INFINITY, |ai, &bi| ai.max(bi))
                    * 1.05;

                for i in 0..data.time.len() {
                    signal_1.push([
                        data.time[i] as f64,
                        data.signal_1[i] as f64 + axis_display_offset,
                    ]);
                }

                for i in 0..data.time.len().min(data.filtered_signal_1.len()) {
                    filtered_signal_1.push([
                        data.time[i] as f64,
                        data.filtered_signal_1[i] as f64 + axis_display_offset,
                    ]);
                }

                let t_fmt =
                    |x: GridMark, _range: &RangeInclusive<f64>| format!("{:4.2} ps", x.value);
                let axis_display_offset_2 = axis_display_offset;
                let s_fmt = move |y: GridMark, _range: &RangeInclusive<f64>| {
                    format!("{:4.2} nA", y.value - axis_display_offset)
                };
                let label_fmt = move |s: &str, val: &PlotPoint| {
                    format!(
                        "{}\n{:4.2} ps\n{:4.2} a.u.",
                        s,
                        val.x,
                        val.y - axis_display_offset_2 as f64
                    )
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
                        signal_plot_ui.line(
                            Line::new(PlotPoints::from(signal_1))
                                .color(egui::Color32::RED)
                                .style(LineStyle::Solid)
                                .name("signal 1"),
                        );
                    }
                    if gui_conf.filtered_signal_1_visible {
                        signal_plot_ui.line(
                            Line::new(PlotPoints::from(filtered_signal_1))
                                .color(egui::Color32::BLUE)
                                .style(LineStyle::Solid)
                                .name("filtered signal 1"),
                        );
                    }
                    if gui_conf.ref_1_visible {
                        signal_plot_ui.line(
                            Line::new(PlotPoints::from(ref_1))
                                .color(egui::Color32::RED)
                                .style(LineStyle::Dashed { length: 10.0 })
                                .name("ref 1"),
                        );
                    }
                });

                ui.add_space(spacing);

                let signal_1_fft: Vec<[f64; 2]> = data
                    .frequencies
                    .iter()
                    .zip(data.signal_1_fft.iter())
                    .map(|(x, y)| {
                        let fft;
                        if gui_conf.log_plot {
                            fft = 20.0 * (*y + 1e-10).log(10.0);
                        } else {
                            fft = *y;
                        }
                        // TODO: is this needed?
                        // if fft < 0.0 {
                        //     fft = 0.0;
                        // }
                        [*x as f64, fft as f64]
                    })
                    .collect();
                let filtered_signal_1_fft: Vec<[f64; 2]> = data
                    .frequencies
                    .iter()
                    .zip(data.filtered_signal_1_fft.iter())
                    .map(|(x, y)| {
                        let fft;
                        if gui_conf.log_plot {
                            fft = 20.0 * (*y + 1e-10).log(10.0);
                        } else {
                            fft = *y;
                        }
                        // TODO: is this needed?
                        // if fft < 0.0 {
                        //     fft = 0.0;
                        // }
                        [*x as f64, fft as f64]
                    })
                    .collect();
                let phase_1_fft: Vec<[f64; 2]> = data
                    .frequencies
                    .iter()
                    .zip(data.phase_1_fft.iter())
                    .map(|(x, y)| [*x as f64, *y as f64])
                    .collect();
                let filtered_phase_1_fft: Vec<[f64; 2]> = data
                    .frequencies
                    .iter()
                    .zip(data.filtered_phase_fft.iter())
                    .map(|(x, y)| [*x as f64, *y as f64])
                    .collect();

                let fft_signals = [&signal_1_fft];

                let mut max_fft_signals = fft_signals
                    .iter()
                    .flat_map(|v| v.iter().copied())
                    .map(|x| x[1])
                    .fold(f64::MIN, |a, b| a.max(b));

                if max_fft_signals < -200.0 {
                    max_fft_signals = -200.0;
                }

                let log_plot = gui_conf.log_plot.clone();
                let phases_visible = gui_conf.phases_visible.clone();

                let a_fmt = move |y: GridMark, _range: &RangeInclusive<f64>| {
                    if log_plot {
                        if phases_visible {
                            format!("{:4.2} °", y.value)
                        } else {
                            format!("{:4.2} dB", y.value - max_fft_signals)
                        }
                    } else {
                        format!("{:4.2} a.u.", y.value)
                    }
                };

                let label_fmt = move |s: &str, val: &PlotPoint| {
                    if log_plot {
                        if phases_visible {
                            format!("{}\n{:4.2} THz\n{:4.2} °", s, val.x, val.y)
                        } else {
                            format!(
                                "{}\n{:4.2} THz\n{:4.2} dB",
                                s,
                                val.x,
                                val.y - max_fft_signals
                            )
                        }
                    } else {
                        format!("{}\n{:4.2} THz\n{:4.2} a.u.", s, val.x, val.y)
                    }
                };
                let f_fmt =
                    |x: GridMark, _range: &RangeInclusive<f64>| format!("{:4.2} THz", x.value);

                let mut fft_plot = Plot::new("fft")
                    .height(height)
                    .width(width)
                    //.y_grid_spacer(log10_grid_spacer)
                    .label_formatter(label_fmt)
                    .y_axis_formatter(a_fmt)
                    .x_axis_formatter(f_fmt)
                    .include_x(0.0)
                    .include_x(10.0);

                if !gui_conf.phases_visible {
                    // fft_plot = fft_plot.include_y(-100.0);
                    fft_plot = fft_plot.include_y(0.0)
                };

                fft_plot.show(ui, |fft_plot_ui| {
                    if gui_conf.signal_1_visible {
                        if !gui_conf.phases_visible {
                            fft_plot_ui.line(
                                Line::new(PlotPoints::from(signal_1_fft))
                                    .color(egui::Color32::RED)
                                    .style(LineStyle::Solid)
                                    .name("signal 1"),
                            );
                        } else {
                            fft_plot_ui.line(
                                Line::new(PlotPoints::from(phase_1_fft))
                                    .color(egui::Color32::RED)
                                    .style(LineStyle::Solid)
                                    .name("phase 1"),
                            );
                        }
                    }
                    if gui_conf.ref_1_visible {
                        if !gui_conf.phases_visible {
                            fft_plot_ui.line(
                                Line::new(PlotPoints::from(filtered_signal_1_fft))
                                    .color(egui::Color32::RED)
                                    .style(LineStyle::Dashed { length: 10.0 })
                                    .name("ref 1"),
                            );
                        } else {
                            fft_plot_ui.line(
                                Line::new(PlotPoints::from(filtered_phase_1_fft))
                                    .color(egui::Color32::RED)
                                    .style(LineStyle::Dashed { length: 10.0 })
                                    .name("ref phase 1"),
                            );
                        }
                    }

                    if gui_conf.water_lines_visible {
                        for line in water_vapour_lines.iter() {
                            fft_plot_ui.vline(
                                VLine::new(*line)
                                    .stroke(Stroke::new(1.0, egui::Color32::BLUE))
                                    .name("water vapour"),
                            );
                        }
                    }
                });
                ui.add_space(5.0);
                ui.horizontal(|ui| {
                    ui.label("Freq Res");
                    if ui
                        .add(
                            DragValue::new(&mut gui_conf.frequency_resolution_temp)
                                .min_decimals(4)
                                .max_decimals(4)
                                .suffix(" THz".to_string()),
                        )
                        .lost_focus()
                    {
                        if gui_conf.frequency_resolution_temp > 1.0 / data.hk.range {
                            gui_conf.frequency_resolution_temp = 1.0 / data.hk.range;
                        } else if gui_conf.frequency_resolution_temp < 0.0001 {
                            gui_conf.frequency_resolution_temp = 0.0001;
                        }
                        gui_conf.frequency_resolution = gui_conf.frequency_resolution_temp;
                        config_tx
                            .send(Config::SetFFTResolution(gui_conf.frequency_resolution))
                            .expect("unable to send config");
                    }
                    ui.add_space(50.0);
                    ui.label("FFT");
                    if ui.add(toggle(&mut gui_conf.phases_visible)).changed() {
                        if gui_conf.phases_visible {
                            gui_conf.log_plot = false;
                        } else {
                            gui_conf.log_plot = true;
                        }
                    };
                    ui.label("Phases");
                    ui.add_space(50.0);
                    ui.add(Checkbox::new(&mut gui_conf.water_lines_visible, ""));
                    ui.colored_label(egui::Color32::BLUE, "— ");
                    ui.label("Water Lines");

                    ui.add_space(ui.available_size().x - 400.0 - right_panel_width);

                    // dynamic range:
                    let length = data.signal_1_fft.len();
                    let dr1 = if data.signal_1_fft.len() != 0 {
                        data.signal_1_fft[length - 100..length].iter().sum::<f32>() / 100.0
                    } else {
                        0.0
                    };
                    ui.label(format!(
                        "DR: CH1 {:.1} dB",
                        20.0 * (dr1.abs() + 1e-10).log(10.0) - max_fft_signals as f32,
                    ));

                    ui.add_space(50.0);

                    // peak to peak
                    let ptp1 = if let (Some(min), Some(max)) = (
                        data.signal_1.iter().cloned().reduce(f32::min),
                        data.signal_1.iter().cloned().reduce(f32::max),
                    ) {
                        max - min
                    } else {
                        0.0
                    };
                    ui.label(format!("ptp: CH1 {:.1} nA", ptp1));
                });
            });
        });
        ctx.request_repaint()
    });
}
