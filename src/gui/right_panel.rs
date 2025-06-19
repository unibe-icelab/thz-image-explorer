use crate::config::{ConfigCommand, ThreadCommunication};
use crate::filters::filter::{draw_filters, FilterDomain};
use crate::gui::application::THzImageExplorer;
use crate::gui::settings_window::settings_window;
use crate::gui::toggle_widget::toggle;
use crate::math_tools::{
    apply_adapted_blackman_window, apply_blackman, apply_flat_top, apply_hamming, apply_hanning,
    FftWindowType,
};
use crate::update::check_update;
use crate::DataPoint;
use bevy_egui::egui;
use bevy_egui::egui::panel::Side;
use bevy_egui::egui::{vec2, DragValue, Stroke, Vec2, Visuals};
use egui_double_slider::DoubleSlider;
use egui_plot::{Line, LineStyle, Plot, PlotPoints, VLine};
use ndarray::Array1;

#[allow(clippy::too_many_arguments)]
pub fn right_panel(
    ctx: &egui::Context,
    explorer: &mut THzImageExplorer,
    right_panel_width: &f32,
    thread_communication: &mut ThreadCommunication,
) {
    let mut data = DataPoint::default();
    if let Ok(read_guard) = thread_communication.data_lock.read() {
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
                        if ui
                            .add(toggle(&mut thread_communication.gui_settings.log_plot))
                            .changed()
                        {
                            thread_communication
                                .config_tx
                                .send(ConfigCommand::SetFFTLogPlot(
                                    thread_communication.gui_settings.log_plot,
                                ))
                                .expect("unable to send config");
                        }
                        ui.end_row();

                        ui.label("Normalize FFT: ");
                        if ui
                            .add(toggle(&mut thread_communication.gui_settings.normalize_fft))
                            .changed()
                        {
                            thread_communication
                                .config_tx
                                .send(ConfigCommand::SetFFTNormalization(
                                    thread_communication.gui_settings.normalize_fft,
                                ))
                                .expect("unable to send config");
                        }

                        ui.end_row();
                        ui.label("Down scaling:");

                        ui.style_mut().spacing.slider_width = 320.0;

                        ui.vertical(|ui| {
                            if !thread_communication.gui_settings.filter_ui_active {
                                ui.disable();
                            }
                            if ui
                                .add(egui::Slider::new(
                                    &mut thread_communication.gui_settings.down_scaling,
                                    1..=10,
                                ))
                                .changed()
                            {
                                explorer.pixel_selected.rect = vec![
                                    [
                                        (explorer.pixel_selected.x as f64)
                                            / thread_communication.gui_settings.down_scaling as f64,
                                        (explorer.pixel_selected.y as f64)
                                            / thread_communication.gui_settings.down_scaling as f64,
                                    ],
                                    [
                                        (explorer.pixel_selected.x as f64)
                                            / thread_communication.gui_settings.down_scaling as f64
                                            + 1.0,
                                        (explorer.pixel_selected.y as f64)
                                            / thread_communication.gui_settings.down_scaling as f64,
                                    ],
                                    [
                                        (explorer.pixel_selected.x as f64)
                                            / thread_communication.gui_settings.down_scaling as f64
                                            + 1.0,
                                        (explorer.pixel_selected.y as f64)
                                            / thread_communication.gui_settings.down_scaling as f64
                                            + 1.0,
                                    ],
                                    [
                                        (explorer.pixel_selected.x as f64)
                                            / thread_communication.gui_settings.down_scaling as f64,
                                        (explorer.pixel_selected.y as f64)
                                            / thread_communication.gui_settings.down_scaling as f64
                                            + 1.0,
                                    ],
                                    [
                                        (explorer.pixel_selected.x as f64)
                                            / thread_communication.gui_settings.down_scaling as f64,
                                        (explorer.pixel_selected.y as f64)
                                            / thread_communication.gui_settings.down_scaling as f64,
                                    ],
                                ];
                                if let Ok(mut s) = thread_communication.scaling_lock.write() {
                                    *s = thread_communication.gui_settings.down_scaling as u8;
                                }
                                if let Ok(mut write_guard) = thread_communication.pixel_lock.write()
                                {
                                    *write_guard = explorer.pixel_selected.clone();
                                }
                                thread_communication
                                    .config_tx
                                    .send(ConfigCommand::SetDownScaling)
                                    .expect("unable to send config");
                            }
                        });
                    });

                ui.add_space(10.0);
                if ui.button("Calculate All Filters").clicked() {
                    thread_communication
                        .config_tx
                        .send(ConfigCommand::UpdateFilters)
                        .expect("unable to send config");
                }
                ui.add_space(10.0);
                ui.separator();

                egui::ScrollArea::vertical().max_height(ui.available_height() - 200.0).show(ui, |ui| {

                    if !thread_communication.gui_settings.filter_ui_active {
                        ui.disable();
                    }

                    // todo: fix this with right_panel_width or similar
                    ui.style_mut().spacing.slider_width = 320.0;

                    // ui.separator();
                    // ui.heading("Time Domain Filter: ");
                    //
                    // ui.add_space(10.0);

                    draw_filters(ui, thread_communication, FilterDomain::TimeBeforeFFTPrioFirst, *right_panel_width);
                    draw_filters(ui, thread_communication, FilterDomain::TimeBeforeFFT, *right_panel_width);

                    ui.add_space(10.0);

                    ui.separator();
                    ui.separator();
                    ui.add_space(10.0);
                    ui.vertical_centered(|ui| {
                        ui.heading("---------- FFT ----------");
                    });

                    egui::CollapsingHeader::new("FFT Settings").show_background(true).default_open(false).show_unindented(ui, |ui| {
                        ui.vertical(|ui| {
                            if !thread_communication.gui_settings.filter_ui_active {
                                ui.disable();
                            }
                            if data.time.is_empty() {
                                data.time = (0..=((1050.0 - 1000.0) / 0.25) as usize)
                                    .map(|i| 1000.0 + i as f32 * 0.25)
                                    .collect();
                                data.signal_1 = vec![1.0; data.time.len()];
                            }

                            let mut window_vals: Vec<[f64; 2]> = Vec::new();
                            let mut p = Array1::from_vec(vec![1.0; data.time.len()]);
                            let t: Array1<f32> = data.time.clone().into();

                            ui.add_space(5.0);

                            let fft_window_type_old = explorer.fft_window_type.clone();

                            egui::ComboBox::from_id_salt("Window Type")
                                .selected_text(explorer.fft_window_type.to_string())
                                .width(80.0)
                                .show_ui(ui, |ui| {
                                    [
                                        FftWindowType::AdaptedBlackman,
                                        FftWindowType::Blackman,
                                        FftWindowType::Hanning,
                                        FftWindowType::Hamming,
                                        FftWindowType::FlatTop,
                                    ]
                                        .iter()
                                        .for_each(|window_type| {
                                            ui.selectable_value(
                                                &mut explorer.fft_window_type,
                                                *window_type,
                                                window_type.to_string(),
                                            );
                                        });
                                });
                            if fft_window_type_old != explorer.fft_window_type {
                                println!("changing type");
                                thread_communication
                                    .config_tx
                                    .send(ConfigCommand::SetFftWindowType(
                                        explorer.fft_window_type.clone(),
                                    ))
                                    .unwrap();
                            }

                            ui.add_space(5.0);

                            match explorer.fft_window_type {
                                FftWindowType::AdaptedBlackman => {
                                    apply_adapted_blackman_window(
                                        &mut p.view_mut(),
                                        &t,
                                        &explorer.fft_bounds[0],
                                        &explorer.fft_bounds[1],
                                    );
                                }
                                FftWindowType::Blackman => apply_blackman(&mut p.view_mut(), &t),
                                FftWindowType::Hanning => apply_hanning(&mut p.view_mut(), &t),
                                FftWindowType::Hamming => apply_hamming(&mut p.view_mut(), &t),
                                FftWindowType::FlatTop => apply_flat_top(&mut p.view_mut(), &t),
                            }

                            for i in 0..t.len() {
                                window_vals.push([t[i] as f64, p[i] as f64]);
                            }
                            let fft_window_plot = Plot::new("FFT Window")
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
                                            .name("Window"),
                                    );
                                    window_plot_ui.vline(
                                        VLine::new(
                                            data.time.first().unwrap_or(&1000.0) + explorer.fft_bounds[0],
                                        )
                                            .stroke(Stroke::new(1.0, egui::Color32::GRAY))
                                            .name("Lower Bound"),
                                    );
                                    window_plot_ui.vline(
                                        VLine::new(
                                            data.time.last().unwrap_or(&1050.0) - explorer.fft_bounds[1],
                                        )
                                            .stroke(Stroke::new(1.0, egui::Color32::GRAY))
                                            .name("Upper Bound"),
                                    );
                                });
                            });

                            let range =
                                data.time.last().unwrap_or(&1050.0) - data.time.first().unwrap_or(&1000.0);

                            let slider_changed = ui.horizontal(|ui| {
                                let right_offset = 0.09 * right_panel_width;
                                let left_offset = 0.01 * right_panel_width;
                                ui.add_space(left_offset);
                                // Display slider, linked to the same range as the plot
                                let mut fft_lower_bound = explorer.fft_bounds[0];
                                let mut fft_upper_bound = range - explorer.fft_bounds[1];

                                let slider = ui
                                    .add(
                                        DoubleSlider::new(
                                            &mut fft_lower_bound,
                                            &mut fft_upper_bound,
                                            0.0..=range,
                                        )
                                            .vertical_scroll(false)
                                            .zoom_factor(2.0)
                                            .scroll_factor(0.005)
                                            .separation_distance(2.0)
                                            .invert_highlighting(true)
                                            .width(right_panel_width - left_offset - right_offset),
                                    )
                                    .on_hover_text(egui::RichText::new(format!(
                                        "{} Scroll and Zoom to adjust the sliders. Double Click to reset.",
                                        egui_phosphor::regular::INFO
                                    )));
                                let slider_changed = slider.changed();
                                if slider.double_clicked() {
                                    fft_lower_bound = 1.0;
                                    fft_upper_bound = range - 7.0;
                                }
                                explorer.fft_bounds = [fft_lower_bound, range - fft_upper_bound];
                                slider_changed
                            });

                            ui.horizontal(|ui| {
                                let val1_changed = ui
                                    .add(DragValue::new(&mut explorer.fft_bounds[0]))
                                    .changed();

                                ui.add_space(0.75 * right_panel_width);

                                let val2_changed = ui
                                    .add(DragValue::new(&mut explorer.fft_bounds[1]))
                                    .changed();

                                if slider_changed.inner || val1_changed || val2_changed {
                                    thread_communication
                                        .config_tx
                                        .send(ConfigCommand::SetFFTWindowLow(explorer.fft_bounds[0]))
                                        .unwrap();
                                    thread_communication
                                        .config_tx
                                        .send(ConfigCommand::SetFFTWindowHigh(explorer.fft_bounds[1]))
                                        .unwrap();
                                }
                            });
                        });
                    });
                    ui.separator();
                    // ui.add_space(10.0);
                    //
                    // ui.heading("Frequency Domain Filter:");
                    // ui.add_space(10.0);

                    // draw time domain filter after FFT
                    draw_filters(ui, thread_communication, FilterDomain::Frequency, *right_panel_width);

                    ui.add_space(10.0);

                    ui.separator();
                    ui.separator();
                    ui.add_space(10.0);
                    ui.vertical_centered(|ui| {
                        ui.heading("---------- iFFT ----------");
                    });
                    ui.separator();

                    // ui.add_space(10.0);
                    //
                    // ui.separator();
                    // ui.heading("Time Domain Filter: ");

                    // draw time domain filter after FFT
                    draw_filters(ui, thread_communication, FilterDomain::TimeAfterFFT, *right_panel_width);
                    draw_filters(ui, thread_communication, FilterDomain::TimeAfterFFTPrioLast, *right_panel_width);
                });
                ui.separator();
                ui.add_space(20.0);

                thread_communication.gui_settings.dark_mode = ui.visuals() == &Visuals::dark();
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

                ui.add_space(20.0);

                if ui
                    .button(format!("{} Settings", egui_phosphor::regular::GEAR_FINE))
                    .clicked()
                {
                    #[cfg(feature = "self_update")]
                    {
                        explorer.new_release = check_update();
                    }
                    explorer.settings_window_open = true;
                }
                if explorer.settings_window_open {
                    settings_window(ui.ctx(), explorer, thread_communication);
                }

                ui.add_space(5.0);
                ui.separator();

                let height = ui.available_size().y - 38.0 - 20.0;
                ui.add_space(height);
                ui.centered_and_justified(|ui| {
                    ui.add(
                        egui::Image::from_bytes("WP", explorer.wp)
                            .fit_to_exact_size(vec2(80.0, 38.0)),
                    );
                });
            });
        });
}
