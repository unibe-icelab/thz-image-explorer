use crate::config::{ConfigCommand, GuiThreadCommunication};
use crate::gui::application::FileDialogState;
use crate::gui::gauge_widget::gauge;
use crate::gui::matrix_plot::{make_dummy, plot_matrix, SelectedPixel, ROI};
use crate::gui::toggle_widget::toggle_ui;
use crate::io::{find_files_with_same_extension, load_psf};
use crate::DataPoint;
use dotthz::DotthzMetaData;
use eframe::egui;
use eframe::egui::panel::Side;
use eframe::egui::TextStyle;
use egui_extras::{Column, TableBuilder};
use egui_file_dialog::information_panel::InformationPanel;
use egui_file_dialog::FileDialog;
use egui_plot::PlotPoint;
use std::path::{Path, PathBuf};

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
    thread_communication: &mut GuiThreadCommunication,
    left_panel_width: &f32,
    pixel_selected: &mut SelectedPixel,
    val: &mut PlotPoint,
    mid_point: &mut f32,
    bw: &mut bool,
    cut_off: &mut [f32; 2],
    file_dialog_state: &mut FileDialogState,
    file_dialog: &mut FileDialog,
    information_panel: &mut InformationPanel,
    other_files: &mut Vec<PathBuf>,
    selected_file_name: &mut String,
    scroll_to_selection: &mut bool,
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
                    *file_dialog_state = FileDialogState::Open;
                    file_dialog.pick_file();
                };
                if ui
                    .button(egui::RichText::new(format!(
                        "{} Load Reference",
                        egui_phosphor::regular::FOLDER_OPEN
                    )))
                    .clicked()
                {
                    *file_dialog_state = FileDialogState::OpenRef;
                    file_dialog.pick_file();
                };
            });

            if !other_files.is_empty() {
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
                if *scroll_to_selection {
                    if let Some(selected_index) = other_files.iter().position(|path| {
                        path.file_name().unwrap().to_str().unwrap() == selected_file_name
                    }) {
                        table_builder =
                            table_builder.scroll_to_row(selected_index, Some(egui::Align::Center));
                    }
                    *scroll_to_selection = false;
                }
                let table = table_builder
                    .column(Column::remainder().at_least(120.0)) // "Date Modified"
                    .header(row_height, |_header| {});
                table.body(|body| {
                    body.rows(row_height, other_files.len(), |mut row| {
                        if let Some(item) = &mut other_files.get(row.index()) {
                            let selected =
                                item.file_name().unwrap().to_str().unwrap() == selected_file_name;
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
                                *selected_file_name =
                                    item.file_name().unwrap().to_str().unwrap().to_string();
                                thread_communication.gui_settings.selected_path =
                                    item.to_path_buf();
                                thread_communication
                                    .config_tx
                                    .send(ConfigCommand::OpenFile(item.to_path_buf()))
                                    .expect("unable to send open file cmd");
                            }
                        }
                    });
                });
                if let Some(selected_index) = other_files.iter().position(|path| {
                    path.file_name().unwrap().to_str().unwrap() == selected_file_name
                }) {
                    if ui.input(|i| i.key_pressed(egui::Key::ArrowDown))
                        && selected_index < other_files.len() - 1
                    {
                        let item = other_files[selected_index + 1].clone();
                        *selected_file_name =
                            item.file_name().unwrap().to_str().unwrap().to_string();
                        thread_communication.gui_settings.selected_path = item.to_path_buf();
                        thread_communication
                            .config_tx
                            .send(ConfigCommand::OpenFile(item.to_path_buf()))
                            .expect("unable to send open file cmd");
                        *scroll_to_selection = true;
                    }
                    if ui.input(|i| i.key_pressed(egui::Key::ArrowUp)) && selected_index > 0 {
                        let item = other_files[selected_index - 1].clone();
                        *selected_file_name =
                            item.file_name().unwrap().to_str().unwrap().to_string();
                        thread_communication.gui_settings.selected_path = item.to_path_buf();
                        thread_communication
                            .config_tx
                            .send(ConfigCommand::OpenFile(item.to_path_buf()))
                            .expect("unable to send open file cmd");
                        *scroll_to_selection = true;
                    }
                }
            }

            file_dialog.set_right_panel_width(300.0);

            let mut repaint = false;
            ctx.input(|i| {
                // Check if files were dropped
                if let Some(dropped_file) = i.raw.dropped_files.last() {
                    let path = dropped_file.clone().path.unwrap();
                    *other_files = find_files_with_same_extension(&path).unwrap();
                    *selected_file_name = path.file_name().unwrap().to_str().unwrap().to_string();
                    *scroll_to_selection = true;
                    file_dialog.config_mut().initial_directory = path.clone();
                    thread_communication.gui_settings.selected_path = path.clone();
                    thread_communication
                        .config_tx
                        .send(ConfigCommand::OpenFile(path))
                        .expect("unable to send open file cmd");
                    repaint = true;
                }
            });

            // Update GUI if we dropped a file
            if repaint {
                ctx.request_repaint();
            }

            match file_dialog_state {
                FileDialogState::Open => {
                    if let Some(path) = file_dialog
                        .update_with_right_panel_ui(ctx, &mut |ui, dia| {
                            information_panel.ui(ui, dia);
                        })
                        .picked()
                    {
                        *file_dialog_state = FileDialogState::None;
                        thread_communication
                            .config_tx
                            .send(ConfigCommand::OpenFile(path.to_path_buf()))
                            .expect("unable to send open file cmd");
                    }
                    if let Some(path) = file_dialog.take_picked() {
                        *other_files = find_files_with_same_extension(&path).unwrap();
                        *selected_file_name =
                            path.file_name().unwrap().to_str().unwrap().to_string();
                        *scroll_to_selection = true;
                        file_dialog.config_mut().initial_directory = path.clone();
                        thread_communication.gui_settings.selected_path = path;
                    }
                }
                FileDialogState::OpenRef => {}
                FileDialogState::OpenPSF => {
                    if let Some(path) = file_dialog
                        .update_with_right_panel_ui(ctx, &mut |ui, dia| {
                            information_panel.ui(ui, dia);
                        })
                        .picked()
                    {
                        *file_dialog_state = FileDialogState::None;
                        if let Ok(psf) = load_psf(&path.to_path_buf()) {
                            thread_communication.gui_settings.psf = psf;
                            thread_communication.gui_settings.beam_shape_path = path.to_path_buf();
                        }
                    }
                }
                FileDialogState::Save => {
                    if let Some(_path) = file_dialog.update(ctx).picked() {
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

            ui.separator();
            ui.heading("Scan");
            let mut img_data = make_dummy();
            if let Ok(read_guard) = thread_communication.img_lock.read() {
                img_data = read_guard.clone();
            }

            // read rois from md

            let open_roi = if let Some(a) = pixel_selected.rois.last() {
                if !a.closed {
                    Some(a.clone())
                } else {
                    None
                }
            } else {
                None
            };

            pixel_selected.rois.clear();

            if let Some(labels) = meta_data.md.get("ROI Labels") {
                let roi_labels: Vec<&str> = labels.split(',').collect();
                for (i, label) in roi_labels.iter().enumerate() {
                    if let Some(roi_data) = meta_data.md.get(&format!("ROI {}", i)) {
                        // Ensure we are correctly extracting coordinates
                        let polygon = roi_data
                            .split("],") // Split by "]," to separate coordinate pairs
                            .filter_map(|point| {
                                let cleaned = point.trim_matches(|c| c == '[' || c == ']');
                                let values: Vec<f64> = cleaned
                                    .split(',')
                                    .filter_map(|v| v.trim().parse::<f64>().ok())
                                    .collect();

                                if values.len() == 2 {
                                    Some([values[0], values[1]])
                                } else {
                                    None
                                }
                            })
                            .collect::<Vec<[f64; 2]>>();

                        if !polygon.is_empty() {
                            pixel_selected.rois.push(ROI {
                                polygon,
                                closed: true,
                                name: label.to_string(),
                            });
                        }
                    }
                }
            }

            if let Some(roi) = open_roi {
                pixel_selected.rois.push(roi.clone());
            }

            // TODO: implement selecting reference pixel
            let pixel_clicked = plot_matrix(
                ui,
                &img_data,
                &(*left_panel_width as f64),
                &(height as f64),
                cut_off,
                val,
                pixel_selected,
                mid_point,
                bw,
            );
            if pixel_clicked {
                thread_communication
                    .config_tx
                    .send(ConfigCommand::SetSelectedPixel(pixel_selected.clone()))
                    .unwrap();
                if let Ok(mut write_guard) = thread_communication.pixel_lock.write() {
                    *write_guard = pixel_selected.clone();
                }
                let mut md_update_requested = false;
                for (roi_i, roi) in pixel_selected.rois.iter_mut().enumerate() {
                    if roi.closed {
                        let formatted = roi
                            .polygon
                            .iter()
                            .map(|&[x, y]| format!("[{:.2},{:.2}]", x, y))
                            .collect::<Vec<String>>()
                            .join(",");
                        meta_data.md.insert(format!("ROI {}", roi_i), formatted);
                        md_update_requested = true;

                        if let Some(labels) = meta_data.md.get_mut("ROI Labels") {
                            // Append new ROI label
                            labels.push_str(&roi.name.clone());
                        }
                    }
                }

                if md_update_requested {
                    let labels = pixel_selected
                        .rois
                        .iter()
                        .map(|l| l.name.clone())
                        .collect::<Vec<String>>()
                        .join(",");
                    meta_data.md.insert("ROI Labels".to_string(), labels);

                    if let Ok(mut md) = thread_communication.md_lock.write() {
                        *md = meta_data.clone();
                    }

                    thread_communication
                        .config_tx
                        .send(ConfigCommand::UpdateMetaData(
                            thread_communication.gui_settings.selected_path.clone(),
                        ))
                        .expect("unable to send save file cmd");
                }
            }

            ui.add_space(10.0);
            ui.label("Black/White");
            toggle_ui(ui, bw);
            ui.label(format!("Pixel: {}", pixel_selected.id));
            ui.label(format!("x: {}", pixel_selected.x));
            ui.label(format!("y: {}", pixel_selected.y));

            ui.separator();
            ui.heading("Regions of Interest (ROI)");
            egui::ScrollArea::both().id_salt("rois").show(ui, |ui| {
                egui::Grid::new("rois polygons")
                    .striped(true)
                    .show(ui, |ui| {
                        let mut changed = false;
                        for roi in pixel_selected.rois.iter_mut() {
                            changed |= ui.add(egui::TextEdit::singleline(&mut roi.name)).changed();
                            let points = roi
                                .polygon
                                .iter()
                                .map(|&[x, y]| format!("[{:.2},{:.2}]", x, y))
                                .collect::<Vec<String>>()
                                .join(",");
                            ui.label(points);
                            ui.end_row();
                        }
                        if changed {
                            let labels = pixel_selected
                                .rois
                                .iter()
                                .map(|l| l.name.clone())
                                .collect::<Vec<String>>()
                                .join(",");
                            meta_data.md.insert("ROI Labels".to_string(), labels);

                            dbg!(&meta_data.md.get("ROI Labels"));
                            if let Ok(mut md) = thread_communication.md_lock.write() {
                                *md = meta_data.clone();
                            }
                            thread_communication
                                .config_tx
                                .send(ConfigCommand::UpdateMetaData(
                                    thread_communication.gui_settings.selected_path.clone(),
                                ))
                                .expect("unable to send save file cmd");
                        }
                    });
            });

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
                        thread_communication
                            .config_tx
                            .send(ConfigCommand::UpdateMetaData(
                                thread_communication.gui_settings.selected_path.clone(),
                            ))
                            .expect("unable to send save file cmd");
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
                        thread_communication
                            .config_tx
                            .send(ConfigCommand::LoadMetaData(
                                thread_communication.gui_settings.selected_path.clone(),
                            ))
                            .expect("unable to send open file cmd");
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

                    for attr in attributes_to_delete {
                        if attr.contains("ROI") {
                            if let Some(labels_string) = meta_data.md.get_mut("ROI Labels") {
                                let mut labels = labels_string.split(",").collect::<Vec<&str>>();
                                if let Some(index) = attr
                                    .strip_prefix("ROI ")
                                    .and_then(|num| num.parse::<usize>().ok())
                                {
                                    dbg!(index);
                                    pixel_selected.rois.remove(index);
                                    if pixel_selected.rois.is_empty() {
                                        let mut roi = ROI::default();
                                        roi.name = format!("ROI {}", pixel_selected.rois.len() + 1);
                                        pixel_selected.rois.push(roi);
                                    }
                                    labels.remove(index);
                                    *labels_string = labels.join(",");
                                } else {
                                    if attr == "ROI Labels" {
                                        for roi_i in 0..pixel_selected.rois.len() {
                                            meta_data.md.swap_remove(&format!("ROI {roi_i}"));
                                        }
                                        pixel_selected.rois.clear();
                                        let mut roi = ROI::default();
                                        roi.name = "ROI 0".to_string();
                                        pixel_selected.rois.push(roi);
                                    }
                                }
                            }
                        }
                        meta_data.md.swap_remove(&attr);
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
