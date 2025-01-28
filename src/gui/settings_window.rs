use crate::gui::application::{FileDialogState, GuiSettingsContainer};
#[cfg(feature = "self_update")]
use crate::update::{check_update, update};
use eframe::egui;
use eframe::egui::{Align2, InnerResponse, Vec2, Visuals};
use egui_file_dialog::FileDialog;
use egui_theme_switch::ThemeSwitch;
#[cfg(feature = "self_update")]
use self_update::restart::restart;
#[cfg(feature = "self_update")]
use self_update::update::Release;
#[cfg(feature = "self_update")]
use semver::Version;

pub fn settings_window(
    ctx: &egui::Context,
    gui_conf: &mut GuiSettingsContainer,
    #[cfg(feature = "self_update")] new_release: &mut Option<Release>,
    settings_window_open: &mut bool,
    update_text: &mut String,
    file_dialog_state: &mut FileDialogState,
    file_dialog: &mut FileDialog,
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
                        .add(ThemeSwitch::new(&mut gui_conf.theme_preference))
                        .changed()
                    {
                        ui.ctx().set_theme(gui_conf.theme_preference);
                    };
                    gui_conf.dark_mode = ui.visuals() == &Visuals::dark();

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
                        *file_dialog_state = FileDialogState::OpenPSF;
                        file_dialog.pick_file();
                    }
                    if ui
                        .selectable_label(false, format!("{}", egui_phosphor::regular::INFO))
                        .clicked()
                    {
                        // TODO: add description of PSF format
                    }
                    if gui_conf.beam_shape.is_empty() {
                        ui.colored_label(egui::Color32::RED, "No PSF loaded.");
                    } else {
                        ui.label(gui_conf.beam_shape_path.to_str().unwrap_or("invalid path"));
                    }
                    ui.end_row();
                    ui.end_row();
                });
            ui.label("");
            #[cfg(feature = "self_update")]
            egui::Grid::new("update settings")
                .striped(true)
                .show(ui, |ui| {
                    if ui.button("Check for Updates").clicked() {
                        *new_release = check_update();
                    }

                    let current_version = Version::parse(env!("CARGO_PKG_VERSION")).unwrap();
                    ui.label(format!("Current version: {}", current_version));

                    ui.end_row();

                    if let Some(r) = &new_release {
                        ui.label(format!("New release: {}", r.version));
                        ui.end_row();
                        if ui.button("Update").clicked() {
                            match update(r.clone()) {
                                Ok(_) => {
                                    log::info!("Update done. {} >> {}", current_version, r.version);
                                    *new_release = None;
                                    *update_text =
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
            ui.label(update_text.clone());

            ui.horizontal(|ui| {
                ui.horizontal(|ui| {
                    if !update_text.is_empty() {
                        ui.disable();
                    };
                    if ui.button("Exit Settings").clicked() {
                        *settings_window_open = false;
                        *update_text = "".to_string();
                    }
                });

                #[cfg(feature = "self_update")]
                if !update_text.is_empty() && ui.button("Restart").clicked() {
                    restart();
                    ctx.request_repaint(); // Optional: Request repaint for immediate feedback
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
        })
}
