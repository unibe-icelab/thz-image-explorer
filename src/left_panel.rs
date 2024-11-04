use std::sync::mpsc::Sender;
use std::sync::{Arc, RwLock};

use eframe::egui;
use eframe::egui::panel::Side;
use eframe::egui::{vec2, Vec2};
use egui_plot::PlotPoint;
use ndarray::Array2;

use crate::config::Config;
use crate::gauge::gauge;
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
                if let Some(path) = rfd::FileDialog::new().pick_folder() {
                    config_tx
                        .send(Config::OpenFile(path.clone()))
                        .expect("unable to send open file cmd");
                }
            };

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
