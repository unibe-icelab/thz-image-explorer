use std::sync::{Arc, RwLock};
use std::sync::mpsc::{Sender};
use eframe::egui;
use eframe::egui::panel::Side;
use eframe::egui::{Color32, ColorImage, Spinner};
use eframe::egui::plot::PlotPoint;
use egui_extras::RetainedImage;
use ndarray::Array2;
use crate::{DataContainer, GuiSettingsContainer, Print, ScannedImage};
use crate::gauge::gauge;
use crate::gui::SelectedPixel;
use crate::matrix_plot::{make_dummy, plot_matrix, plot_waterfall};
use serde::{Deserialize, Serialize};
use crate::toggle::toggle;

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub enum MODE {
    TimeSeries,
    Scan,
    Debug,
}

pub fn left_panel(ctx: &egui::Context,
                  left_panel_width: &f32,
                  picked_path: &mut String,
                  gui_conf: &mut GuiSettingsContainer,
                  coconut_light: &RetainedImage,
                  coconut_dark: &RetainedImage,
                  pixel_selected: &mut SelectedPixel,
                  val: &mut PlotPoint,
                  img_lock: &Arc<RwLock<Array2<f64>>>,
                  waterfall_lock: &Arc<RwLock<Array2<f64>>>,
                  data_lock: &Arc<RwLock<DataContainer>>,
                  print_lock: &Arc<RwLock<Vec<Print>>>,
                  pixel_lock: &Arc<RwLock<SelectedPixel>>,
                  save_tx: &Sender<String>,
                  load_tx: &Sender<String>,
) {
    let gauge_size = left_panel_width / 2.5;
    let mut data = DataContainer::default();
    if let Ok( read_guard) = data_lock.read() {
        data = read_guard.clone();
    }

    egui::SidePanel::new(Side::Left, 4)
        .min_width(*left_panel_width)
        .max_width(*left_panel_width)
        .resizable(false)
        .show(ctx, |ui| {
            ui.add_enabled_ui(true, |ui| {
                ui.set_visible(true);
                ui.horizontal(|ui| {
                    ui.heading("Housekeeping");
                });
                ui.separator();

                ui.horizontal(|ui| {
                    ui.add_space((left_panel_width - 2.0 * gauge_size) / 3.0);
                    ui.add(gauge(&data.hk.ambient_temperature, -273.15, 100.0, gauge_size as f64, "°C", "T_A"));
                    ui.add_space((left_panel_width - 2.0 * gauge_size) / 3.0);
                    ui.add(gauge(&data.hk.sample_temperature, -273.15, 100.0, gauge_size as f64, "°C", "T_S"));
                });
                ui.horizontal(|ui| {
                    ui.add_space((left_panel_width - 2.0 * gauge_size) / 3.0);
                    ui.add(gauge(&data.hk.ambient_humidity, 0.0, 100.0, gauge_size as f64, "%", "RH"));
                    ui.add_space((left_panel_width - 2.0 * gauge_size) / 3.0);
                    ui.add(gauge(&data.hk.ambient_pressure, 900.0, 1100.0, gauge_size as f64, "hpa", "p0"));
                });
            });

            if ui.button("Load Scan").clicked() {
                match rfd::FileDialog::new().pick_folder() {
                    Some(path) =>
                        {
                            *picked_path = path.display().to_string();
                            load_tx.send(picked_path.clone());
                        }
                    None => {}
                }
            };


            let logo_height = coconut_dark.height() as f32 / coconut_dark.width() as f32 * left_panel_width;
            let height = ui.available_size().y - logo_height - 20.0;


            let mut img_data = make_dummy();
            let mut waterfall_data = make_dummy();
            if let Ok(read_guard) = img_lock.read() {
                img_data = read_guard.clone();
            }
            if let Ok(read_guard) = waterfall_lock.read() {
                waterfall_data = read_guard.clone();
            }
            let img = plot_matrix(
                ui,
                &img_data,
                &(*left_panel_width as f64),
                &(height as f64),
                &mut data.cut_off,
                val,
                pixel_selected,
            );
            if pixel_selected.selected {
                if let Ok(mut write_guard) = pixel_lock.write() {
                    *write_guard = pixel_selected.clone();
                }
            }
            let img = plot_waterfall(
                ui,
                &waterfall_data,
                &(*left_panel_width as f64),
                &(*left_panel_width as f64),
                &mut data.cut_off,
                val,
                pixel_selected,
            );

            let height = ui.available_size().y - logo_height - 20.0;
            ui.add_space(height);
            if gui_conf.dark_mode == true {
                ui.add(egui::Image::new(coconut_dark.texture_id(ctx), coconut_dark.size_vec2() / coconut_dark.width() as f32 * *left_panel_width));
            } else {
                ui.add(egui::Image::new(coconut_light.texture_id(ctx), coconut_light.size_vec2() / coconut_light.width() as f32 * *left_panel_width));
            }

        });
}