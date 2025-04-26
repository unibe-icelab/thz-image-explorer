use crate::gui::application::{FileDialogState, GuiSettingsContainer, THzImageExplorer};
#[cfg(feature = "self_update")]
use crate::update::{check_update, update};
use bevy_egui::egui;
use bevy_egui::egui::{
    vec2, Align2, Color32, ColorImage, InnerResponse, TextureOptions, Vec2, Visuals,
};
use egui_file_dialog::FileDialog;
use egui_plot::{Line, LineStyle, Plot, PlotImage, PlotPoint, PlotPoints};
use egui_theme_switch::ThemeSwitch;
use ndarray::{Array1, Axis};
#[cfg(feature = "self_update")]
use self_update::restart::restart;
#[cfg(feature = "self_update")]
use self_update::update::Release;
#[cfg(feature = "self_update")]
use semver::Version;

fn gaussian(x: &Array1<f64>, params: &[f64]) -> Array1<f64> {
    let x0 = params[0];
    let w = params[1];
    x.mapv(|xi| {
        (2.0 * (-2.0 * (xi - x0).powf(2.0) / (w * w)) / (2.0 * std::f64::consts::PI).sqrt() * w)
            .exp()
    })
}
pub fn settings_window(
    ctx: &egui::Context,
    explorer: &mut THzImageExplorer,
) -> Option<InnerResponse<Option<()>>> {
    egui::Window::new("Settings")
        .fixed_size(Vec2 { x: 600.0, y: 200.0 })
        .anchor(Align2::CENTER_CENTER, Vec2 { x: 0.0, y: 0.0 })
        .collapsible(false)
        .show(ctx, |ui| {
            egui::Grid::new("theme settings")
                .striped(true)
                .show(ui, |ui| {
                    ui.label("Theme: ");
                    if ui
                        .add(ThemeSwitch::new(&mut explorer.thread_communication.gui_settings.theme_preference))
                        .changed()
                    {
                        ui.ctx().set_theme(explorer.thread_communication.gui_settings.theme_preference);
                    };
                    explorer.thread_communication.gui_settings.dark_mode = ui.visuals() == &Visuals::dark();

                    ui.end_row();
                    ui.end_row();
                    ui.label("PSF: ");
                    if ui
                        .button(egui::RichText::new(format!(
                            "{} Open PSF",
                            egui_phosphor::regular::FOLDER_OPEN
                        )))
                        .on_hover_text("The PSF raw data should be located in a directory.")
                        .clicked()
                    {
                        explorer.file_dialog_state = FileDialogState::OpenPSF;
                        explorer.file_dialog.pick_file();
                    }
                    if ui
                        .selectable_label(false, format!("{}", egui_phosphor::regular::INFO))
                        .clicked()
                    {
                        // TODO: add description of PSF format
                    }

                    // TODO: maybe change this check here...
                    if explorer.thread_communication.gui_settings.psf.popt_x.is_empty() {
                        ui.colored_label(egui::Color32::RED, "No PSF loaded.");
                    } else {
                        ui.label(
                            explorer.thread_communication.gui_settings
                                .beam_shape_path
                                .file_name()
                                .unwrap_or("invalid name".as_ref())
                                .to_str()
                                .unwrap_or("invalid path"),
                        );
                    }
                    ui.end_row();
                    ui.end_row();
                });

            let signal_plot = Plot::new("signal")
                // .height(height)
                // .width(width)
                // .y_axis_formatter(s_fmt)
                // .x_axis_formatter(t_fmt)
                // .label_formatter(label_fmt)
                // .coordinates_formatter(Corner::LeftTop, position_fmt)
                // .include_x(&self.tera_flash_conf.t_begin + &self.tera_flash_conf.range)
                // .include_x(self.tera_flash_conf.t_begin)
                // .min_size(vec2(50.0, 100.0))
              ;

            // Assuming `beam_x` is of type `Array2<f64>`
            let beam_x_array = explorer.thread_communication.gui_settings.clone().psf.popt_x;

            let start = -5.0;
            let end = 5.0;
            let step = 0.5;

            let values: Vec<f64> = (0..)
                .map(|i| start + i as f64 * step)
                .take_while(|&x| x <= end)
                .collect();
            let array = Array1::from(values);
            if !beam_x_array.is_empty() {
                let binding = beam_x_array.row(0);
                let first_row = binding.as_slice().unwrap();

                let beam_x_vec: Vec<[f64; 2]> = gaussian(&array, &first_row)
                    .iter()
                    .zip(array.iter())
                    .map(|(p, x)| [*x, *p])
                    .collect();

                signal_plot.show(ui, |signal_plot_ui| {
                    signal_plot_ui.line(
                        Line::new(PlotPoints::from(beam_x_vec))
                            .color(egui::Color32::RED)
                            .style(LineStyle::Solid)
                            .width(2.0)
                            .name("x"),
                    );
                });
            }

            ui.end_row();

            let plot_height = 100.0;
            let plot_width = 100.0;
            let data = &explorer.thread_communication.gui_settings.psf.popt_x;

            let width = data.len_of(Axis(0));
            let height = data.len_of(Axis(1));

            let size = [plot_width / width as f64, plot_height / height as f64]
                .iter()
                .fold(f64::INFINITY, |ai, &bi| ai.min(bi));

            let plot = Plot::new("image")
                .height(0.75 * height as f32 * size as f32)
                .width(0.75 * width as f32 * size as f32)
                .show_axes([false, false])
                .show_x(false)
                .show_y(false)
                .set_margin_fraction(Vec2 { x: 0.0, y: 0.0 })
                .allow_drag(false);

            let max = data
                .iter()
                .fold(f64::NEG_INFINITY, |ai, &bi| ai.max(bi as f64));

            let img = ColorImage::new([width, height], Color32::TRANSPARENT);
            let mut intensity_matrix = vec![vec![0.0; height]; width];
            let mut id_matrix = vec![vec!["".to_string(); height]; width];

            for y in 0..height {
                for x in 0..width {
                    if let Some(i) = data.get((x, y)) {
                        intensity_matrix[x][height - 1 - y] = *i as f64 / max * 100.0;
                        id_matrix[x][height - 1 - y] = format!("{:05}-{:05}", x, y);
                    }
                }
            }

            let texture = ui
                .ctx()
                .load_texture("image", img.clone(), TextureOptions::NEAREST);
            let im = PlotImage::new(
                &texture,
                PlotPoint::new((img.width() as f64) / 2.0, (img.height() as f64) / 2.0),
                img.height() as f32 * vec2(texture.aspect_ratio(), 1.0),
            );

            let _plot_response = plot.show(ui, |plot_ui| {
                plot_ui.image(im);
            });

            ui.label("");
            #[cfg(feature = "self_update")]
            egui::Grid::new("update settings")
                .striped(true)
                .show(ui, |ui| {
                    if ui.button("Check for Updates").clicked() {
                        explorer.new_release = check_update();
                    }

                    let current_version = Version::parse(env!("CARGO_PKG_VERSION")).unwrap();
                    ui.label(format!("Current version: {}", current_version));

                    ui.end_row();

                    if let Some(r) = &explorer.new_release {
                        ui.label(format!("New release: {}", r.version));
                        ui.end_row();
                        if ui.button("Update").clicked() {
                            match update(r.clone()) {
                                Ok(_) => {
                                    log::info!("Update done. {} >> {}", current_version, r.version);
                                    explorer.new_release = None;
                                    explorer.update_text =
                                        "Update done. Please Restart Application.".to_string();
                                }
                                Err(err) => {
                                    log::error!("{}", err);
                                }
                            }
                        }
                    } else {
                        ui.label("");
                        ui.end_row();
                        ui.horizontal(|ui| {
                            ui.disable();
                            let _ = ui.button("Update");
                        });
                        ui.label("You have the latest version");
                    }
                });
            ui.label(explorer.update_text.clone());

            ui.horizontal(|ui| {
                ui.horizontal(|ui| {
                    if !explorer.update_text.is_empty() {
                        ui.disable();
                    };
                    if ui.button("Exit Settings").clicked() {
                        explorer.settings_window_open = false;
                        explorer.update_text = "".to_string();
                    }
                });

                #[cfg(feature = "self_update")]
                if !explorer.update_text.is_empty() && ui.button("Restart").clicked() {
                    restart();
                    ctx.request_repaint(); // Optional: Request repaint for immediate feedback
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
        })
}
