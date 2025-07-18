use crate::config::{send_latest_config, ConfigCommand, ThreadCommunication};
use crate::gui::application::{THzImageExplorer, Tab};
use crate::gui::threed_plot::{three_dimensional_plot_ui, CameraInputAllowed, OpacityThreshold};
use crate::gui::toggle_widget::toggle;
use crate::vec2;
use bevy::prelude::*;
use bevy_egui::egui::{self, Checkbox, DragValue, Slider, Stroke, Ui};
use bevy_egui::egui::{epaint, Popup, PopupCloseBehavior};
use egui_plot::{GridMark, Legend, Line, LineStyle, Plot, PlotPoint, PlotPoints, VLine};
use std::ops::RangeInclusive;

const ROI_COLORS: [egui::Color32; 8] = [
    egui::Color32::from_rgb(0, 255, 128),   // Mint green
    egui::Color32::from_rgb(128, 0, 128),   // Purple
    egui::Color32::from_rgb(0, 200, 200),   // Cyan
    egui::Color32::from_rgb(255, 0, 255),   // Magenta
    egui::Color32::from_rgb(255, 128, 0),   // Orange
    egui::Color32::from_rgb(128, 64, 0),    // Brown
    egui::Color32::from_rgb(255, 128, 192), // Pink
    egui::Color32::from_rgb(128, 255, 0),   // Lime
];

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
        if let Ok(read_guard) = thread_communication.data_lock.try_read() {
            explorer.data = read_guard.clone();
        }

        let mut signal_1: Vec<[f64; 2]> = Vec::new();
        let mut filtered_signal_1: Vec<[f64; 2]> = Vec::new();
        let mut avg_signal_1: Vec<[f64; 2]> = Vec::new();

        let axis_display_offset_signal_1 = explorer
            .data
            .signal
            .iter()
            .fold(f64::INFINITY, |ai, &bi| ai.min(bi as f64))
            .abs();

        let axis_display_offset_filtered_signal_1 = explorer
            .data
            .filtered_signal
            .iter()
            .fold(f64::INFINITY, |ai, &bi| ai.min(bi as f64))
            .abs();

        let axis_display_offset_avg_signal_1 = explorer
            .data
            .avg_signal
            .iter()
            .fold(f64::INFINITY, |ai, &bi| ai.min(bi as f64))
            .abs();

        // Create a vector of [ROI name, plot points] pairs
        let mut roi_plots = Vec::new();

        // Find minimum values for each ROI signal to calculate offsets
        let mut roi_min_values = Vec::new();
        for (_, (_, roi_data)) in &explorer.data.roi_signal {
            let min_value = roi_data
                .iter()
                .fold(f64::INFINITY, |ai, &bi| ai.min(bi as f64))
                .abs();
            roi_min_values.push(min_value);
        }

        // Get the maximum offset value from all signals including ROIs
        let axis_display_offset = [
            axis_display_offset_signal_1,
            axis_display_offset_filtered_signal_1,
            axis_display_offset_avg_signal_1,
        ]
        .iter()
        .chain(roi_min_values.iter())
        .fold(f64::NEG_INFINITY, |ai, &bi| ai.max(bi))
            * 1.05;

        // Generate plot points for each ROI
        for (_, (roi_name, roi_data)) in &explorer.data.roi_signal {
            let mut roi_plot_points = Vec::new();

            for i in 0..explorer.data.time.len().min(roi_data.len()) {
                roi_plot_points.push([
                    explorer.data.time[i] as f64,
                    roi_data[i] as f64 + axis_display_offset,
                ]);
            }

            if !roi_plot_points.is_empty() {
                roi_plots.push((roi_name.clone(), roi_plot_points));
            }
        }

        for i in 0..explorer.data.time.len() {
            signal_1.push([
                explorer.data.time[i] as f64,
                explorer.data.signal[i] as f64 + axis_display_offset,
            ]);
        }

        for i in 0..explorer
            .data
            .filtered_time
            .len()
            .min(explorer.data.filtered_signal.len())
        {
            filtered_signal_1.push([
                explorer.data.filtered_time[i] as f64,
                explorer.data.filtered_signal[i] as f64 + axis_display_offset,
            ]);
        }

        for i in 0..explorer.data.time.len().min(explorer.data.avg_signal.len()) {
            avg_signal_1.push([
                explorer.data.time[i] as f64,
                explorer.data.avg_signal[i] as f64 + axis_display_offset,
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
            .min_size(vec2(50.0, 100.0))
            .legend(Legend::default());

        signal_plot.show(ui, |signal_plot_ui| {
            signal_plot_ui.line(
                Line::new("Signal".to_string(), PlotPoints::from(signal_1))
                    .color(egui::Color32::RED)
                    .style(LineStyle::Solid)
                    .width(2.0),
            );

            signal_plot_ui.line(
                Line::new(
                    "Filtered Signal".to_string(),
                    PlotPoints::from(filtered_signal_1),
                )
                .color(egui::Color32::BLUE)
                .style(LineStyle::Solid)
                .width(2.0),
            );

            signal_plot_ui.line(
                Line::new("Avg Signal".to_string(), PlotPoints::from(avg_signal_1))
                    .color(egui::Color32::YELLOW)
                    .style(LineStyle::Solid)
                    .width(2.0),
            );

            // Plot each ROI with its own color
            for (i, (roi_name, roi_points)) in roi_plots.iter().enumerate() {
                let color_idx = i % ROI_COLORS.len();

                signal_plot_ui.line(
                    Line::new(roi_name.to_string(), PlotPoints::from(roi_points.clone()))
                        .color(ROI_COLORS[color_idx])
                        .style(LineStyle::Solid)
                        .width(2.0),
                );
            }
        });

        ui.add_space(spacing);

        // First, get the current value of min_fft_signals
        let fft_signals = [&explorer.data.signal_fft];
        let min_fft_signals = fft_signals
            .iter()
            .flat_map(|v| v.iter().copied())
            .map(|x| x)
            .fold(f32::MAX, |a, b| a.min(b));

        let floor_value = min_fft_signals / 5.0;

        let signal_1_fft: Vec<[f64; 2]> = explorer
            .data
            .frequencies
            .iter()
            .zip(explorer.data.signal_fft.iter())
            .map(|(x, y)| {
                let fft = if thread_communication.gui_settings.fft_log_plot {
                    if *y < floor_value {
                        20.0 * (floor_value).log10()
                    } else {
                        20.0 * (*y).log10()
                    }
                } else {
                    *y
                };
                [*x as f64, fft as f64]
            })
            .collect();
        let filtered_signal_1_fft: Vec<[f64; 2]> = explorer
            .data
            .filtered_frequencies
            .iter()
            .zip(explorer.data.filtered_signal_fft.iter())
            .map(|(x, y)| {
                let fft = if thread_communication.gui_settings.fft_log_plot {
                    if *y < floor_value {
                        20.0 * (floor_value).log10()
                    } else {
                        20.0 * (*y).log10()
                    }
                } else {
                    *y
                };
                [*x as f64, fft as f64]
            })
            .collect();
        let avg_signal_1_fft: Vec<[f64; 2]> = explorer
            .data
            .frequencies
            .iter()
            .zip(explorer.data.avg_signal_fft.iter())
            .map(|(x, y)| {
                let fft = if thread_communication.gui_settings.fft_log_plot {
                    if *y < floor_value {
                        20.0 * (floor_value).log10()
                    } else {
                        20.0 * (*y).log10()
                    }
                } else {
                    *y
                };
                [*x as f64, fft as f64]
            })
            .collect();
        let phase_1_fft: Vec<[f64; 2]> = explorer
            .data
            .frequencies
            .iter()
            .zip(explorer.data.phase_fft.iter())
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

        // Create a vector of [ROI name, amplitude plot points, phase plot points] tuples
        let mut roi_fft_plots = Vec::new();

        // Generate FFT plot points for each ROI
        for (roi_uuid, _) in &explorer.data.roi_signal {
            if let (Some((roi_name, roi_signal_fft)), Some((_, roi_phase_fft))) = (
                explorer.data.roi_signal_fft.get(roi_uuid),
                explorer.data.roi_phase.get(roi_uuid),
            ) {
                let roi_amplitude_plot: Vec<[f64; 2]> = explorer
                    .data
                    .frequencies
                    .iter()
                    .zip(roi_signal_fft.iter())
                    .map(|(x, y)| {
                        let fft = if thread_communication.gui_settings.fft_log_plot {
                            if *y < floor_value {
                                20.0 * (floor_value).log10()
                            } else {
                                20.0 * (*y).log10()
                            }
                        } else {
                            *y
                        };
                        [*x as f64, fft as f64]
                    })
                    .collect();

                let roi_phase_plot: Vec<[f64; 2]> = explorer
                    .data
                    .frequencies
                    .iter()
                    .zip(roi_phase_fft.iter())
                    .map(|(x, y)| [*x as f64, *y as f64])
                    .collect();

                roi_fft_plots.push((roi_name.clone(), roi_amplitude_plot, roi_phase_plot));
            }
        }

        let fft_signals = [&signal_1_fft];

        let mut max_fft_signals = fft_signals
            .iter()
            .flat_map(|v| v.iter().copied())
            .map(|x| x[1])
            .fold(f64::MIN, |a, b| a.max(b));

        if max_fft_signals < -200.0 {
            max_fft_signals = -200.0;
        }

        let log_plot = thread_communication.gui_settings.fft_log_plot;
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
            .include_x(10.0)
            .legend(Legend::default());

        if !thread_communication.gui_settings.phases_visible {
            // fft_plot = fft_plot.include_y(-100.0);
            fft_plot = fft_plot.include_y(0.0)
        };

        fft_plot.show(ui, |fft_plot_ui| {
            if !thread_communication.gui_settings.phases_visible {
                fft_plot_ui.line(
                    Line::new("Amplitude".to_string(), PlotPoints::from(signal_1_fft))
                        .color(egui::Color32::RED)
                        .style(LineStyle::Solid)
                        .width(2.0),
                );
            } else {
                fft_plot_ui.line(
                    Line::new("Phase".to_string(), PlotPoints::from(phase_1_fft))
                        .color(egui::Color32::RED)
                        .style(LineStyle::Solid)
                        .width(2.0),
                );
            }

            if !thread_communication.gui_settings.phases_visible {
                fft_plot_ui.line(
                    Line::new(
                        "Filtered Amplitude".to_string(),
                        PlotPoints::from(filtered_signal_1_fft),
                    )
                    .color(egui::Color32::BLUE)
                    .style(LineStyle::Solid)
                    .width(2.0),
                )
            } else {
                fft_plot_ui.line(
                    Line::new(
                        "Filtered Phase".to_string(),
                        PlotPoints::from(filtered_phase_1_fft),
                    )
                    .color(egui::Color32::BLUE)
                    .style(LineStyle::Solid)
                    .width(2.0),
                );
            }

            if !thread_communication.gui_settings.phases_visible {
                fft_plot_ui.line(
                    Line::new(
                        "Avg Amplitude".to_string(),
                        PlotPoints::from(avg_signal_1_fft),
                    )
                    .color(egui::Color32::YELLOW)
                    .style(LineStyle::Solid)
                    .width(2.0),
                )
            } else {
                fft_plot_ui.line(
                    Line::new("Avg Phase".to_string(), PlotPoints::from(avg_phase_1_fft))
                        .color(egui::Color32::YELLOW)
                        .style(LineStyle::Solid)
                        .width(2.0),
                );
            }

            if thread_communication.gui_settings.water_lines_visible {
                for line in explorer.water_vapour_lines.iter() {
                    fft_plot_ui.vline(
                        VLine::new("Water Vapour".to_string(), *line)
                            .stroke(Stroke::new(1.0, egui::Color32::BLUE))
                            .width(2.0),
                    );
                }
            }

            // Plot each ROI FFT with its own color
            for (i, (roi_name, roi_amplitude, roi_phase)) in roi_fft_plots.iter().enumerate() {
                let color_idx = i % ROI_COLORS.len();

                if !thread_communication.gui_settings.phases_visible {
                    fft_plot_ui.line(
                        Line::new(
                            roi_name.to_string(),
                            PlotPoints::from(roi_amplitude.clone()),
                        )
                        .color(ROI_COLORS[color_idx])
                        .style(LineStyle::Solid)
                        .width(2.0),
                    );
                } else {
                    fft_plot_ui.line(
                        Line::new(roi_name.to_string(), PlotPoints::from(roi_phase.clone()))
                            .color(ROI_COLORS[color_idx])
                            .style(LineStyle::Solid)
                            .width(2.0),
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
                thread_communication.gui_settings.fft_log_plot =
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

            ui.add_space(ui.available_size().x - 250.0 - right_panel_width);

            // dynamic range:
            let length = explorer.data.signal_fft.len();
            let dr1 = if !explorer.data.signal_fft.is_empty() {
                explorer.data.signal_fft[length - 100..length]
                    .iter()
                    .sum::<f32>()
                    / 100.0
            } else {
                0.0
            };
            ui.label(format!(
                "DR: {:.1} dB",
                20.0 * (dr1.abs() + 1e-10).log(10.0) - max_fft_signals as f32,
            ));

            ui.add_space(50.0);

            // peak to peak
            let ptp1 = if let (Some(min), Some(max)) = (
                explorer.data.signal.iter().cloned().reduce(f32::min),
                explorer.data.signal.iter().cloned().reduce(f32::max),
            ) {
                max - min
            } else {
                0.0
            };
            ui.label(format!("ptp: {:.1} nA", ptp1));
        });
    });
}

pub fn optical_properties_tab(
    ui: &mut Ui,
    height: f32,
    width: f32,
    spacing: f32,
    right_panel_width: f32,
    explorer: &mut THzImageExplorer,
    thread_communication: &mut ThreadCommunication,
) {
    if let Ok(data) = thread_communication.data_lock.try_read() {
        if !data.roi_signal.is_empty() {
            let roi_names: Vec<String> = data.roi_signal.values().map(|v| v.0.clone()).collect();
            explorer.data.available_references = roi_names.clone();
            explorer.data.available_samples = roi_names;
            explorer
                .data
                .available_samples
                .push("Selected Pixel".to_string());
        } else {
            // If no ROIs are available yet
            explorer.data.available_references = vec![];
            explorer.data.available_samples = vec!["Selected Pixel".to_string()];
        }
    }

    ui.vertical(|ui| {
        // Signal selection controls
        ui.horizontal(|ui| {
            ui.label("Reference:");
            egui::ComboBox::from_id_salt("reference_selection")
                .selected_text(thread_communication.gui_settings.selected_reference.clone())
                .width(120.0)
                .show_ui(ui, |ui| {
                    for av_ref in explorer.data.available_references.iter() {
                        if ui
                            .selectable_value(
                                &mut thread_communication.gui_settings.selected_reference,
                                av_ref.to_string(),
                                av_ref,
                            )
                            .clicked()
                        {
                            thread_communication
                                .config_tx
                                .send(ConfigCommand::SetReference(av_ref.clone()))
                                .unwrap();
                        }
                    }
                });

            ui.add_space(20.0);

            ui.label("Sample:");
            egui::ComboBox::from_id_salt("sample_selection")
                .selected_text(thread_communication.gui_settings.selected_sample.clone())
                .width(120.0)
                .show_ui(ui, |ui| {
                    for av_sample in explorer.data.available_samples.iter() {
                        if ui
                            .selectable_value(
                                &mut thread_communication.gui_settings.selected_sample,
                                av_sample.to_string(),
                                av_sample,
                            )
                            .clicked()
                        {
                            thread_communication
                                .config_tx
                                .send(ConfigCommand::SetSample(av_sample.clone()))
                                .unwrap();
                        }
                    }
                });

            ui.add_space(20.0);

            // Create a unique ID for this filter's info popup
            let popup_id = ui.make_persistent_id("info_popup_optical_properties");
            // Show info icon and handle clicks
            let info_button = ui.button(format!("{}", egui_phosphor::regular::INFO));

            Popup::menu(&info_button)
                .id(popup_id)
                .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
                .show(
                |ui: &mut egui::Ui| {
                    ui.set_max_width(300.0);
                    ui.label("Select a reference and a sample to plot the optical properties. If no options are available, please load either a reference dataset and/or create at least one region of interest (ROI) in the 2D viewer.");
                },
            );
        });

        ui.add_space(spacing / 2.0);

        if let Ok(read_guard) = thread_communication.data_lock.read() {
            explorer.data = read_guard.clone();
        }

        // Refractive index plot data
        let refractive_index: Vec<[f64; 2]> = explorer
            .data
            .frequencies
            .iter()
            .zip(explorer.data.refractive_index.iter())
            .map(|(x, y)| [*x as f64, *y as f64])
            .filter(|point| !point[1].is_infinite()) // Filter out infinite values
            .collect();

        let extinction_coefficient: Vec<[f64; 2]> = explorer
            .data
            .frequencies
            .iter()
            .zip(explorer.data.extinction_coefficient.iter())
            .map(|(x, y)| [*x as f64, *y as f64])
            .filter(|point| !point[1].is_infinite()) // Filter out infinite values
            .collect();

        let absorption: Vec<[f64; 2]> = explorer
            .data
            .frequencies
            .iter()
            .zip(explorer.data.absorption_coefficient.iter())
            .map(|(x, y)| [*x as f64, *y as f64])
            .filter(|point| !point[1].is_infinite()) // Filter out infinite values
            .collect();

        // Format functions for the plots
        let f_fmt = |x: GridMark, _range: &RangeInclusive<f64>| format!("{:4.2} THz", x.value);
        let n_fmt = |y: GridMark, _range: &RangeInclusive<f64>| format!("{:4.2}", y.value);
        let label_fmt =
            |s: &str, val: &PlotPoint| format!("{}\n{:4.2} THz\n{:4.2}", s, val.x, val.y);

        // Refractive index plot
        let n_plot = Plot::new("refractive_index")
            .height(height)
            .width(width)
            .y_axis_formatter(n_fmt)
            .x_axis_formatter(f_fmt)
            .label_formatter(label_fmt)
            .include_x(0.0)
            .include_x(explorer.data.frequencies.last().unwrap_or(&10.0) * 1.05)
            .legend(Legend::default());

        n_plot.show(ui, |plot_ui| {
            plot_ui.line(
                Line::new("Refractive Index (n)".to_string(), PlotPoints::from(refractive_index))
                    .color(egui::Color32::RED)
                    .style(LineStyle::Solid)
                    .width(2.0)
            );
        });

        ui.add_space(spacing);

        // Absorption plot
        let a_fmt = |y: GridMark, _range: &RangeInclusive<f64>| format!("{:4.2} cm⁻¹", y.value);
        let a_label_fmt =
            |s: &str, val: &PlotPoint| format!("{}\n{:4.2} THz\n{:4.2} cm⁻¹", s, val.x, val.y);

        let absorption_plot = Plot::new("absorption")
            .height(height)
            .width(width)
            .y_axis_formatter(a_fmt)
            .x_axis_formatter(f_fmt)
            .label_formatter(a_label_fmt)
            .legend(Legend::default());

        absorption_plot.show(ui, |plot_ui| {
            plot_ui.line(
                Line::new("Absorption (α)".to_string(), PlotPoints::from(absorption))
                    .color(egui::Color32::GREEN)
                    .style(LineStyle::Solid)
                    .width(2.0)
            );

            plot_ui.line(
                Line::new("Extinction Coefficient (k)".to_string(), PlotPoints::from(extinction_coefficient))
                    .color(egui::Color32::BLUE)
                    .style(LineStyle::Solid)
                    .width(2.0)
            );

            if thread_communication.gui_settings.water_lines_visible {
                for line in explorer.water_vapour_lines.iter() {
                    plot_ui.vline(
                        VLine::new("Water Vapour".to_string(), *line)
                            .stroke(Stroke::new(1.0, egui::Color32::BLUE))
                            .width(2.0)
                    );
                }
            }
        });

        // Bottom controls
        ui.add_space(5.0);
        ui.horizontal(|ui| {
            ui.label("Sample Thickness:");
            if ui
                .add(
                    // we just use 20 as a maximum value for the slider for practical purposes
                    Slider::new(
                        &mut thread_communication.gui_settings.sample_thickness,
                        0.01..=20.0,
                    )
                        .min_decimals(2)
                        .max_decimals(2)
                        .suffix(" mm"),
                )
                .changed()
            {
                thread_communication
                    .config_tx
                    .send(ConfigCommand::SetMaterialThickness(
                        thread_communication.gui_settings.sample_thickness,
                    ))
                    .unwrap();
            }

            ui.add_space(10.0);
            ui.add(Checkbox::new(
                &mut thread_communication.gui_settings.water_lines_visible,
                "",
            ));
            ui.colored_label(egui::Color32::BLUE, "— ");
            ui.label("Water Lines");

            ui.add_space(ui.available_size().x - 400.0 - right_panel_width);

            // Display material statistics if available
            if let Some(max_n) = explorer
                .data
                .refractive_index
                .iter()
                .cloned()
                .reduce(f32::max)
            {
                ui.label(format!("Max n: {:.2}", max_n));
            }

            ui.add_space(30.0);

            if let Some(max_alpha) = explorer
                .data
                .absorption_coefficient
                .iter()
                .cloned()
                .reduce(f32::max)
            {
                ui.label(format!("Max α: {:.2} cm⁻¹", max_alpha));
            }
        });
        ui.add_space(5.0);
    });
}

#[allow(clippy::too_many_arguments)]
pub fn center_panel(
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
        let height = ui.available_size().y * 0.45;
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
                .selectable_label(tabs[1], Tab::OpticalProperties.to_string())
                .clicked()
            {
                thread_communication.gui_settings.tab = Tab::OpticalProperties;
                thread_communication
                    .config_tx
                    .send(ConfigCommand::UpdateMaterialCalculation)
                    .unwrap();
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
                Tab::OpticalProperties => optical_properties_tab(
                    ui,
                    height * 0.95,
                    width,
                    spacing,
                    *right_panel_width,
                    explorer,
                    thread_communication,
                ),
                Tab::ThreeD => {
                    three_dimensional_plot_ui(
                        cube_preview_texture_id,
                        width,
                        window_height,
                        ui,
                        opacity_threshold,
                        cam_input,
                        thread_communication,
                        explorer,
                    );
                } // TODO add BRDF / sweep for .thzswp files
            }
        });
    });
}
