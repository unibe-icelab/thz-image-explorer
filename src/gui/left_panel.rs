use crate::config::{send_latest_config, ConfigCommand, ThreadCommunication};
use crate::gui::application::{FileDialogState, THzImageExplorer};
use crate::gui::gauge_widget::gauge;
use crate::gui::matrix_plot::{make_dummy, plot_matrix, ImageState};
use crate::gui::toggle_widget::toggle_ui;
use crate::io::{find_files_with_same_extension, load_psf};
use crate::DataPoint;
use bevy_egui::egui;
use bevy_egui::egui::panel::Side;
use bevy_egui::egui::TextStyle;
use dotthz::DotthzMetaData;
use egui_extras::{Column, TableBuilder};
use std::path::Path;

/// Calculates the width of a single char.
fn calc_char_width(ui: &egui::Ui, char: char) -> f32 {
    ui.fonts(|f| f.glyph_width(&egui::TextStyle::Body.resolve(ui.style()), char))
}

/// Calculates the width of the specified text using the current font configuration.
/// Does not take new lines or text breaks into account!
pub fn calc_text_width(ui: &egui::Ui, text: &str) -> f32 {
    let mut width = 0.0;

    for char in text.chars() {
        width += calc_char_width(ui, char);
    }

    width
}

/// Truncates a date to a specified maximum length `max_length`
/// Returns the truncated filename as a string
pub fn truncate_filename(ui: &egui::Ui, item: &Path, max_length: f32) -> String {
    const TRUNCATE_STR: &str = "...";

    let path = item;

    let file_stem = if path.is_file() {
        path.file_stem().and_then(|f| f.to_str()).unwrap_or("")
    } else {
        item.file_name().unwrap().to_str().unwrap()
    };

    let extension = if path.is_file() {
        path.extension().map_or(String::new(), |ext| {
            format!(".{}", ext.to_str().unwrap_or(""))
        })
    } else {
        String::new()
    };

    let extension_width = calc_text_width(ui, &extension);
    let reserved = extension_width + calc_text_width(ui, TRUNCATE_STR);

    if max_length <= reserved {
        return format!("{TRUNCATE_STR}{extension}");
    }

    let mut width = reserved;
    let mut front = String::new();
    let mut back = String::new();

    for (i, char) in file_stem.chars().enumerate() {
        let w = calc_char_width(ui, char);

        if width + w > max_length {
            break;
        }

        front.push(char);
        width += w;

        let back_index = file_stem.len() - i - 1;

        if back_index <= i {
            break;
        }

        if let Some(char) = file_stem.chars().nth(back_index) {
            let w = calc_char_width(ui, char);

            if width + w > max_length {
                break;
            }

            back.push(char);
            width += w;
        }
    }

    format!(
        "{front}{TRUNCATE_STR}{}{extension}",
        back.chars().rev().collect::<String>()
    )
}

#[allow(clippy::too_many_arguments)]
pub fn left_panel(
    ctx: &egui::Context,
    explorer: &mut THzImageExplorer,
    left_panel_width: &f32,
    image_state: &mut ImageState,
    thread_communication: &mut ThreadCommunication,
) {
    let gauge_size = left_panel_width / 3.0;
    let mut data = DataPoint::default();
    if let Ok(read_guard) = thread_communication.data_lock.read() {
        data = read_guard.clone();
    }
    let mut meta_data = DotthzMetaData::default();
    if let Ok(md) = thread_communication.md_lock.read() {
        meta_data = md.clone();
    }
    if let Some(t_s) = meta_data.md.get("T_S [K]") {
        data.hk.sample_temperature = t_s.parse().unwrap();
    }
    if let Some(pressure) = meta_data.md.get("P [mbar]") {
        data.hk.ambient_pressure = pressure.parse().unwrap();
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
                        &data.hk.sample_temperature,
                        0.0,
                        400.0,
                        gauge_size as f64,
                        "K",
                        "T_S",
                    ));
                    ui.add_space((left_panel_width - 2.0 * gauge_size) / 3.0);
                    ui.add(gauge(
                        &data.hk.ambient_pressure,
                        1.0e-8,
                        1.0e+3,
                        gauge_size as f64,
                        "mbar",
                        "p0",
                    ));
                });
            });
            ui.separator();
            ui.heading("Data Source");
            ui.horizontal(|ui| {
                if ui
                    .button(egui::RichText::new(format!(
                        "{} Load Scan",
                        egui_phosphor::regular::FOLDER_OPEN
                    )))
                    .clicked()
                {
                    explorer.file_dialog_state = FileDialogState::Open;
                    explorer.file_dialog.pick_file();
                };
                if ui
                    .button(egui::RichText::new(format!(
                        "{} Load Reference",
                        egui_phosphor::regular::FOLDER_OPEN
                    )))
                    .clicked()
                {
                    explorer.file_dialog_state = FileDialogState::OpenRef;
                    explorer.file_dialog.pick_file();
                };
            });

            if !explorer.other_files.is_empty() {
                ui.add_space(5.0);
                ui.label("Files in same directory:");
                let row_height = ui
                    .style()
                    .text_styles
                    .get(&TextStyle::Body)
                    .map_or(15.0, |font_id| 1.0 + ui.fonts(|f| f.row_height(font_id)));

                let mut table_builder = TableBuilder::new(ui)
                    .sense(egui::Sense::click())
                    .striped(true)
                    .resizable(false)
                    .max_scroll_height(row_height * 5.0)
                    .cell_layout(egui::Layout::left_to_right(egui::Align::Center));
                if explorer.scroll_to_selection {
                    if let Some(selected_index) = explorer.other_files.iter().position(|path| {
                        path.file_name().unwrap().to_str().unwrap() == explorer.selected_file_name
                    }) {
                        table_builder =
                            table_builder.scroll_to_row(selected_index, Some(egui::Align::Center));
                    }
                    explorer.scroll_to_selection = false;
                }
                let table = table_builder
                    .column(Column::remainder().at_least(120.0)) // "Date Modified"
                    .header(row_height, |_header| {});
                table.body(|body| {
                    body.rows(row_height, explorer.other_files.len(), |mut row| {
                        if let Some(item) = &mut explorer.other_files.get(row.index()) {
                            let selected = item.file_name().unwrap().to_str().unwrap()
                                == explorer.selected_file_name;
                            row.set_selected(selected);

                            row.col(|ui| {
                                let text_width = calc_text_width(
                                    ui,
                                    item.file_name().unwrap().to_str().unwrap(),
                                );

                                // Calc available width for the file name and include a small margin
                                let available_width = ui.available_width() - 15.0;

                                let text = if available_width < text_width {
                                    truncate_filename(ui, item, available_width)
                                } else {
                                    item.file_name().unwrap().to_str().unwrap().to_string()
                                };
                                let display_name = text.to_string();
                                let name_response =
                                    ui.add(egui::Label::new(display_name).selectable(false));
                                if available_width < text_width {
                                    name_response
                                        .on_hover_text(item.file_name().unwrap().to_str().unwrap());
                                }
                            });
                            if row.response().clicked() {
                                explorer.selected_file_name =
                                    item.file_name().unwrap().to_str().unwrap().to_string();
                                thread_communication.gui_settings.selected_path =
                                    item.to_path_buf();

                                send_latest_config(
                                    thread_communication,
                                    ConfigCommand::OpenFile(item.to_path_buf()),
                                );
                            }
                        }
                    });
                });
                if let Some(selected_index) = explorer.other_files.iter().position(|path| {
                    path.file_name().unwrap().to_str().unwrap() == explorer.selected_file_name
                }) {
                    if ui.input(|i| i.key_pressed(egui::Key::ArrowDown))
                        && selected_index < explorer.other_files.len() - 1
                    {
                        let item = explorer.other_files[selected_index + 1].clone();
                        explorer.selected_file_name =
                            item.file_name().unwrap().to_str().unwrap().to_string();
                        thread_communication.gui_settings.selected_path = item.to_path_buf();
                        send_latest_config(
                            thread_communication,
                            ConfigCommand::OpenFile(item.to_path_buf()),
                        );
                        explorer.scroll_to_selection = true;
                    }
                    if ui.input(|i| i.key_pressed(egui::Key::ArrowUp)) && selected_index > 0 {
                        let item = explorer.other_files[selected_index - 1].clone();
                        explorer.selected_file_name =
                            item.file_name().unwrap().to_str().unwrap().to_string();
                        thread_communication.gui_settings.selected_path = item.to_path_buf();

                        send_latest_config(
                            thread_communication,
                            ConfigCommand::OpenFile(item.to_path_buf()),
                        );
                        explorer.scroll_to_selection = true;
                    }
                }
            }

            explorer.file_dialog.set_right_panel_width(300.0);

            let mut repaint = false;
            ctx.input(|i| {
                // Check if files were dropped
                if let Some(dropped_file) = i.raw.dropped_files.last() {
                    let path = dropped_file.clone().path.unwrap();
                    explorer.other_files = find_files_with_same_extension(&path).unwrap();
                    explorer.selected_file_name =
                        path.file_name().unwrap().to_str().unwrap().to_string();
                    explorer.scroll_to_selection = true;
                    explorer.file_dialog.config_mut().initial_directory = path.clone();
                    thread_communication.gui_settings.selected_path = path.clone();
                    send_latest_config(thread_communication, ConfigCommand::OpenFile(path));
                    repaint = true;
                }
            });

            // Update GUI if we dropped a file
            if repaint {
                ctx.request_repaint();
            }

            match explorer.file_dialog_state {
                FileDialogState::Open => {
                    if let Some(path) = explorer
                        .file_dialog
                        .update_with_right_panel_ui(ctx, &mut |ui, dia| {
                            explorer.information_panel.ui(ui, dia);
                        })
                        .picked()
                    {
                        explorer.file_dialog_state = FileDialogState::None;
                        send_latest_config(
                            thread_communication,
                            ConfigCommand::OpenFile(path.to_path_buf()),
                        );
                    }
                    if let Some(path) = explorer.file_dialog.take_picked() {
                        explorer.other_files = find_files_with_same_extension(&path).unwrap();
                        explorer.selected_file_name =
                            path.file_name().unwrap().to_str().unwrap().to_string();
                        explorer.scroll_to_selection = true;
                        explorer.file_dialog.config_mut().initial_directory = path.clone();
                        thread_communication.gui_settings.selected_path = path;
                    }
                }
                FileDialogState::OpenRef => {
                    if let Some(path) = explorer
                        .file_dialog
                        .update_with_right_panel_ui(ctx, &mut |ui, dia| {
                            explorer.information_panel.ui(ui, dia);
                        })
                        .picked()
                    {
                        explorer.file_dialog_state = FileDialogState::None;
                        send_latest_config(
                            thread_communication,
                            ConfigCommand::OpenRef(path.to_path_buf()),
                        );
                    }
                }
                FileDialogState::OpenPSF => {
                    if let Some(path) = explorer
                        .file_dialog
                        .update_with_right_panel_ui(ctx, &mut |ui, dia| {
                            explorer.information_panel.ui(ui, dia);
                        })
                        .picked()
                    {
                        explorer.file_dialog_state = FileDialogState::None;
                        if let Ok(psf) = load_psf(&path.to_path_buf()) {
                            thread_communication.gui_settings.psf = psf;
                            thread_communication.gui_settings.beam_shape_path = path.to_path_buf();
                        }
                    }
                }
                FileDialogState::Save => {
                    if let Some(_path) = explorer.file_dialog.update(ctx).picked() {
                        explorer.file_dialog_state = FileDialogState::None;
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

            ui.separator();
            ui.heading("Scan");
            let mut img_data = make_dummy();
            if let Ok(read_guard) = thread_communication.img_lock.read() {
                img_data = read_guard.clone();
            }

            // TODO: implement selecting reference pixel
            let pixel_clicked = plot_matrix(
                ui,
                &img_data,
                &(*left_panel_width as f64),
                &(height as f64),
                explorer,
                image_state,
                &data.rois,
            );
            if pixel_clicked {
                send_latest_config(
                    thread_communication,
                    ConfigCommand::SetSelectedPixel(explorer.pixel_selected.clone()),
                );

                // Check if any ROI was just closed and send AddROI command
                if let Some(roi) = &explorer.pixel_selected.open_roi {
                    if roi.closed {
                        send_latest_config(
                            thread_communication,
                            ConfigCommand::AddROI(roi.clone()),
                        );
                        explorer.pixel_selected.open_roi = None;
                    }
                }
            }

            ui.add_space(10.0);
            ui.label("Black/White");
            toggle_ui(ui, &mut explorer.bw);
            ui.label(format!("Pixel: {}", explorer.pixel_selected.id));
            ui.label(format!("x: {}", explorer.pixel_selected.x));
            ui.label(format!("y: {}", explorer.pixel_selected.y));

            ui.separator();
            ui.heading("Regions of Interest (ROI)");
            egui::ScrollArea::both().id_salt("rois").show(ui, |ui| {
                egui::Grid::new("rois polygons")
                    .striped(true)
                    .show(ui, |ui| {
                        for roi in data.rois.iter_mut() {
                            if ui
                                .selectable_label(
                                    false,
                                    egui::RichText::new(format!(
                                        "{}",
                                        egui_phosphor::regular::TRASH
                                    )),
                                )
                                .clicked()
                            {
                                send_latest_config(
                                    thread_communication,
                                    ConfigCommand::DeleteROI(roi.name.clone()),
                                );
                            }

                            let old_name = roi.name.clone();
                            let changed =
                                ui.add(egui::TextEdit::singleline(&mut roi.name)).changed();
                            let points = roi
                                .polygon
                                .iter()
                                .map(|&[x, y]| format!("[{:.2},{:.2}]", x, y))
                                .collect::<Vec<String>>()
                                .join(",");
                            ui.label(points);
                            ui.end_row();

                            if changed {
                                send_latest_config(
                                    thread_communication,
                                    ConfigCommand::UpdateROI(old_name.clone(), roi.clone()),
                                );
                            }
                        }

                        if let Some(roi) = &mut explorer.pixel_selected.open_roi {
                            let mut delete_roi = false;
                            if ui
                                .selectable_label(
                                    false,
                                    egui::RichText::new(format!(
                                        "{}",
                                        egui_phosphor::regular::TRASH
                                    )),
                                )
                                .clicked()
                            {
                                delete_roi = true;
                            }
                            ui.add(egui::TextEdit::singleline(&mut roi.name));
                            let points = roi
                                .polygon
                                .iter()
                                .map(|&[x, y]| format!("[{:.2},{:.2}]", x, y))
                                .collect::<Vec<String>>()
                                .join(",");
                            ui.label(points);
                            ui.end_row();
                            if delete_roi {
                                explorer.pixel_selected.open_roi = None;
                            }
                        }
                    });
            });

            if ui
                .button("Save ROIs")
                .on_hover_text("Save current ROIs to file")
                .clicked()
            {
                send_latest_config(
                    thread_communication,
                    ConfigCommand::SaveROIs(
                        thread_communication.gui_settings.selected_path.clone(),
                    ),
                );
            }

            ui.separator();
            ui.heading("Meta Data");
            ui.horizontal(|ui| {
                let text = if thread_communication.gui_settings.meta_data_edit {
                    "Save"
                } else {
                    "Edit"
                };
                if ui
                    .selectable_label(
                        thread_communication.gui_settings.meta_data_edit,
                        egui::RichText::new(format!("{} {}", egui_phosphor::regular::PENCIL, text)),
                    )
                    .clicked()
                {
                    if thread_communication.gui_settings.meta_data_edit {
                        send_latest_config(
                            thread_communication,
                            ConfigCommand::UpdateMetaData(
                                thread_communication.gui_settings.selected_path.clone(),
                            ),
                        );
                    }
                    thread_communication.gui_settings.meta_data_edit =
                        !thread_communication.gui_settings.meta_data_edit;
                }

                if thread_communication.gui_settings.meta_data_edit {
                    if ui
                        .button(egui::RichText::new(format!(
                            "{} Revert",
                            egui_phosphor::regular::ARROW_COUNTER_CLOCKWISE
                        )))
                        .clicked()
                    {
                        thread_communication.gui_settings.meta_data_edit = false;
                        thread_communication.gui_settings.meta_data_unlocked = false;
                        send_latest_config(
                            thread_communication,
                            ConfigCommand::LoadMetaData(
                                thread_communication.gui_settings.selected_path.clone(),
                            ),
                        );
                    }
                    if ui.button(egui::RichText::new("Cancel")).clicked() {
                        thread_communication.gui_settings.meta_data_edit = false;
                        thread_communication.gui_settings.meta_data_unlocked = false;
                    }
                }
            });
            egui::ScrollArea::both().show(ui, |ui| {
                egui::Grid::new("meta_data").striped(true).show(ui, |ui| {
                    // this is an aesthetic hack to draw the empty meta-data grid to full width
                    if meta_data.md.is_empty() {
                        ui.label("Data");
                        ui.label(format!("{:50}", " "));
                        ui.end_row();
                    }
                    let mut attributes_to_delete = vec![];
                    for (name, value) in meta_data.md.iter_mut() {
                        ui.label(name);
                        ui.horizontal(|ui| {
                            if thread_communication.gui_settings.meta_data_edit {
                                if ui
                                    .selectable_label(
                                        false,
                                        egui::RichText::new(format!(
                                            "{}",
                                            egui_phosphor::regular::TRASH
                                        )),
                                    )
                                    .clicked()
                                {
                                    attributes_to_delete.push(name.clone());
                                }
                                let lock = if thread_communication.gui_settings.meta_data_unlocked {
                                    egui::RichText::new(format!(
                                        "{}",
                                        egui_phosphor::regular::LOCK_OPEN
                                    ))
                                } else {
                                    egui::RichText::new(format!("{}", egui_phosphor::regular::LOCK))
                                };
                                if ui
                                    .selectable_label(
                                        thread_communication.gui_settings.meta_data_unlocked,
                                        lock,
                                    )
                                    .clicked()
                                {
                                    thread_communication.gui_settings.meta_data_unlocked =
                                        !thread_communication.gui_settings.meta_data_unlocked;
                                }
                                if thread_communication.gui_settings.meta_data_unlocked {
                                    ui.add(
                                        egui::TextEdit::singleline(value)
                                            .desired_width(ui.available_width()),
                                    );
                                } else {
                                    ui.label(value.clone());
                                }
                            } else {
                                ui.label(value.clone());
                            }
                        });
                        ui.end_row()
                    }

                    for attr in attributes_to_delete.iter() {
                        meta_data.md.swap_remove(attr);
                    }

                    ui.label("User:");
                    if thread_communication.gui_settings.meta_data_edit {
                        ui.add(
                            egui::TextEdit::singleline(&mut meta_data.user)
                                .desired_width(ui.available_width()),
                        );
                    } else {
                        ui.label(meta_data.user.clone());
                    }
                    ui.end_row();
                    ui.label("E-mail:");
                    if thread_communication.gui_settings.meta_data_edit {
                        ui.add(
                            egui::TextEdit::singleline(&mut meta_data.email)
                                .desired_width(ui.available_width()),
                        );
                    } else {
                        ui.label(meta_data.email.clone());
                    }
                    ui.end_row();
                    ui.label("ORCID:");
                    if thread_communication.gui_settings.meta_data_edit {
                        ui.add(
                            egui::TextEdit::singleline(&mut meta_data.orcid)
                                .desired_width(ui.available_width()),
                        );
                    } else {
                        ui.label(meta_data.orcid.clone());
                    }
                    ui.end_row();
                    ui.label("Institution:");
                    if thread_communication.gui_settings.meta_data_edit {
                        ui.add(
                            egui::TextEdit::singleline(&mut meta_data.institution)
                                .desired_width(ui.available_width()),
                        );
                    } else {
                        ui.label(meta_data.institution.clone());
                    }
                    ui.end_row();
                    ui.label("Instrument:");
                    if thread_communication.gui_settings.meta_data_edit {
                        ui.add(
                            egui::TextEdit::singleline(&mut meta_data.instrument)
                                .desired_width(ui.available_width()),
                        );
                    } else {
                        ui.label(meta_data.instrument.clone());
                    }
                    ui.end_row();
                    ui.label("Version:");
                    if thread_communication.gui_settings.meta_data_edit {
                        ui.add(
                            egui::TextEdit::singleline(&mut meta_data.version)
                                .desired_width(ui.available_width()),
                        );
                    } else {
                        ui.label(meta_data.version.clone());
                    }
                    ui.end_row();
                    ui.label("Mode:");
                    if thread_communication.gui_settings.meta_data_edit {
                        ui.add(
                            egui::TextEdit::singleline(&mut meta_data.mode)
                                .desired_width(ui.available_width()),
                        );
                    } else {
                        ui.label(meta_data.mode.clone());
                    }
                    ui.end_row();
                    ui.label("Date:");
                    if thread_communication.gui_settings.meta_data_edit {
                        ui.add(
                            egui::TextEdit::singleline(&mut meta_data.date)
                                .desired_width(ui.available_width()),
                        );
                    } else {
                        ui.label(meta_data.date.clone());
                    }
                    ui.end_row();
                    ui.label("Time:");
                    if thread_communication.gui_settings.meta_data_edit {
                        ui.add(
                            egui::TextEdit::singleline(&mut meta_data.time)
                                .desired_width(ui.available_width()),
                        );
                    } else {
                        ui.label(meta_data.time.clone());
                    }
                    ui.end_row();
                });
            });
            if thread_communication.gui_settings.meta_data_edit {
                if let Ok(mut md) = thread_communication.md_lock.write() {
                    *md = meta_data.clone();
                }
            }
        });
}
