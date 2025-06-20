use crate::config::{send_latest_config, ConfigCommand, ThreadCommunication};
use crate::gui::application::{THzImageExplorer, Tab};
use crate::gui::threed_plot::{three_dimensional_plot_ui, CameraInputAllowed, OpacityThreshold};
use crate::gui::toggle_widget::toggle;
use crate::vec2;
use bevy::prelude::*;
use bevy_egui::egui::epaint;
use bevy_egui::egui::{self, Checkbox, DragValue, Stroke, Ui};
use bevy_voxel_plot::InstanceMaterialData;
use egui_plot::{GridMark, Line, LineStyle, Plot, PlotPoint, PlotPoints, VLine};
use ndarray::Array2;
use std::ops::RangeInclusive;

pub fn pulse_tab(
    ui: &mut Ui,
    height: f32,
    width: f32,
    spacing: f32,
    right_panel_width: f32,
    explorer: &mut THzImageExplorer,
    thread_communication: &mut ThreadCommunication,
) {
    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            ui.add_space(50.0);
            ui.add(Checkbox::new(
                &mut thread_communication.gui_settings.signal_1_visible,
                "",
            ));
            ui.colored_label(egui::Color32::RED, "— ");
            ui.label("Signal 1");
            ui.add_space(50.0);
            ui.add(Checkbox::new(
                &mut thread_communication.gui_settings.filtered_signal_1_visible,
                "",
            ));
            ui.colored_label(egui::Color32::BLUE, "— ");
            ui.label("Filtered Signal 1");
            ui.add_space(50.0);
            ui.add(Checkbox::new(
                &mut thread_communication.gui_settings.avg_signal_1_visible,
                "",
            ));
            ui.colored_label(egui::Color32::YELLOW, "--- ");
            ui.label("Averaged Signal 1");
        });

        if let Ok(read_guard) = thread_communication.data_lock.read() {
            explorer.data = read_guard.clone();
            // self.data.time = linspace::<f64>(self.tera_flash_conf.t_begin as f64,
            //                                  (self.tera_flash_conf.t_begin + self.tera_flash_conf.range) as f64, NUM_PULSE_LINES).collect();
        }

        let mut signal_1: Vec<[f64; 2]> = Vec::new();
        let mut filtered_signal_1: Vec<[f64; 2]> = Vec::new();
        let mut avg_signal_1: Vec<[f64; 2]> = Vec::new();

        let mut axis_display_offset_signal_1 = f64::NEG_INFINITY;
        let mut axis_display_offset_filtered_signal_1 = f64::NEG_INFINITY;
        let mut axis_display_offset_avg_signal_1 = f64::NEG_INFINITY;

        if thread_communication.gui_settings.signal_1_visible {
            axis_display_offset_signal_1 = explorer
                .data
                .signal_1
                .iter()
                .fold(f64::INFINITY, |ai, &bi| ai.min(bi as f64))
                .abs();
        }
        if thread_communication.gui_settings.filtered_signal_1_visible {
            axis_display_offset_filtered_signal_1 = explorer
                .data
                .filtered_signal_1
                .iter()
                .fold(f64::INFINITY, |ai, &bi| ai.min(bi as f64))
                .abs();
        }
        if thread_communication.gui_settings.avg_signal_1_visible {
            axis_display_offset_avg_signal_1 = explorer
                .data
                .avg_signal_1
                .iter()
                .fold(f64::INFINITY, |ai, &bi| ai.min(bi as f64))
                .abs();
        }

        let axis_display_offset = [
            axis_display_offset_signal_1,
            axis_display_offset_filtered_signal_1,
            axis_display_offset_avg_signal_1,
        ]
        .iter()
        .fold(f64::NEG_INFINITY, |ai, &bi| ai.max(bi))
            * 1.05;

        for i in 0..explorer.data.time.len() {
            signal_1.push([
                explorer.data.time[i] as f64,
                explorer.data.signal_1[i] as f64 + axis_display_offset,
            ]);
        }

        for i in 0..explorer
            .data
            .filtered_time
            .len()
            .min(explorer.data.filtered_signal_1.len())
        {
            filtered_signal_1.push([
                explorer.data.filtered_time[i] as f64,
                explorer.data.filtered_signal_1[i] as f64 + axis_display_offset,
            ]);
        }

        for i in 0..explorer
            .data
            .time
            .len()
            .min(explorer.data.avg_signal_1.len())
        {
            avg_signal_1.push([
                explorer.data.time[i] as f64,
                explorer.data.avg_signal_1[i] as f64 + axis_display_offset,
            ]);
        }

        let t_fmt = |x: GridMark, _range: &RangeInclusive<f64>| format!("{:4.2} ps", x.value);
        let axis_display_offset_2 = axis_display_offset;
        let s_fmt = move |y: GridMark, _range: &RangeInclusive<f64>| {
            format!("{:4.2} nA", y.value - axis_display_offset)
        };
        let label_fmt = move |s: &str, val: &PlotPoint| {
            format!(
                "{}\n{:4.2} ps\n{:4.2} a.u.",
                s,
                val.x,
                val.y - axis_display_offset_2
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
            if thread_communication.gui_settings.signal_1_visible {
                signal_plot_ui.line(
                    Line::new(PlotPoints::from(signal_1))
                        .color(egui::Color32::RED)
                        .style(LineStyle::Solid)
                        .width(2.0)
                        .name("signal 1"),
                );
            }
            if thread_communication.gui_settings.filtered_signal_1_visible {
                signal_plot_ui.line(
                    Line::new(PlotPoints::from(filtered_signal_1))
                        .color(egui::Color32::BLUE)
                        .style(LineStyle::Solid)
                        .width(2.0)
                        .name("filtered signal 1"),
                );
            }
            if thread_communication.gui_settings.avg_signal_1_visible {
                signal_plot_ui.line(
                    Line::new(PlotPoints::from(avg_signal_1))
                        .color(egui::Color32::YELLOW)
                        .style(LineStyle::Dashed { length: 10.0 })
                        .width(2.0)
                        .name("ref 1"),
                );
            }
        });

        ui.add_space(spacing);

        let signal_1_fft: Vec<[f64; 2]> = explorer
            .data
            .frequencies
            .iter()
            .zip(explorer.data.signal_1_fft.iter())
            .map(|(x, y)| {
                let fft = if thread_communication.gui_settings.log_plot {
                    20.0 * (*y + 1e-10).log(10.0)
                } else {
                    *y
                };
                // TODO: is this needed?
                // if fft < 0.0 {
                //     fft = 0.0;
                // }
                [*x as f64, fft as f64]
            })
            .collect();
        let filtered_signal_1_fft: Vec<[f64; 2]> = explorer
            .data
            .filtered_frequencies
            .iter()
            .zip(explorer.data.filtered_signal_1_fft.iter())
            .map(|(x, y)| {
                let fft = if thread_communication.gui_settings.log_plot {
                    20.0 * (*y + 1e-10).log(10.0)
                } else {
                    *y
                };
                // TODO: is this needed?
                // if fft < 0.0 {
                //     fft = 0.0;
                // }
                [*x as f64, fft as f64]
            })
            .collect();
        let avg_signal_1_fft: Vec<[f64; 2]> = explorer
            .data
            .frequencies
            .iter()
            .zip(explorer.data.avg_signal_1_fft.iter())
            .map(|(x, y)| {
                let fft = if thread_communication.gui_settings.log_plot {
                    20.0 * (*y + 1e-10).log(10.0)
                } else {
                    *y
                };
                // TODO: is this needed?
                // if fft < 0.0 {
                //     fft = 0.0;
                // }
                [*x as f64, fft as f64]
            })
            .collect();
        let phase_1_fft: Vec<[f64; 2]> = explorer
            .data
            .frequencies
            .iter()
            .zip(explorer.data.phase_1_fft.iter())
            .map(|(x, y)| [*x as f64, *y as f64])
            .collect();
        let filtered_phase_1_fft: Vec<[f64; 2]> = explorer
            .data
            .filtered_frequencies
            .iter()
            .zip(explorer.data.filtered_phase_fft.iter())
            .map(|(x, y)| [*x as f64, *y as f64])
            .collect();
        let avg_phase_1_fft: Vec<[f64; 2]> = explorer
            .data
            .frequencies
            .iter()
            .zip(explorer.data.avg_phase_fft.iter())
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

        let log_plot = thread_communication.gui_settings.log_plot;
        let phases_visible = thread_communication.gui_settings.phases_visible;

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
        let f_fmt = |x: GridMark, _range: &RangeInclusive<f64>| format!("{:4.2} THz", x.value);

        let mut fft_plot = Plot::new("fft")
            .height(height)
            .width(width)
            //.y_grid_spacer(log10_grid_spacer)
            .label_formatter(label_fmt)
            .y_axis_formatter(a_fmt)
            .x_axis_formatter(f_fmt)
            .include_x(0.0)
            .include_x(10.0);

        if !thread_communication.gui_settings.phases_visible {
            // fft_plot = fft_plot.include_y(-100.0);
            fft_plot = fft_plot.include_y(0.0)
        };

        fft_plot.show(ui, |fft_plot_ui| {
            if thread_communication.gui_settings.signal_1_visible {
                if !thread_communication.gui_settings.phases_visible {
                    fft_plot_ui.line(
                        Line::new(PlotPoints::from(signal_1_fft))
                            .color(egui::Color32::RED)
                            .style(LineStyle::Solid)
                            .width(2.0)
                            .name("amplitude 1"),
                    );
                } else {
                    fft_plot_ui.line(
                        Line::new(PlotPoints::from(phase_1_fft))
                            .color(egui::Color32::RED)
                            .style(LineStyle::Solid)
                            .width(2.0)
                            .name("phase 1"),
                    );
                }
            }
            if thread_communication.gui_settings.filtered_signal_1_visible {
                if !thread_communication.gui_settings.phases_visible {
                    fft_plot_ui.line(
                        Line::new(PlotPoints::from(filtered_signal_1_fft))
                            .color(egui::Color32::BLUE)
                            .style(LineStyle::Dashed { length: 10.0 })
                            .width(2.0)
                            .name("filtered amplitude 1"),
                    )
                } else {
                    fft_plot_ui.line(
                        Line::new(PlotPoints::from(filtered_phase_1_fft))
                            .color(egui::Color32::BLUE)
                            .style(LineStyle::Dashed { length: 10.0 })
                            .width(2.0)
                            .name("filtered phase 1"),
                    );
                }
            }
            if thread_communication.gui_settings.avg_signal_1_visible {
                if !thread_communication.gui_settings.phases_visible {
                    fft_plot_ui.line(
                        Line::new(PlotPoints::from(avg_signal_1_fft))
                            .color(egui::Color32::YELLOW)
                            .style(LineStyle::Dashed { length: 10.0 })
                            .width(2.0)
                            .name("avg amplitude 1"),
                    )
                } else {
                    fft_plot_ui.line(
                        Line::new(PlotPoints::from(avg_phase_1_fft))
                            .color(egui::Color32::YELLOW)
                            .style(LineStyle::Dashed { length: 10.0 })
                            .width(2.0)
                            .name("avg phase 1"),
                    );
                }
            }

            if thread_communication.gui_settings.water_lines_visible {
                for line in explorer.water_vapour_lines.iter() {
                    fft_plot_ui.vline(
                        VLine::new(*line)
                            .stroke(Stroke::new(1.0, egui::Color32::BLUE))
                            .width(2.0)
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
                    DragValue::new(
                        &mut thread_communication.gui_settings.frequency_resolution_temp,
                    )
                    .min_decimals(4)
                    .max_decimals(4)
                    .suffix(" THz".to_string()),
                )
                .lost_focus()
            {
                if thread_communication.gui_settings.frequency_resolution_temp
                    > 1.0 / explorer.data.hk.range
                {
                    thread_communication.gui_settings.frequency_resolution_temp =
                        1.0 / explorer.data.hk.range;
                } else if thread_communication.gui_settings.frequency_resolution_temp < 0.0001 {
                    thread_communication.gui_settings.frequency_resolution_temp = 0.0001;
                }
                thread_communication.gui_settings.frequency_resolution =
                    thread_communication.gui_settings.frequency_resolution_temp;
                send_latest_config(
                    thread_communication,
                    ConfigCommand::SetFFTResolution(
                        thread_communication.gui_settings.frequency_resolution,
                    ),
                );
            }
            ui.add_space(50.0);
            ui.label("FFT");
            if ui
                .add(toggle(
                    &mut thread_communication.gui_settings.phases_visible,
                ))
                .changed()
            {
                thread_communication.gui_settings.log_plot =
                    !thread_communication.gui_settings.phases_visible;
            };
            ui.label("Phases");
            ui.add_space(50.0);
            ui.add(Checkbox::new(
                &mut thread_communication.gui_settings.water_lines_visible,
                "",
            ));
            ui.colored_label(egui::Color32::BLUE, "— ");
            ui.label("Water Lines");

            ui.add_space(ui.available_size().x - 400.0 - right_panel_width);

            // dynamic range:
            let length = explorer.data.signal_1_fft.len();
            let dr1 = if !explorer.data.signal_1_fft.is_empty() {
                explorer.data.signal_1_fft[length - 100..length]
                    .iter()
                    .sum::<f32>()
                    / 100.0
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
                explorer.data.signal_1.iter().cloned().reduce(f32::min),
                explorer.data.signal_1.iter().cloned().reduce(f32::max),
            ) {
                max - min
            } else {
                0.0
            };
            ui.label(format!("ptp: CH1 {:.1} nA", ptp1));
        });
    });
}

pub fn refractive_index_tab(
    ui: &mut Ui,
    height: f32,
    width: f32,
    spacing: f32,
    right_panel_width: f32,
    explorer: &mut THzImageExplorer,
    thread_communication: &mut ThreadCommunication,
) {
    // ui.vertical(|ui| {
    //     // Signal selection controls
    //     ui.horizontal(|ui| {
    //         ui.label("Reference:");
    //         egui::ComboBox::from_id_source("reference_selection")
    //             .selected_text(format!("Reference {}", thread_communication.gui_settings.reference_index + 1))
    //             .width(120.0)
    //             .show_ui(ui, |ui| {
    //                 for i in 0..explorer.data.available_references.len() {
    //                     if ui.selectable_label(
    //                         thread_communication.gui_settings.reference_index == i,
    //                         format!("Reference {}", i + 1),
    //                     ).clicked() {
    //                         thread_communication.gui_settings.reference_index = i;
    //                         thread_communication.config_tx
    //                             .send(ConfigCommand::UpdateMaterialCalculation)
    //                             .unwrap();
    //                     }
    //                 }
    //             });
    //
    //         ui.add_space(20.0);
    //
    //         ui.label("Sample:");
    //         egui::ComboBox::from_id_source("sample_selection")
    //             .selected_text(format!("Sample {}", thread_communication.gui_settings.sample_index + 1))
    //             .width(120.0)
    //             .show_ui(ui, |ui| {
    //                 for i in 0..explorer.data.available_samples.len() {
    //                     if ui.selectable_label(
    //                         thread_communication.gui_settings.sample_index == i,
    //                         format!("Sample {}", i + 1),
    //                     ).clicked() {
    //                         thread_communication.gui_settings.sample_index = i;
    //                         thread_communication.config_tx
    //                             .send(ConfigCommand::UpdateMaterialCalculation)
    //                             .unwrap();
    //                     }
    //                 }
    //             });
    //     });
    //
    //     // Visibility toggles
    //     ui.horizontal(|ui| {
    //         ui.add_space(50.0);
    //         ui.add(Checkbox::new(
    //             &mut thread_communication.gui_settings.refractive_index_visible,
    //             "",
    //         ));
    //         ui.colored_label(egui::Color32::RED, "— ");
    //         ui.label("Refractive Index (n)");
    //
    //         ui.add_space(50.0);
    //         ui.add(Checkbox::new(
    //             &mut thread_communication.gui_settings.extinction_coefficient_visible,
    //             "",
    //         ));
    //         ui.colored_label(egui::Color32::BLUE, "— ");
    //         ui.label("Extinction Coefficient (k)");
    //
    //         ui.add_space(50.0);
    //         ui.add(Checkbox::new(
    //             &mut thread_communication.gui_settings.absorption_visible,
    //             "",
    //         ));
    //         ui.colored_label(egui::Color32::GREEN, "— ");
    //         ui.label("Absorption (α)");
    //     });
    //
    //     if let Ok(read_guard) = thread_communication.data_lock.read() {
    //         explorer.data = read_guard.clone();
    //     }
    //
    //     // Refractive index plot data
    //     let refractive_index: Vec<[f64; 2]> = explorer
    //         .data
    //         .frequencies
    //         .iter()
    //         .zip(explorer.data.refractive_index.iter())
    //         .map(|(x, y)| [*x as f64, *y as f64])
    //         .collect();
    //
    //     let extinction_coefficient: Vec<[f64; 2]> = explorer
    //         .data
    //         .frequencies
    //         .iter()
    //         .zip(explorer.data.extinction_coefficient.iter())
    //         .map(|(x, y)| [*x as f64, *y as f64])
    //         .collect();
    //
    //     let absorption: Vec<[f64; 2]> = explorer
    //         .data
    //         .frequencies
    //         .iter()
    //         .zip(explorer.data.absorption.iter())
    //         .map(|(x, y)| [*x as f64, *y as f64])
    //         .collect();
    //
    //     // Format functions for the plots
    //     let f_fmt = |x: GridMark, _range: &RangeInclusive<f64>| format!("{:4.2} THz", x.value);
    //     let n_fmt = |y: GridMark, _range: &RangeInclusive<f64>| format!("{:4.2}", y.value);
    //     let label_fmt = |s: &str, val: &PlotPoint| {
    //         format!("{}\n{:4.2} THz\n{:4.2}", s, val.x, val.y)
    //     };
    //
    //     // Refractive index plot
    //     let n_plot = Plot::new("refractive_index")
    //         .height(height)
    //         .width(width)
    //         .y_axis_formatter(n_fmt)
    //         .x_axis_formatter(f_fmt)
    //         .label_formatter(label_fmt)
    //         .include_x(0.0)
    //         .include_x(explorer.data.frequencies.last().unwrap_or(&10.0) * 1.05);
    //
    //     n_plot.show(ui, |plot_ui| {
    //         if thread_communication.gui_settings.refractive_index_visible {
    //             plot_ui.line(
    //                 Line::new(PlotPoints::from(refractive_index))
    //                     .color(egui::Color32::RED)
    //                     .style(LineStyle::Solid)
    //                     .width(2.0)
    //                     .name("Refractive Index (n)"),
    //             );
    //         }
    //
    //         if thread_communication.gui_settings.extinction_coefficient_visible {
    //             plot_ui.line(
    //                 Line::new(PlotPoints::from(extinction_coefficient))
    //                     .color(egui::Color32::BLUE)
    //                     .style(LineStyle::Solid)
    //                     .width(2.0)
    //                     .name("Extinction Coefficient (k)"),
    //             );
    //         }
    //     });
    //
    //     ui.add_space(spacing);
    //
    //     // Absorption plot
    //     let a_fmt = |y: GridMark, _range: &RangeInclusive<f64>| format!("{:4.2} cm⁻¹", y.value);
    //     let a_label_fmt = |s: &str, val: &PlotPoint| {
    //         format!("{}\n{:4.2} THz\n{:4.2} cm⁻¹", s, val.x, val.y)
    //     };
    //
    //     let absorption_plot = Plot::new("absorption")
    //         .height(height)
    //         .width(width)
    //         .y_axis_formatter(a_fmt)
    //         .x_axis_formatter(f_fmt)
    //         .label_formatter(a_label_fmt)
    //         .include_x(0.0)
    //         .include_x(explorer.data.frequencies.last().unwrap_or(&10.0) * 1.05);
    //
    //     absorption_plot.show(ui, |plot_ui| {
    //         if thread_communication.gui_settings.absorption_visible {
    //             plot_ui.line(
    //                 Line::new(PlotPoints::from(absorption))
    //                     .color(egui::Color32::GREEN)
    //                     .style(LineStyle::Solid)
    //                     .width(2.0)
    //                     .name("Absorption (α)"),
    //             );
    //         }
    //
    //         if thread_communication.gui_settings.water_lines_visible {
    //             for line in explorer.water_vapour_lines.iter() {
    //                 plot_ui.vline(
    //                     VLine::new(*line)
    //                         .stroke(Stroke::new(1.0, egui::Color32::BLUE))
    //                         .width(2.0)
    //                         .name("water vapour"),
    //                 );
    //             }
    //         }
    //     });
    //
    //     // Bottom controls
    //     ui.add_space(5.0);
    //     ui.horizontal(|ui| {
    //         ui.label("Sample Thickness:");
    //         if ui
    //             .add(
    //                 DragValue::new(&mut thread_communication.gui_settings.sample_thickness)
    //                     .min_decimals(2)
    //                     .max_decimals(2)
    //                     .suffix(" mm"),
    //             )
    //             .changed()
    //         {
    //             thread_communication.config_tx
    //                 .send(ConfigCommand::UpdateMaterialCalculation)
    //                 .unwrap();
    //         }
    //
    //         ui.add_space(50.0);
    //         ui.add(Checkbox::new(
    //             &mut thread_communication.gui_settings.water_lines_visible,
    //             "",
    //         ));
    //         ui.colored_label(egui::Color32::BLUE, "— ");
    //         ui.label("Water Lines");
    //
    //         ui.add_space(ui.available_size().x - 400.0 - right_panel_width);
    //
    //         // Display material statistics if available
    //         if let Some(max_n) = explorer.data.refractive_index.iter().cloned().reduce(f32::max) {
    //             ui.label(format!("Max n: {:.2}", max_n));
    //         }
    //
    //         ui.add_space(50.0);
    //
    //         if let Some(max_alpha) = explorer.data.absorption.iter().cloned().reduce(f32::max) {
    //             ui.label(format!("Max α: {:.2} cm⁻¹", max_alpha));
    //         }
    //     });
    // });
}

#[allow(clippy::too_many_arguments)]
pub fn center_panel(
    meshes: &mut ResMut<Assets<Mesh>>,
    query: &mut Query<(&mut InstanceMaterialData, &mut Mesh3d)>,
    cube_preview_texture_id: &epaint::TextureId,
    ctx: &egui::Context,
    right_panel_width: &f32,
    left_panel_width: &f32,
    explorer: &mut THzImageExplorer,
    opacity_threshold: &mut ResMut<OpacityThreshold>,
    cam_input: &mut ResMut<CameraInputAllowed>,
    thread_communication: &mut ResMut<ThreadCommunication>,
) {
    egui::CentralPanel::default().show(ctx, |ui| {
        let window_height = ui.available_height();
        let height = ui.available_size().y * 0.45 - 10.0;
        let spacing = (ui.available_size().y - 2.0 * height) / 3.0 - 10.0;
        let width = ui.available_size().x - 40.0 - *left_panel_width - *right_panel_width;
        ui.horizontal(|ui| {
            ui.add_space(*left_panel_width + 20.0);
            let tabs = thread_communication.gui_settings.tab.to_arr();
            if ui
                .selectable_label(tabs[0], Tab::Pulse.to_string())
                .clicked()
            {
                thread_communication.gui_settings.tab = Tab::Pulse;
            }
            if ui
                .selectable_label(tabs[1], Tab::RefractiveIndex.to_string())
                .clicked()
            {
                thread_communication.gui_settings.tab = Tab::RefractiveIndex;
            }
            if ui
                .selectable_label(tabs[2], Tab::ThreeD.to_string())
                .clicked()
            {
                thread_communication.gui_settings.tab = Tab::ThreeD;
            }
        });

        ui.add_space(5.0);

        ui.horizontal(|ui| {
            ui.add_space(*left_panel_width + 20.0);
            match thread_communication.gui_settings.tab {
                Tab::Pulse => pulse_tab(
                    ui,
                    height,
                    width,
                    spacing,
                    *right_panel_width,
                    explorer,
                    thread_communication,
                ),
                Tab::RefractiveIndex => refractive_index_tab(
                    ui,
                    height,
                    width,
                    spacing,
                    *right_panel_width,
                    explorer,
                    thread_communication,
                ),
                Tab::ThreeD => {
                    three_dimensional_plot_ui(
                        meshes,
                        cube_preview_texture_id,
                        width,
                        window_height,
                        ui,
                        query,
                        opacity_threshold,
                        cam_input,
                        thread_communication,
                    );
                }
            }
        });
    });
}

#[allow(dead_code)]
fn generate_sample_image(width: usize, height: usize) -> Array2<f32> {
    let mut image = Array2::zeros((width, height));
    let center_x = width as f32 / 2.0;
    let center_y = height as f32 / 2.0;

    for x in 0..width {
        for y in 0..height {
            let dx = (x as f32 - center_x) / center_x;
            let dy = (y as f32 - center_y) / center_y;
            image[[x, y]] = (-10.0 * (dx * dx + dy * dy)).exp(); // Gaussian function
        }
    }
    image
}
