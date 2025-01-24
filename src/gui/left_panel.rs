use crate::config::Config;
use crate::gauge::gauge;
use crate::gui::application::{FileDialogState, GuiSettingsContainer};
use crate::io::find_files_with_same_extension;
use crate::gui::matrix_plot::{make_dummy, plot_matrix, SelectedPixel};
use crate::toggle::toggle_ui;
use crate::DataPoint;
use dotthz::DotthzMetaData;
use eframe::egui;
use eframe::egui::panel::Side;
use eframe::egui::TextStyle;
use egui_extras::{Column, TableBuilder};
use egui_file_dialog::information_panel::InformationPanel;
use egui_file_dialog::FileDialog;
use egui_plot::PlotPoint;
use ndarray::Array2;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::sync::{Arc, RwLock};

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
    gui_conf: &mut GuiSettingsContainer,
    left_panel_width: &f32,
    pixel_selected: &mut SelectedPixel,
    val: &mut PlotPoint,
    mid_point: &mut f32,
    bw: &mut bool,
    file_dialog_state: &mut FileDialogState,
    file_dialog: &mut FileDialog,
    information_panel: &mut InformationPanel,
    other_files: &mut Vec<PathBuf>,
    selected_file_name: &mut String,
    scroll_to_selection: &mut bool,
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
    let mut meta_data = DotthzMetaData::default();
    if let Ok(md) = md_lock.read() {
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
                                config_tx
                                    .send(Config::OpenFile(item.to_path_buf()))
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
                        config_tx
                            .send(Config::OpenFile(item.to_path_buf()))
                            .expect("unable to send open file cmd");
                        *scroll_to_selection = true;
                    }
                    if ui.input(|i| i.key_pressed(egui::Key::ArrowUp)) && selected_index > 0 {
                        let item = other_files[selected_index - 1].clone();
                        *selected_file_name =
                            item.file_name().unwrap().to_str().unwrap().to_string();
                        config_tx
                            .send(Config::OpenFile(item.to_path_buf()))
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
                    config_tx
                        .send(Config::OpenFile(path))
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
                        config_tx
                            .send(Config::OpenFile(path.to_path_buf()))
                            .expect("unable to send open file cmd");
                    }
                    if let Some(path) = file_dialog.take_picked() {
                        *other_files = find_files_with_same_extension(&path).unwrap();
                        *selected_file_name =
                            path.file_name().unwrap().to_str().unwrap().to_string();
                        *scroll_to_selection = true;
                        file_dialog.config_mut().initial_directory = path.clone();
                        gui_conf.selected_path = path;
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

            ui.separator();
            ui.heading("Meta Data");
            egui::ScrollArea::both().show(ui, |ui| {
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
        });
}
