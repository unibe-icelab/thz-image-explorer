use dotthz::DotthzMetaData;
use eframe::egui;
use eframe::egui::panel::Side;
use eframe::egui::{vec2, Vec2};
use egui_file_dialog::information_panel::InformationPanel;
use egui_file_dialog::FileDialog;
use egui_plot::PlotPoint;
use ndarray::Array2;
use std::sync::mpsc::Sender;
use std::sync::{Arc, RwLock};

use crate::config::Config;
use crate::gauge::gauge;
use crate::gui::FileDialogState;
use crate::matrix_plot::{make_dummy, plot_matrix, SelectedPixel};
use crate::toggle::toggle_ui;
use crate::{DataPoint, GuiSettingsContainer};

#[allow(clippy::too_many_arguments)]
pub fn left_panel(
    ctx: &egui::Context,
    left_panel_width: &f32,
    gui_conf: &mut GuiSettingsContainer,
    coconut_light: egui::Image,
    coconut_dark: egui::Image,
    pixel_selected: &mut SelectedPixel,
    val: &mut PlotPoint,
    mid_point: &mut f32,
    bw: &mut bool,
    file_dialog_state: &mut FileDialogState,
    file_dialog: &mut FileDialog,
    information_panel: &mut InformationPanel,
    md_lock: &Arc<RwLock<DotthzMetaData>>,
    img_lock: &Arc<RwLock<Array2<f32>>>,
    data_lock: &Arc<RwLock<DataPoint>>,
    pixel_lock: &Arc<RwLock<SelectedPixel>>,
    config_tx: &Sender<Config>,
) {
    let gauge_size = left_panel_width / 3.0;
    let mut data = DataPoint::default();
    if let Ok(read_guard) = data_lock.read() {
        data = read_guard.clone();
    }

    egui::SidePanel::new(Side::Left, "Left Panel Settings")
        .min_width(*left_panel_width)
        .max_width(*left_panel_width)
        .resizable(false)
        .show(ctx, |ui| {
            ui.add_enabled_ui(true, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("Housekeeping");
                });
                ui.separator();

                ui.horizontal(|ui| {
                    ui.add_space((left_panel_width - 2.0 * gauge_size) / 3.0);
                    ui.add(gauge(
                        &data.hk.ambient_temperature,
                        -273.15,
                        100.0,
                        gauge_size as f64,
                        "°C",
                        "T_A",
                    ));
                    ui.add_space((left_panel_width - 2.0 * gauge_size) / 3.0);
                    ui.add(gauge(
                        &data.hk.sample_temperature,
                        -273.15,
                        100.0,
                        gauge_size as f64,
                        "°C",
                        "T_S",
                    ));
                });
                ui.horizontal(|ui| {
                    ui.add_space((left_panel_width - 2.0 * gauge_size) / 3.0);
                    ui.add(gauge(
                        &data.hk.ambient_humidity,
                        0.0,
                        100.0,
                        gauge_size as f64,
                        "%",
                        "RH",
                    ));
                    ui.add_space((left_panel_width - 2.0 * gauge_size) / 3.0);
                    ui.add(gauge(
                        &data.hk.ambient_pressure,
                        900.0,
                        1100.0,
                        gauge_size as f64,
                        "hpa",
                        "p0",
                    ));
                });
            });

            if ui
                .button(egui::RichText::new(format!(
                    "{} Load Scan",
                    egui_phosphor::regular::FOLDER_OPEN
                )))
                .clicked()
            {
                *file_dialog_state = FileDialogState::Open;
                file_dialog.pick_file();
            };

            file_dialog.set_right_panel_width(300.0);

            match file_dialog_state {
                FileDialogState::Open => {
                    if let Some(path) = file_dialog
                        .update_with_right_panel_ui(ctx, &mut |ui, dia| {
                            information_panel.ui(ui, dia);
                        })
                        .picked()
                    {
                        *file_dialog_state = FileDialogState::None;
                        config_tx
                            .send(Config::OpenFile(path.to_path_buf()))
                            .expect("unable to send open file cmd");
                    }
                }
                FileDialogState::Save => {
                    if let Some(path) = file_dialog.update(ctx).picked() {
                        *file_dialog_state = FileDialogState::None;
                        // match tera_flash_conf.filetype {
                        //     FileType::Csv => {
                        //         picked_path.set_extension("csv");
                        //     }
                        //     FileType::Binary => {
                        //         picked_path.set_extension("npy");
                        //     }
                        //     FileType::DotTHz => {
                        //         picked_path.set_extension("thz");
                        //     }
                        // }
                        // if let Err(e) = save_tx.send(picked_path.clone()) {
                        //
                        // }
                    }
                }
                FileDialogState::None => {}
            }

            let logo_height = 100.0;
            let height = ui.available_size().y - logo_height - 20.0;

            let mut img_data = make_dummy();
            if let Ok(read_guard) = img_lock.read() {
                img_data = read_guard.clone();
            }
            let pixel_clicked = plot_matrix(
                ui,
                &img_data,
                &(*left_panel_width as f64),
                &(height as f64),
                &mut 0.0,
                val,
                pixel_selected,
                mid_point,
                bw,
            );
            if pixel_clicked {
                config_tx
                    .send(Config::SetSelectedPixel(pixel_selected.clone()))
                    .unwrap();
                if let Ok(mut write_guard) = pixel_lock.write() {
                    *write_guard = pixel_selected.clone();
                }
            }

            ui.add_space(10.0);
            ui.label("Black/White");
            toggle_ui(ui, bw);
            ui.label(format!("Pixel: {}", pixel_selected.id));
            ui.label(format!("x: {}", pixel_selected.x));
            ui.label(format!("y: {}", pixel_selected.y));

            let mut meta_data = DotthzMetaData::default();
            if let Ok(md) = md_lock.read() {
                meta_data = md.clone();
            }
            ui.label("Meta Data:");
            egui::ScrollArea::vertical()
                .max_height(200.0)
                .show(ui, |ui| {
                    egui::Grid::new("meta_data")
                        .num_columns(2)
                        .striped(true)
                        .show(ui, |ui| {
                            for (name, value) in meta_data.md {
                                ui.label(name);
                                ui.label(value);
                                ui.end_row()
                            }
                            ui.label("User:");
                            ui.label(meta_data.user);
                            ui.end_row();
                            ui.label("E-mail:");
                            ui.label(meta_data.email);
                            ui.end_row();
                            ui.label("ORCID:");
                            ui.label(meta_data.orcid);
                            ui.end_row();
                            ui.label("Institution:");
                            ui.label(meta_data.institution);
                            ui.end_row();
                            ui.label("Instrument:");
                            ui.label(meta_data.instrument);
                            ui.end_row();
                            ui.label("Version:");
                            ui.label(meta_data.version);
                            ui.end_row();
                            ui.label("Mode:");
                            ui.label(meta_data.mode);
                            ui.end_row();
                            ui.label("Date:");
                            ui.label(meta_data.date);
                            ui.end_row();
                            ui.label("Time:");
                            ui.label(meta_data.time);
                            ui.end_row();
                        });
                });

            let height = ui.available_size().y - logo_height - 20.0;
            ui.add_space(height);
            if gui_conf.dark_mode {
                let size = coconut_dark.size().unwrap_or(Vec2 { x: 200.0, y: 100.0 });
                ui.add(
                    coconut_dark
                        .fit_to_exact_size(vec2(size.y / logo_height * size.x, logo_height)),
                );
            } else {
                let size = coconut_light.size().unwrap_or(Vec2 { x: 200.0, y: 100.0 });
                ui.add(
                    coconut_light
                        .fit_to_exact_size(vec2(size.y / logo_height * size.x, logo_height)),
                );
            }
        });
}
