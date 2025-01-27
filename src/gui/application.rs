use core::f64;
use dotthz::DotthzFile;
use egui_file_dialog::information_panel::InformationPanel;
use egui_file_dialog::FileDialog;
use std::path::PathBuf;
use std::sync::Arc;

use eframe::egui::ThemePreference;
use eframe::{egui, Storage};
use egui_plot::PlotPoint;
use home::home_dir;
use preferences::Preferences;
use self_update::update::Release;
use serde::{Deserialize, Serialize};

use crate::config::GuiThreadCommunication;
use crate::data_container::DataPoint;
use crate::gui::center_panel::center_panel;
use crate::gui::left_panel::left_panel;
use crate::gui::matrix_plot::SelectedPixel;
use crate::gui::right_panel::right_panel;
use crate::APP_INFO;
use crate::math_tools::FftWindowType;

#[derive(Clone)]
pub enum FileDialogState {
    Open,
    Save,
    None,
}
#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct GuiSettingsContainer {
    pub selected_path: PathBuf,
    pub log_plot: bool,
    pub down_scaling: usize,
    pub normalize_fft: bool,
    pub signal_1_visible: bool,
    pub avg_signal_1_visible: bool,
    pub filtered_signal_1_visible: bool,
    pub water_lines_visible: bool,
    pub phases_visible: bool,
    pub frequency_resolution_temp: f32,
    pub frequency_resolution: f32,
    pub advanced_settings_window: bool,
    pub debug: bool,
    pub dark_mode: bool,
    pub x: f32,
    pub y: f32,
    pub theme_preference: ThemePreference,
}

impl GuiSettingsContainer {
    pub fn new() -> GuiSettingsContainer {
        GuiSettingsContainer {
            selected_path: home_dir().unwrap_or_else(|| PathBuf::from("/")),
            log_plot: true,
            down_scaling: 1,
            normalize_fft: false,
            signal_1_visible: true,
            avg_signal_1_visible: false,
            filtered_signal_1_visible: false,
            water_lines_visible: false,
            phases_visible: false,
            frequency_resolution_temp: 0.001,
            frequency_resolution: 0.001,
            advanced_settings_window: false,
            debug: true,
            dark_mode: true,
            x: 1600.0,
            y: 900.0,
            theme_preference: ThemePreference::System,
        }
    }
}

pub struct THzImageExplorer<'a> {
    fft_bounds: [f32; 2],
    fft_window_type: FftWindowType,
    filter_bounds: [f32; 2],
    time_window: [f32; 2],
    pixel_selected: SelectedPixel,
    val: PlotPoint,
    mid_point: f32,
    bw: bool,
    water_vapour_lines: Vec<f64>,
    wp: egui::Image<'a>,
    data: DataPoint,
    file_dialog_state: FileDialogState,
    file_dialog: FileDialog,
    information_panel: InformationPanel,
    other_files: Vec<PathBuf>,
    selected_file_name: String,
    scroll_to_selection: bool,
    thread_communication: GuiThreadCommunication,
    settings_window_open: bool,
    update_text: String,
    #[cfg(feature = "self_update")]
    new_release: Option<Release>,
}

impl THzImageExplorer<'_> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(cc: &eframe::CreationContext, thread_communication: GuiThreadCommunication) -> Self {
        let mut water_vapour_lines: Vec<f64> = Vec::new();
        let buffered = include_str!("../../resources/water_lines.csv");
        for line in buffered.lines() {
            water_vapour_lines.push(line.trim().parse().unwrap());
        }

        let mut file_dialog = FileDialog::default()
            //.initial_directory(PathBuf::from("/path/to/app"))
            .default_file_name("measurement.thz")
            .default_size([600.0, 400.0])
            // .add_quick_access("Project", |s| {
            //     s.add_path("☆  Examples", "examples");
            //     s.add_path("📷  Media", "media");
            //     s.add_path("📂  Source", "src");
            // })
            .set_file_icon(
                "🖹",
                Arc::new(|path| path.extension().unwrap_or_default().to_ascii_lowercase() == "md"),
            )
            .set_file_icon(
                "",
                Arc::new(|path| {
                    path.file_name().unwrap_or_default().to_ascii_lowercase() == ".gitignore"
                }),
            )
            .add_file_filter(
                "dotTHz files",
                Arc::new(|p| p.extension().unwrap_or_default().to_ascii_lowercase() == "thz"),
            )
            .add_file_filter(
                "npy files",
                Arc::new(|p| p.extension().unwrap_or_default().to_ascii_lowercase() == "npy"),
            )
            .add_file_filter(
                "npz files",
                Arc::new(|p| p.extension().unwrap_or_default().to_ascii_lowercase() == "npz"),
            )
            .add_file_filter(
                "CSV files",
                Arc::new(|p| p.extension().unwrap_or_default().to_ascii_lowercase() == "csv"),
            )
            .initial_directory(thread_communication.gui_settings.selected_path.clone())
            .default_file_filter("dotTHz files");
        // Load the persistent data of the file dialog.
        // Alternatively, you can also use the `FileDialog::storage` builder method.
        if let Some(storage) = cc.storage {
            *file_dialog.storage_mut() =
                eframe::get_value(storage, "file_dialog_storage").unwrap_or_default()
        }

        Self {
            water_vapour_lines,
            wp: egui::Image::from_bytes("WP", include_bytes!("../../images/WP-Logo.png")),
            data: DataPoint::default(),
            file_dialog_state: FileDialogState::None,
            file_dialog,
            information_panel: InformationPanel::default()
                .add_file_preview("csv", |ui, item| {
                    ui.label("CSV preview:");
                    if let Some(content) = item.content() {
                        egui::ScrollArea::vertical()
                            .max_height(150.0)
                            .show(ui, |ui| {
                                ui.add(
                                    egui::TextEdit::multiline(&mut content.to_string())
                                        .code_editor(),
                                );
                            });
                    }
                })
                .add_metadata_loader("thz", |other_data, path| {
                    if let Ok(file) = DotthzFile::load(&path.to_path_buf()) {
                        if let Ok(group_names) = file.get_group_names() {
                            other_data
                                .insert("Groups: ".to_string(), group_names.join(", ").to_string());
                            if let Some(group_name) = group_names.first() {
                                if let Ok(meta_data) = file.get_meta_data(group_name) {
                                    other_data.insert(
                                        "Description".to_string(),
                                        meta_data.description.clone(),
                                    );
                                    for (name, md) in meta_data.md.clone() {
                                        other_data.insert(name, md);
                                    }
                                    other_data.insert("Mode".to_string(), meta_data.mode.clone());
                                    other_data
                                        .insert("Version".to_string(), meta_data.version.clone());
                                    other_data.insert(
                                        "Instrument".to_string(),
                                        meta_data.instrument.clone(),
                                    );
                                }
                            }
                        }
                    }
                }),
            other_files: vec![],
            selected_file_name: "".to_string(),
            scroll_to_selection: false,
            fft_bounds: [1.0, 7.0],
            fft_window_type: FftWindowType::AdaptedBlackman,
            filter_bounds: [0.0, 10.0],
            time_window: [1000.0, 1050.0],
            pixel_selected: SelectedPixel::default(),
            val: PlotPoint { x: 0.0, y: 0.0 },
            mid_point: 50.0,
            bw: false,
            thread_communication,
            settings_window_open: false,
            update_text: "".to_string(),
            #[cfg(feature = "self_update")]
            new_release: None,
        }
    }
}

impl eframe::App for THzImageExplorer<'_> {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let left_panel_width = 300.0;
        let right_panel_width = 500.0;

        center_panel(
            ctx,
            &right_panel_width,
            &left_panel_width,
            &mut self.thread_communication,
            &mut self.data,
            &self.water_vapour_lines,
        );

        left_panel(
            ctx,
            &mut self.thread_communication,
            &left_panel_width,
            &mut self.pixel_selected,
            &mut self.val,
            &mut self.mid_point,
            &mut self.bw,
            &mut self.file_dialog_state,
            &mut self.file_dialog,
            &mut self.information_panel,
            &mut self.other_files,
            &mut self.selected_file_name,
            &mut self.scroll_to_selection,
        );

        right_panel(
            ctx,
            &mut self.settings_window_open,
            &mut self.update_text,
            &right_panel_width,
            &mut self.thread_communication,
            &mut self.filter_bounds,
            &mut self.fft_bounds,
            &mut self.fft_window_type,
            &mut self.time_window,
            &mut self.pixel_selected,
            self.wp.clone(),
            #[cfg(feature = "self_update")]
            &mut self.new_release,
        );

        self.thread_communication.gui_settings.x = ctx.used_size().x;
        self.thread_communication.gui_settings.y = ctx.used_size().y;
    }

    fn save(&mut self, storage: &mut dyn Storage) {
        let prefs_key = "config/gui";
        match self
            .thread_communication
            .gui_settings
            .save(&APP_INFO, prefs_key)
        {
            Ok(_) => {}
            Err(err) => {
                log::error!("error saving gui_conf: {err:?}");
            }
        }
        // Save the persistent data of the file dialog
        eframe::set_value(
            storage,
            "file_dialog_storage",
            self.file_dialog.storage_mut(),
        );
    }
}
