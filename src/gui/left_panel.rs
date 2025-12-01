use crate::config::{send_latest_config, ConfigCommand, ThreadCommunication};
use crate::gui::application::{FileDialogState, THzImageExplorer};
use crate::gui::gauge_widget::gauge;
use crate::gui::matrix_plot::{make_dummy, plot_matrix, ColorBarState, ImageState};
use crate::gui::toggle_widget::toggle_ui;
use crate::io::find_files_with_same_extension;
use crate::PlotDataContainer;
use bevy_egui::egui;
use bevy_egui::egui::panel::Side;
use bevy_egui::egui::TextStyle;
use dotthz::DotthzMetaData;
use egui_extras::{Column, TableBuilder};
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Calculates the width of a single char.
fn calc_char_width(ui: &egui::Ui, char: char) -> f32 {
    ui.fonts_mut(|f| f.glyph_width(&egui::TextStyle::Body.resolve(ui.style()), char))
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
    color_bar_state: &mut ColorBarState,
    thread_communication: &mut ThreadCommunication,
) {
    let mut data = PlotDataContainer::default();
    if let Ok(read_guard) = thread_communication.data_lock.read() {
        data = read_guard.clone();
    }
    if let Ok(roi) = thread_communication.roi_rx.try_recv() {
        match roi {
            Some(roi) => {
                explorer.rois.insert(roi.0, roi.1);
            }
            None => {
                explorer.rois.clear();
            }
        };
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
                ui.heading("Data Source");
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui
                        .button(egui::RichText::new(format!(
                            "{} Load Scan",
                            egui_phosphor::regular::FOLDER_OPEN
                        )))
                        .clicked()
                    {
                        #[cfg(not(target_os = "macos"))]
                        explorer.file_dialog.pick_file();
                        explorer.file_dialog_state = FileDialogState::Open;
                    };
                    if ui
                        .button(egui::RichText::new(format!(
                            "{} Load Reference",
                            egui_phosphor::regular::FOLDER_OPEN
                        )))
                        .clicked()
                    {
                        #[cfg(not(target_os = "macos"))]
                        explorer.file_dialog.pick_file();
                        explorer.file_dialog_state = FileDialogState::OpenRef;
                    };
                });

                if !explorer.other_files.is_empty() {
                    ui.add_space(5.0);
                    ui.label("Files in same directory:");
                    let row_height = ui
                        .style()
                        .text_styles
                        .get(&TextStyle::Body)
                        .map_or(15.0, |font_id| {
                            1.0 + ui.fonts_mut(|f| f.row_height(font_id))
                        });

                    let mut table_builder = TableBuilder::new(ui)
                        .sense(egui::Sense::click())
                        .striped(true)
                        .resizable(false)
                        .max_scroll_height(row_height * 5.0)
                        .cell_layout(egui::Layout::left_to_right(egui::Align::Center));
                    if explorer.scroll_to_selection {
                        if let Some(selected_index) = explorer.other_files.iter().position(|path| {
                            path.file_name().unwrap().to_str().unwrap()
                                == explorer.selected_file_name
                        }) {
                            table_builder = table_builder
                                .scroll_to_row(selected_index, Some(egui::Align::Center));
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
                                        name_response.on_hover_text(
                                            item.file_name().unwrap().to_str().unwrap(),
                                        );
                                    }
                                });
                                if row.response().clicked() {
                                    explorer.selected_file_name =
                                        item.file_name().unwrap().to_str().unwrap().to_string();
                                    thread_communication.gui_settings.selected_path =
                                        item.to_path_buf();

                                    explorer.new_meta_data = vec![("".to_string(), "".to_string())];

                                    explorer.rois = HashMap::new();

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
                            explorer.new_meta_data = vec![("".to_string(), "".to_string())];
                            explorer.rois = HashMap::new();
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
                            explorer.new_meta_data = vec![("".to_string(), "".to_string())];
                            explorer.rois = HashMap::new();
                            send_latest_config(
                                thread_communication,
                                ConfigCommand::OpenFile(item.to_path_buf()),
                            );
                            explorer.scroll_to_selection = true;
                        }
                    }
                }
                #[cfg(not(target_os = "macos"))]
                {
                    explorer.file_dialog.set_right_panel_width(300.0);
                }

                ctx.input(|i| {
                    // Check if files were dropped
                    if let Some(dropped_file) = i.raw.dropped_files.last() {
                        let path = dropped_file.clone().path.unwrap();

                        if let Some(ext) = path.extension() {
                            if ext == "npz" {
                                send_latest_config(
                                    &thread_communication,
                                    ConfigCommand::OpenPSF(path.clone()),
                                );
                            } else {
                                explorer.other_files =
                                    find_files_with_same_extension(&path).unwrap();
                                explorer.selected_file_name =
                                    path.file_name().unwrap().to_str().unwrap().to_string();
                                explorer.scroll_to_selection = true;
                                #[cfg(not(target_os = "macos"))]
                                {
                                    explorer.file_dialog.config_mut().initial_directory =
                                        path.clone();
                                }
                                thread_communication.gui_settings.selected_path = path.clone();
                                explorer.new_meta_data = vec![("".to_string(), "".to_string())];
                                explorer.rois = HashMap::new();
                                send_latest_config(
                                    thread_communication,
                                    ConfigCommand::OpenFile(path),
                                );
                                #[cfg(target_os = "macos")]
                                {
                                    if let Ok(mut path_guard) =
                                        thread_communication.macos_path_lock.write()
                                    {
                                        *path_guard =
                                            thread_communication.gui_settings.selected_path.clone();
                                    }
                                }
                            }
                        }
                    }
                });

                match explorer.file_dialog_state {
                    FileDialogState::Open => {
                        #[cfg(target_os = "macos")]
                        {
                            // use RFD for macOS to be able to use the dotTHz plugin
                            let thread_communication_clone = thread_communication.clone();
                            std::thread::spawn(move || {
                                let task = rfd::AsyncFileDialog::new()
                                    .set_title("Open File")
                                    .add_filter("thz", &["thz", "thzimg", "thzswp"])
                                    .pick_file();

                                futures::executor::block_on(async {
                                    if let Some(file) = task.await {
                                        if let Ok(mut path_guard) =
                                            thread_communication_clone.macos_path_lock.write()
                                        {
                                            *path_guard = file.path().to_path_buf();
                                        }
                                        send_latest_config(
                                            &thread_communication_clone,
                                            ConfigCommand::OpenFile(file.path().to_path_buf()),
                                        );
                                    }
                                });
                            });

                            explorer.file_dialog_state = FileDialogState::None;
                        }

                        #[cfg(not(target_os = "macos"))]
                        {
                            if let Some(path) = explorer
                                .file_dialog
                                .update_with_right_panel_ui(ctx, &mut |ui, dia| {
                                    explorer.information_panel.ui(ui, dia);
                                })
                                .picked()
                            {
                                explorer.file_dialog_state = FileDialogState::None;
                                explorer.new_meta_data = vec![("".to_string(), "".to_string())];
                                explorer.rois = HashMap::new();
                                send_latest_config(
                                    thread_communication,
                                    ConfigCommand::OpenFile(path.to_path_buf()),
                                );
                            }
                        }
                    }
                    FileDialogState::OpenRef => {
                        #[cfg(target_os = "macos")]
                        {
                            // use RFD for macOS to be able to use the dotTHz plugin
                            let thread_communication_clone = thread_communication.clone();
                            std::thread::spawn(move || {
                                let task = rfd::AsyncFileDialog::new()
                                    .set_title("Open Reference File")
                                    .add_filter("thz", &["thz"])
                                    .pick_file();

                                futures::executor::block_on(async {
                                    if let Some(file) = task.await {
                                        send_latest_config(
                                            &thread_communication_clone,
                                            ConfigCommand::OpenRef(file.path().to_path_buf()),
                                        );
                                    }
                                });
                            });

                            explorer.file_dialog_state = FileDialogState::None;
                        }
                        #[cfg(not(target_os = "macos"))]
                        {
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
                    }
                    FileDialogState::OpenPSF => {
                        #[cfg(target_os = "macos")]
                        {
                            // use RFD for macOS to be able to use the dotTHz plugin
                            let thread_communication_clone = thread_communication.clone();
                            std::thread::spawn(move || {
                                let task = rfd::AsyncFileDialog::new()
                                    .set_title("Open File")
                                    .add_filter("npz", &["npz"])
                                    .pick_file();

                                futures::executor::block_on(async {
                                    if let Some(file) = task.await {
                                        send_latest_config(
                                            &thread_communication_clone,
                                            ConfigCommand::OpenPSF(file.path().to_path_buf()),
                                        );
                                    }
                                });
                            });

                            explorer.file_dialog_state = FileDialogState::None;
                        }
                        #[cfg(not(target_os = "macos"))]
                        {
                            if let Some(path) = explorer
                                .file_dialog
                                .update_with_right_panel_ui(ctx, &mut |ui, dia| {
                                    explorer.information_panel.ui(ui, dia);
                                })
                                .picked()
                            {
                                send_latest_config(
                                    &thread_communication,
                                    ConfigCommand::OpenPSF(path.to_path_buf()),
                                );
                                explorer.file_dialog_state = FileDialogState::None;
                            }
                        }
                    }
                    FileDialogState::Save => {
                        // if let Some(_path) = explorer.file_dialog.update(ctx).picked() {
                        //     explorer.file_dialog_state = FileDialogState::None;
                        // }
                    }
                    FileDialogState::SaveToVTU => {
                        #[cfg(target_os = "macos")]
                        {
                            // use RFD for macOS to be able to use the dotTHz plugin
                            let thread_communication_clone = thread_communication.clone();
                            std::thread::spawn(move || {
                                let task = rfd::AsyncFileDialog::new()
                                    .set_title("Save File")
                                    .set_file_name("thz_scan.vtu")
                                    .save_file();

                                futures::executor::block_on(async {
                                    if let Some(file) = task.await {
                                        send_latest_config(
                                            &thread_communication_clone,
                                            ConfigCommand::SaveVTU(file.path().to_path_buf()),
                                        );
                                    }
                                });
                            });

                            explorer.file_dialog_state = FileDialogState::None;
                        }
                        #[cfg(not(target_os = "macos"))]
                        {
                            if let Some(path) = explorer
                                .file_dialog
                                .update_with_right_panel_ui(ctx, &mut |ui, dia| {
                                    explorer.information_panel.ui(ui, dia);
                                })
                                .picked()
                            {
                                send_latest_config(
                                    &thread_communication,
                                    ConfigCommand::SaveVTU(path.to_path_buf()),
                                );
                                explorer.file_dialog_state = FileDialogState::None;
                            }
                        }
                    }
                    FileDialogState::None => {
                        #[cfg(target_os = "macos")]
                        {
                            if let Ok(path_guard) = thread_communication.macos_path_lock.read() {
                                if thread_communication.gui_settings.selected_path != *path_guard {
                                    explorer.new_meta_data = vec![("".to_string(), "".to_string())];
                                    explorer.rois = HashMap::new();
                                }
                                thread_communication.gui_settings.selected_path =
                                    path_guard.clone();
                            }
                        }
                    }
                }

                ui.separator();
                ui.heading("Housekeeping");
                ui.separator();

                let gauge_size = left_panel_width / 4.0;

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

            let bottom_height = 100.0;
            let height = ui.available_size().y - bottom_height - 20.0;

            ui.heading("Scan");
            let mut img_data = make_dummy();
            if let Ok(read_guard) = thread_communication.img_lock.read() {
                img_data = read_guard.clone();
            }

            let pixel_clicked = plot_matrix(
                ui,
                &img_data,
                &(*left_panel_width as f64),
                &(height as f64),
                explorer,
                image_state,
                color_bar_state,
            );
            if pixel_clicked {
                send_latest_config(
                    thread_communication,
                    ConfigCommand::SetSelectedPixel(explorer.pixel_selected.clone()),
                );

                // Check if any ROI was just closed and send AddROI command
                if let Some(roi) = &explorer.pixel_selected.open_roi {
                    if roi.closed {
                        let roi_uuid = uuid::Uuid::new_v4();
                        explorer.rois.insert(roi_uuid.to_string(), roi.clone());
                        send_latest_config(
                            thread_communication,
                            ConfigCommand::AddROI(roi_uuid.to_string(), roi.clone()),
                        );
                        explorer.pixel_selected.open_roi = None;
                    }
                }
            }

            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.label("B/W");
                toggle_ui(ui, &mut explorer.bw);
                ui.add_space(5.0);
                ui.label(format!(
                    "Pixel: {}/{}",
                    explorer.pixel_selected.x, explorer.pixel_selected.y
                ));
                // flip x axis because of image coordinate system
                let pos_x = explorer.data.hk.x_range[0]
                    + explorer.data.hk.dx
                        * (((explorer.data.hk.x_range[1] - explorer.data.hk.x_range[0])
                            / explorer.data.hk.dx) as usize
                            - explorer.pixel_selected.x) as f32;
                let pos_y = explorer.data.hk.y_range[0]
                    + explorer.data.hk.dy * explorer.pixel_selected.y as f32;
                ui.label(format!(" {:.2} mm /{:.2} mm", pos_x, pos_y));
            });

            ui.separator();
            ui.heading("Regions of Interest (ROI)");
            egui::ScrollArea::both().id_salt("rois").show(ui, |ui| {
                egui::Grid::new("rois polygons")
                    .striped(true)
                    .show(ui, |ui| {
                        let mut rois_to_update = HashMap::new();
                        let mut rois_to_delete = HashSet::new();
                        for (roi_uuid, roi) in explorer.rois.iter_mut() {
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
                                rois_to_delete.insert(roi_uuid.clone());
                                send_latest_config(
                                    thread_communication,
                                    ConfigCommand::DeleteROI(roi_uuid.clone()),
                                );
                            }

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
                                rois_to_update.insert(roi_uuid.clone(), roi.clone());
                                send_latest_config(
                                    thread_communication,
                                    ConfigCommand::UpdateROI(roi_uuid.clone(), roi.clone()),
                                );
                            }
                        }

                        for roi in rois_to_delete.iter() {
                            explorer.rois.remove(roi);
                        }

                        for (roi_uuid, roi) in rois_to_update.iter() {
                            explorer.rois.insert(roi_uuid.clone(), roi.clone());
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
            ui.add_space(5.0);
            ui.horizontal(|ui| {
                let text = if thread_communication.gui_settings.meta_data_edit {
                    "Cancel"
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
                        explorer.new_meta_data = vec![("".to_string(), "".to_string())];
                        thread_communication.gui_settings.meta_data_edit = false;
                        thread_communication.gui_settings.meta_data_unlocked = false;
                        send_latest_config(
                            thread_communication,
                            ConfigCommand::LoadMetaData(
                                thread_communication.gui_settings.selected_path.clone(),
                            ),
                        );
                    }
                    if ui
                        .button(egui::RichText::new(format!(
                            "{} Save",
                            egui_phosphor::regular::FLOPPY_DISK
                        )))
                        .clicked()
                    {
                        if thread_communication.gui_settings.meta_data_edit {
                            for (key, val) in explorer.new_meta_data.iter() {
                                if !key.is_empty() && !val.is_empty() {
                                    meta_data.md.insert(key.clone(), val.clone());
                                }
                            }

                            if let Ok(mut md) = thread_communication.md_lock.write() {
                                *md = meta_data.clone();
                            }

                            send_latest_config(
                                thread_communication,
                                ConfigCommand::UpdateMetaData(
                                    thread_communication.gui_settings.selected_path.clone(),
                                ),
                            );
                        }
                        thread_communication.gui_settings.meta_data_edit = false;
                        thread_communication.gui_settings.meta_data_unlocked = false;
                    }
                }
            });
            egui::ScrollArea::both().show(ui, |ui| {
                egui::Grid::new("meta_data")
                    .striped(true)
                    .min_row_height(20.0)
                    .show(ui, |ui| {
                        // this is an aesthetic hack to draw the empty meta-data grid to full width
                        if meta_data.md.is_empty() {
                            ui.label("Attributes:");
                            ui.label(format!("{:75}", " "));
                            ui.end_row();
                        }
                        if thread_communication.gui_settings.meta_data_edit {
                            let mut add_new_metdata = false;
                            if let Some((key, val)) = explorer.new_meta_data.last_mut() {
                                ui.horizontal(|ui| {
                                    ui.horizontal(|ui| {
                                        if key == "" {
                                            ui.disable();
                                        }
                                        if ui
                                            .button(format!("{}", egui_phosphor::regular::PLUS))
                                            .on_hover_text("Add a new Metadata Attribute.")
                                            .clicked()
                                        {
                                            add_new_metdata = true;
                                        }
                                    });
                                    ui.add(
                                        egui::TextEdit::singleline(key)
                                            .interactive(true)
                                            .desired_width(ui.available_width()),
                                    );
                                });
                                ui.add(
                                    egui::TextEdit::singleline(val)
                                        .desired_width(ui.available_width()),
                                );
                            }
                            if add_new_metdata {
                                explorer
                                    .new_meta_data
                                    .push(("".to_string(), "".to_string()));
                            }
                            ui.end_row();

                            let mut keys_to_remove = vec![];

                            for i in 0..explorer.new_meta_data.len() - 1 {
                                ui.horizontal(|ui| {
                                    ui.add(
                                        egui::TextEdit::singleline(
                                            &mut explorer.new_meta_data[i].0,
                                        )
                                        .desired_width(ui.available_width()),
                                    );
                                });
                                ui.horizontal(|ui| {
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
                                        keys_to_remove.push(explorer.new_meta_data[i].0.clone());
                                    }

                                    ui.add(
                                        egui::TextEdit::singleline(
                                            &mut explorer.new_meta_data[i].1,
                                        )
                                        .desired_width(ui.available_width()),
                                    );
                                });
                                ui.end_row();
                            }

                            for key in keys_to_remove.iter() {
                                explorer.new_meta_data.retain(|(k, _)| k != key);
                            }
                        }
                        let mut attributes_to_delete = vec![];
                        for (name, value) in meta_data.md.iter_mut() {
                            ui.label(name);
                            ui.horizontal(|ui| {
                                if thread_communication.gui_settings.meta_data_edit {
                                    ui.add_enabled_ui(
                                        thread_communication.gui_settings.meta_data_unlocked,
                                        |ui| {
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
                                        },
                                    );
                                    let lock =
                                        if thread_communication.gui_settings.meta_data_unlocked {
                                            egui::RichText::new(format!(
                                                "{}",
                                                egui_phosphor::regular::LOCK_OPEN
                                            ))
                                        } else {
                                            egui::RichText::new(format!(
                                                "{}",
                                                egui_phosphor::regular::LOCK
                                            ))
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
