use crate::config::{ConfigCommand, GuiThreadCommunication};
use crate::data_container::DataPoint;
use crate::gui::application::Tab;
use crate::gui::toggle_widget::toggle;
use crate::vec2;
use eframe::egui;
use eframe::egui::{Checkbox, DragValue, Stroke, Ui, Vec2};
use egui_plot::{GridMark, Line, LineStyle, Plot, PlotPoint, PlotPoints, VLine};
use egui_plotter::EguiBackend;
use ndarray::Array2;
use plotters::prelude::*;
use std::ops::RangeInclusive;
const MOVE_SCALE: f32 = 0.01;
const SCROLL_SCALE: f32 = 0.001;

pub fn pulse_tab(
    ui: &mut Ui,
    height: f32,
    width: f32,
    spacing: f32,
    right_panel_width: f32,
    thread_communication: &mut GuiThreadCommunication,
    data: &mut DataPoint,
    water_vapour_lines: &[f64],
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
            *data = read_guard.clone();
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
            axis_display_offset_signal_1 = data
                .signal_1
                .iter()
                .fold(f64::INFINITY, |ai, &bi| ai.min(bi as f64))
                .abs();
        }
        if thread_communication.gui_settings.filtered_signal_1_visible {
            axis_display_offset_filtered_signal_1 = data
                .filtered_signal_1
                .iter()
                .fold(f64::INFINITY, |ai, &bi| ai.min(bi as f64))
                .abs();
        }
        if thread_communication.gui_settings.avg_signal_1_visible {
            axis_display_offset_avg_signal_1 = data
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

        for i in 0..data.time.len().min(data.avg_signal_1.len()) {
            avg_signal_1.push([
                data.time[i] as f64,
                data.avg_signal_1[i] as f64 + axis_display_offset,
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

        let signal_1_fft: Vec<[f64; 2]> = data
            .frequencies
            .iter()
            .zip(data.signal_1_fft.iter())
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
        let filtered_signal_1_fft: Vec<[f64; 2]> = data
            .frequencies
            .iter()
            .zip(data.filtered_signal_1_fft.iter())
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
        let avg_signal_1_fft: Vec<[f64; 2]> = data
            .frequencies
            .iter()
            .zip(data.avg_signal_1_fft.iter())
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
        let avg_phase_1_fft: Vec<[f64; 2]> = data
            .frequencies
            .iter()
            .zip(data.avg_phase_fft.iter())
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
                for line in water_vapour_lines.iter() {
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
                if thread_communication.gui_settings.frequency_resolution_temp > 1.0 / data.hk.range
                {
                    thread_communication.gui_settings.frequency_resolution_temp =
                        1.0 / data.hk.range;
                } else if thread_communication.gui_settings.frequency_resolution_temp < 0.0001 {
                    thread_communication.gui_settings.frequency_resolution_temp = 0.0001;
                }
                thread_communication.gui_settings.frequency_resolution =
                    thread_communication.gui_settings.frequency_resolution_temp;
                thread_communication
                    .config_tx
                    .send(ConfigCommand::SetFFTResolution(
                        thread_communication.gui_settings.frequency_resolution,
                    ))
                    .expect("unable to send config");
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
            let length = data.signal_1_fft.len();
            let dr1 = if !data.signal_1_fft.is_empty() {
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
}

pub fn refractive_index_tab(
    ui: &mut Ui,
    height: f32,
    width: f32,
    spacing: f32,
    right_panel_width: f32,
    thread_communication: &mut GuiThreadCommunication,
    data: &mut DataPoint,
    water_vapour_lines: &[f64],
) {
}

fn three_dimensional_plot(
    ui: &mut Ui,
    thread_communication: &mut GuiThreadCommunication,
    right_panel_width: f32,
    height: f32,
) {
    let size = Vec2::new(ui.available_width() - right_panel_width, height);
    ui.allocate_ui(size, |ui| {
        // First, get mouse data
        let (pitch_delta, yaw_delta, scale_delta) = ui.input(|input| {
            let pointer = &input.pointer;
            let delta = pointer.delta();

            let (pitch_delta, yaw_delta) = match pointer.primary_down() {
                true => (delta.y * MOVE_SCALE, -delta.x * MOVE_SCALE),
                false => (
                    thread_communication.gui_settings.chart_pitch_vel,
                    thread_communication.gui_settings.chart_yaw_vel,
                ),
            };

            let scale_delta = input.smooth_scroll_delta.y * SCROLL_SCALE;

            (pitch_delta, yaw_delta, scale_delta)
        });

        thread_communication.gui_settings.chart_pitch_vel = pitch_delta;
        thread_communication.gui_settings.chart_yaw_vel = yaw_delta;

        thread_communication.gui_settings.chart_pitch +=
            thread_communication.gui_settings.chart_pitch_vel;
        thread_communication.gui_settings.chart_yaw +=
            thread_communication.gui_settings.chart_yaw_vel;
        thread_communication.gui_settings.chart_scale += scale_delta;

        // Next plot everything
        let root = EguiBackend::new(ui).into_drawing_area();

        root.fill(&WHITE).unwrap();

        let mut width = 10;
        let mut height = 10;
        let mut image = Array2::zeros((width, height));
        if let Ok(img) = thread_communication.img_lock.read() {
            let shape = img.shape();
            width = shape[0];
            height = shape[1];
            image = img.clone().into();
        }

        // Ensure equal axis scaling
        let max_dim = width.max(height);

        let mut chart = ChartBuilder::on(&root)
            .caption("3D Plot", (FontFamily::SansSerif, 20))
            .build_cartesian_3d(
                -(max_dim as f32 / 2.0)..max_dim as f32 / 2.0,
                -(max_dim as f32 / 2.0)..max_dim as f32 / 2.0,
                -(max_dim as f32 / 2.0)..max_dim as f32 / 2.0,
            )
            .unwrap();

        chart.with_projection(|mut pb| {
            pb.yaw = thread_communication.gui_settings.chart_yaw as f64;
            pb.pitch = thread_communication.gui_settings.chart_pitch as f64;
            pb.scale = thread_communication.gui_settings.chart_scale as f64;
            pb.into_matrix()
        });

        chart
            .configure_axes()
            .light_grid_style(BLACK.mix(0.15))
            .max_light_lines(3)
            .draw()
            .unwrap();

        // Compute min and max values for normalization
        let (min_val, max_val) = image
            .iter()
            .cloned()
            .fold((f32::INFINITY, f32::NEG_INFINITY), |(min, max), v| {
                (min.min(v), max.max(v))
            });
        let range = max_val - min_val;

        let surface_series = SurfaceSeries::xoy(
            (-((image.shape()[0] as isize) / 2)..(image.shape()[0] as isize / 2)).map(|x| x as f32),
            (-((image.shape()[1] as isize) / 2)..(image.shape()[1] as isize / 2)).map(|y| y as f32),
            |x, y| {
                let xi = ((x + width as f32 / 2.0).round() as usize).clamp(0, image.shape()[0] - 1);
                let yi =
                    ((y + height as f32 / 2.0).round() as usize).clamp(0, image.shape()[1] - 1);
                let z = image[[xi, yi]];
                // Normalize z to [0, 1] range
                if range > 0.0 {
                    (z - min_val) / range
                } else {
                    0.5 // Avoid division by zero, default mid-gray color
                }
            },
        )
        .style_func(&|z| {
            let normalized = ((z + 1.0) / 2.0).clamp(0.0, 1.0); // Normalize between 0 and 1
            HSLColor(normalized as f64, 1.0, 0.5).filled() // Color mapping
        });

        // 🔹 Draw cubes at fixed Z-height with heatmap colors

        let height_level = max_dim as f32 / 4.0; // Fixed Z-level for cubes

        chart
            .draw_series(
                (0..image.shape()[0])
                    .flat_map(|x| (0..image.shape()[1]).map(move |y| (x, y)))
                    .map(|(x, y)| {
                        let z = image[[x, y]];
                        let normalized_z = if range > 0.0 {
                            (z - min_val) / range
                        } else {
                            0.5
                        };

                        let color = heatmap_color(normalized_z);

                        Cubiod::new(
                            [
                                (
                                    x as f32 - width as f32 / 2.0,
                                    height_level,
                                    y as f32 - height as f32 / 2.0,
                                ),
                                (
                                    x as f32 - width as f32 / 2.0 + 1.0,
                                    height_level + 0.5,
                                    y as f32 - height as f32 / 2.0 + 1.0,
                                ),
                            ],
                            color.filled(),
                            &TRANSPARENT,
                        )
                    }),
            )
            .unwrap();

        chart.draw_series(surface_series).unwrap();

        // chart
        //     .draw_series(
        //         SurfaceSeries::xoz(
        //             (-30..30).map(|f| f as f64 / 10.0),
        //             (-30..30).map(|f| f as f64 / 10.0),
        //             |x, z| (x * x + z * z).cos(),
        //         )
        //             .style(BLUE.mix(0.2).filled()),
        //     )
        //     .unwrap()
        //     .label("Surface")
        //     .legend(|(x, y)| Rectangle::new([(x + 5, y - 5), (x + 15, y + 5)], BLUE.mix(0.5).filled()));

        // chart
        //     .draw_series(LineSeries::new(
        //         (-100..100)
        //             .map(|y| y as f64 / 40.0)
        //             .map(|y| ((y * 10.0).sin(), y, (y * 10.0).cos())),
        //         &BLACK,
        //     ))
        //     .unwrap()
        //     .label("Line")
        //     .legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], BLACK));

        chart
            .configure_series_labels()
            .border_style(BLACK)
            .draw()
            .unwrap();

        root.present().unwrap();
    });
}

#[allow(clippy::too_many_arguments)]
pub fn center_panel(
    ctx: &egui::Context,
    right_panel_width: &f32,
    left_panel_width: &f32,
    thread_communication: &mut GuiThreadCommunication,
    data: &mut DataPoint,
    water_vapour_lines: &[f64],
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
                    thread_communication,
                    data,
                    water_vapour_lines,
                ),
                Tab::RefractiveIndex => refractive_index_tab(
                    ui,
                    height,
                    width,
                    spacing,
                    *right_panel_width,
                    thread_communication,
                    data,
                    water_vapour_lines,
                ),
                Tab::ThreeD => {
                    three_dimensional_plot(
                        ui,
                        thread_communication,
                        *right_panel_width,
                        window_height,
                    );
                }
            }
        });
    });
}

#[allow(dead_code)]
/// **Map Heatmap Values to RGB Colors**
fn heatmap_color(value: f32) -> RGBColor {
    let normalized = ((value + 1.0) / 2.0).clamp(0.0, 1.0); // Normalize between 0 and 1
    let r = (255.0 * normalized) as u8;
    let g = (255.0 * (1.0 - normalized)) as u8;
    let b = 150;
    RGBColor(r, g, b)
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
