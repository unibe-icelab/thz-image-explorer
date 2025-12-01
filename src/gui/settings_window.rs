use crate::config::ThreadCommunication;
use crate::gui::application::{FileDialogState, THzImageExplorer};
#[cfg(feature = "self_update")]
use crate::update::{check_for_software_updates, update};
use crate::APP_INFO;
use bevy::app::AppExit;
use bevy::prelude::MessageWriter;
use bevy_egui::egui;
use bevy_egui::egui::{vec2, Align2, InnerResponse, Popup, PopupCloseBehavior, Vec2};
use egui_theme_switch::ThemeSwitch;
use preferences::Preferences;
#[cfg(feature = "self_update")]
use self_update::restart::restart;
#[cfg(feature = "self_update")]
use semver::Version;

pub fn settings_window(
    ctx: &egui::Context,
    explorer: &mut THzImageExplorer,
    thread_communication: &mut ThreadCommunication,
    exit: &mut MessageWriter<AppExit>,
) -> Option<InnerResponse<Option<()>>> {
    egui::Window::new("Settings")
        .fixed_size(Vec2 {
            x: 400.0,
            y: 1000.0,
        })
        .anchor(Align2::CENTER_CENTER, Vec2 { x: 0.0, y: 0.0 })
        .collapsible(false)
        .show(ctx, |ui| {
            egui::Grid::new("theme settings")
                .striped(true)
                .show(ui, |ui| {
                    ui.label("Theme: ");
                    if ui
                        .add(ThemeSwitch::new(
                            &mut thread_communication.gui_settings.theme_preference,
                        ))
                        .changed()
                    {
                        ui.ctx()
                            .set_theme(thread_communication.gui_settings.theme_preference);

                        // ====================================================================
                        // TEMPORARY WORKAROUND: Remove when bevy_egui supports system theme
                        // ====================================================================
                        // For System theme, detect and apply OS theme
                        if thread_communication.gui_settings.theme_preference
                            == egui::ThemePreference::System
                        {
                            crate::system_theme::apply_system_theme_if_needed(
                                ui.ctx(),
                                thread_communication.gui_settings.theme_preference,
                            );
                        } else {
                            // For Dark/Light modes, explicitly set visuals
                            let is_dark = thread_communication.gui_settings.theme_preference
                                == egui::ThemePreference::Dark;
                            ui.ctx().set_visuals(if is_dark {
                                egui::Visuals::dark()
                            } else {
                                egui::Visuals::light()
                            });
                            // Re-apply handle shape
                            ui.ctx().style_mut(|style| {
                                style.visuals.handle_shape = egui::style::HandleShape::Circle;
                            });
                        }
                        // ====================================================================
                    };

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
                        #[cfg(not(target_os = "macos"))]
                        explorer.file_dialog.pick_file();
                        explorer.file_dialog_state = FileDialogState::OpenPSF;
                    }
                    // Create a unique ID for this filter's info popup
                    let popup_id = ui.make_persistent_id(format!("PSF info_popup"));

                    // Show info icon and handle clicks
                    let info_button = ui.button(format!("{}", egui_phosphor::regular::INFO));

                    Popup::menu(&info_button)
                        .id(popup_id)
                        .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
                        .show(|ui: &mut egui::Ui| {
                            // Set max width for the popup
                            ui.set_max_width(500.0);

                            ui.heading("PSF File Format");
                            ui.label("The .npz file must contain the following datasets:");
                            ui.add_space(8.0);

                            ui.label(
                                egui::RichText::new("Beam width in X (wx) - Hybrid fit:").strong(),
                            );
                            ui.label("  • 'wx_base_a': 1/f coefficient (scalar)");
                            ui.label("  • 'wx_base_b': constant offset (scalar)");
                            ui.label(
                                "  • 'wx_corr_knots_thz': frequency knots for correction (THz)",
                            );
                            ui.label("  • 'wx_corr_values_mm': correction values at knots (mm)");
                            ui.label("  • 'wx_corr_coeff_a/b/c/d': cubic spline coefficients");
                            ui.add_space(4.0);

                            ui.label(
                                egui::RichText::new("Beam width in Y (wy) - Hybrid fit:").strong(),
                            );
                            ui.label("  • 'wy_base_a', 'wy_base_b': base model parameters");
                            ui.label("  • 'wy_corr_knots_thz', 'wy_corr_values_mm': knots/values");
                            ui.label("  • 'wy_corr_coeff_a/b/c/d': cubic spline coefficients");
                            ui.add_space(4.0);

                            ui.label(
                                egui::RichText::new("Beam center in X (x0) - Spline:").strong(),
                            );
                            ui.label("  • 'x0_knots_thz', 'x0_values_mm': knots and values");
                            ui.label("  • 'x0_coeff_a/b/c/d': cubic spline coefficients");
                            ui.add_space(4.0);

                            ui.label(
                                egui::RichText::new("Beam center in Y (y0) - Spline:").strong(),
                            );
                            ui.label("  • 'y0_knots_thz', 'y0_values_mm': knots and values");
                            ui.label("  • 'y0_coeff_a/b/c/d': cubic spline coefficients");
                        });

                    if thread_communication
                        .gui_settings
                        .psf
                        .wx_fit
                        .correction
                        .knots
                        .is_empty()
                    {
                        ui.colored_label(egui::Color32::RED, "No PSF loaded.");
                    } else {
                        ui.label(
                            thread_communication
                                .gui_settings
                                .beam_shape_path
                                .file_name()
                                .unwrap_or("invalid name".as_ref())
                                .to_str()
                                .unwrap_or("invalid path"),
                        );
                    }
                    ui.end_row();
                });

            ui.end_row();

            ui.label("");
            #[cfg(feature = "self_update")]
            egui::Grid::new("update settings")
                .striped(true)
                .show(ui, |ui| {
                    let branch = option_env!("GIT_BRANCH").unwrap_or("(No Git Branch Found)");
                    let commit = option_env!("GIT_HASH").unwrap_or("(No Git Hash Found)");
                    ui.label(format!("Build: {} @ {}", branch, commit));
                    ui.end_row();

                    if ui.button("Check for Updates").clicked() {
                        explorer.new_release = check_for_software_updates();
                    }

                    let current_version =
                        Version::parse(env!("CARGO_PKG_VERSION")).unwrap_or(Version::new(0, 0, 1));
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

            ui.end_row();
            ui.label("Logger");
            ui.end_row();

            ui.allocate_ui(vec2(300.0, 300.0), |ui| {
                egui_logger::logger_ui().show(ui);
            });
            ui.end_row();
            ui.separator();
            ui.end_row();

            ui.vertical_centered(|ui| {
                let escape_key_pressed = ui.input(|i| i.key_pressed(egui::Key::Escape))
                    && explorer.update_text.is_empty();
                ui.add_enabled_ui(explorer.update_text.is_empty(), |ui| {
                    if ui.button("Close").clicked() || escape_key_pressed {
                        explorer.settings_window_open = false;
                        explorer.update_text = "".to_string();

                        let _ = thread_communication
                            .gui_settings
                            .save(&APP_INFO, "config/gui");
                    }
                });

                #[cfg(feature = "self_update")]
                if !explorer.update_text.is_empty() && ui.button("Restart").clicked() {
                    restart();
                    ctx.request_repaint(); // Optional: Request repaint for immediate feedback
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                    exit.write(AppExit::Success);
                }
            });

            // update PSF also in gui thread
            let mut psf_temp = None;
            if let Ok(psf_guard) = thread_communication.psf_lock.try_read() {
                psf_temp = Some((psf_guard.0.clone(), psf_guard.1.clone()));
            }
            if let Some((path, psf)) = psf_temp {
                thread_communication.gui_settings.psf = psf;
                thread_communication.gui_settings.beam_shape_path = path;
            }
        })
}
