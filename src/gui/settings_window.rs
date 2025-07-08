use crate::config::ThreadCommunication;
use crate::gui::application::{FileDialogState, THzImageExplorer};
#[cfg(feature = "self_update")]
use crate::update::{check_for_software_updates, update};
use crate::APP_INFO;
use bevy_egui::egui;
use bevy_egui::egui::{vec2, Align2, InnerResponse, Vec2, Visuals};
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
) -> Option<InnerResponse<Option<()>>> {
    egui::Window::new("Settings")
        .fixed_size(Vec2 { x: 400.0, y: 1000.0 })
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
                    };
                    thread_communication.gui_settings.dark_mode = ui.visuals() == &Visuals::dark();

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
                    }
                    // Create a unique ID for this filter's info popup
                    let popup_id = ui.make_persistent_id(format!("PSF info_popup"));

                    // Show info icon and handle clicks
                    let info_button = ui.button(format!("{}", egui_phosphor::regular::INFO));
                    if info_button.clicked() {
                        ui.memory_mut(|mem| mem.toggle_popup(popup_id));
                    }

                    egui::popup_below_widget(
                        ui,
                        popup_id,
                        &info_button,
                        egui::popup::PopupCloseBehavior::CloseOnClickOutside,
                        |ui: &mut egui::Ui| {
                            // Set max width for the popup
                            ui.set_max_width(400.0);

                            // Add description text

                            // The PSF format is an npz file containing the following data structure:
                            // - 'low_cut': float, low cut-off frequency
                            // - 'high_cut': float, high cut-off frequency
                            // - 'start_freq': float, start frequency for filters
                            // - 'end_freq': float, end frequency for filters
                            // - 'n_filters': int, number of filters
                            // - 'filters': ndarray, filter coefficients, shape (n_filters, len(times_psf) // 5)
                            // - 'filt_freqs': ndarray, filter frequencies, shape (n_filters,)
                            // - '[x_0, w_x]': ndarray, fitted x parameters, shape (n_filters, 2)
                            // - '[y_0, w_y]': ndarray, fitted y parameters, shape (n_filters, 2)

                            ui.label("The PSF format is an npz file containing:");
                            ui.label("- 'low_cut': float, low cut-off frequency");
                            ui.label("- 'high_cut': float, high cut-off frequency");
                            ui.label("- 'start_freq': float, start frequency for filters");
                            ui.label("- 'end_freq': float, end frequency for filters");
                            ui.label("- 'n_filters': int, number of filters");
                            ui.label("- 'filters': ndarray, filter coefficients, shape (n_filters, len(times_psf) // 5)");
                            ui.label("- 'filt_freqs': ndarray, filter frequencies, shape (n_filters,)");
                            ui.label("- '[x_0, w_x]': ndarray, fitted x parameters, shape (n_filters, 2)");
                            ui.label("- '[y_0, w_y]': ndarray, fitted y parameters, shape (n_filters, 2)");
                        },
                    );

                    if thread_communication.gui_settings.psf.popt_x.is_empty() {
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
                    if ui.button("Check for Updates").clicked() {
                        explorer.new_release = check_for_software_updates();
                    }

                    let current_version = Version::parse(env!("CARGO_PKG_VERSION")).unwrap_or(Version::new(0, 0, 1));
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

            ui.horizontal(|ui| {
                ui.horizontal(|ui| {
                    if !explorer.update_text.is_empty() {
                        ui.disable();
                    };
                    let escape_key_pressed = ui.input(|i| i.key_pressed(egui::Key::Escape));
                    ui.vertical_centered(|ui| {
                        if ui.button("Close").clicked() || escape_key_pressed {
                            explorer.settings_window_open = false;
                            explorer.update_text = "".to_string();

                            thread_communication.gui_settings.dark_mode = ui.visuals() == &Visuals::dark();

                            let _ = thread_communication
                                .gui_settings
                                .save(&APP_INFO, "config/gui");
                        }
                    });
                });

                #[cfg(feature = "self_update")]
                if !explorer.update_text.is_empty() && ui.button("Restart").clicked() {
                    restart();
                    ctx.request_repaint(); // Optional: Request repaint for immediate feedback
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
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
