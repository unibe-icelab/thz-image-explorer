//! This module implements the graphical user interface (GUI) for the THz image explorer application.
//!
//! It provides structures and methods to manage user interactions, graphical settings,
//! file operations, and visualization panels for signal and image processing.

use crate::config::ThreadCommunication;
use crate::data_container::PlotDataContainer;
use crate::filters::psf::PSF;
use crate::gui::center_panel::center_panel;
use crate::gui::left_panel::left_panel;
use crate::gui::matrix_plot::{ImageState, SelectedPixel, ROI};
use crate::gui::right_panel::right_panel;
use crate::gui::threed_plot::{CameraInputAllowed, OpacityThreshold, RenderImage, SceneVisibility};
use crate::math_tools::FftWindowType;
use bevy::prelude::*;
use bevy_egui::egui::ThemePreference;
use bevy_egui::{egui, EguiContexts};
use bevy_voxel_plot::InstanceMaterialData;
use core::f64;
use dotthz::DotthzFile;
use egui_file_dialog::information_panel::InformationPanel;
use egui_file_dialog::FileDialog;
use egui_plot::PlotPoint;
use home::home_dir;
use self_update::update::Release;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

/// Represents the state of the file dialog for opening, saving, or working with PSF files.
#[derive(Clone)]
pub enum FileDialogState {
    /// File dialog is set to open a generic file.
    Open,
    /// File dialog is set to open a reference file.
    OpenRef,
    /// File dialog is set to open a PSF file.
    OpenPSF,
    /// File dialog is set to save a file.
    Save,
    /// File dialog is not active.
    None,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum Tab {
    Pulse,
    RefractiveIndex,
    ThreeD,
}

impl Display for Tab {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Tab::Pulse => {
                write!(f, "Pulse")
            }
            Tab::RefractiveIndex => {
                write!(f, "Refractive Index")
            }
            Tab::ThreeD => {
                write!(f, "3D")
            }
        }
    }
}

impl Tab {
    pub fn to_arr(&self) -> [bool; 3] {
        match self {
            Tab::Pulse => [true, false, false],
            Tab::RefractiveIndex => [false, true, false],
            Tab::ThreeD => [false, false, true],
        }
    }
}

/// Contains the GUI settings and user preferences for the application.
///
/// This structure manages settings such as theme mode, visibility preferences for plots,
/// file paths, resolution parameters, and debug options.
///
/// # Fields
/// - `selected_path`: The currently selected file path.
/// - `log_plot`: Whether log scale for plots is enabled.
/// - `down_scaling`: Downscaling factor for visualizations.
/// - `normalize_fft`: Whether FFT results are normalized.
/// - `signal_1_visible`: Visibility of Signal 1.
/// - `avg_signal_1_visible`: Visibility of averaged Signal 1.
/// - `filtered_signal_1_visible`: Visibility of the filtered Signal 1.
/// - `water_lines_visible`: Visibility of water vapor lines.
/// - `phases_visible`: Visibility of phase information.
/// - `frequency_resolution_temp`: Temporary frequency resolution parameter.
/// - `frequency_resolution`: Finalized frequency resolution parameter.
/// - `advanced_settings_window`: Whether advanced settings are open.
/// - `debug`: Debugging mode status.
/// - `dark_mode`: Flag for enabling/disabling dark mode.
/// - `x`, `y`: GUI dimensions.
/// - `theme_preference`: User's theme preference (e.g., dark, light, or system).
/// - `beam_shape`: Coordinates of the beam shape.
/// - `beam_shape_path`: File path for the beam shape data.
/// - `psf`: The point spread function represented as a 2D array.
#[derive(Serialize, Deserialize, PartialEq, Debug, Clone, Resource)]
pub struct GuiSettingsContainer {
    pub reference_index: usize,
    pub sample_index: usize,
    pub selected_path: PathBuf,
    pub log_plot: bool,
    pub down_scaling: usize,
    pub normalize_fft: bool,
    pub avg_in_fourier_space: bool,
    pub water_lines_visible: bool,
    pub phases_visible: bool,
    pub frequency_resolution_temp: f32,
    pub frequency_resolution: f32,
    pub advanced_settings_window: bool,
    pub debug: bool,
    pub dark_mode: bool,
    pub meta_data_edit: bool,
    pub meta_data_unlocked: bool,
    pub x: f32,
    pub y: f32,
    pub tab: Tab,
    pub chart_pitch: f32,
    pub chart_yaw: f32,
    pub chart_scale: f32,
    pub chart_pitch_vel: f32,
    pub chart_yaw_vel: f32,
    pub last_progress_bar_update: HashMap<String, i64>,
    pub progress_bars: HashMap<String, Option<f32>>,
    pub progress_start_time: HashMap<String, i64>,
    pub filter_ui_active: bool,
    pub filter_info: HashMap<String, bool>,
    pub opacity_threshold: f32,
    pub theme_preference: ThemePreference,
    pub beam_shape: Vec<[f64; 2]>,
    pub beam_shape_path: PathBuf,
    pub psf: PSF,
}

impl GuiSettingsContainer {
    /// Creates a new `GuiSettingsContainer` with default values.
    ///
    /// Default values include:
    /// - Dark mode enabled.
    /// - Log plot enabled.
    /// - Advanced settings window disabled.
    /// - Default file paths set to the user's home directory or `/`.
    pub fn new() -> GuiSettingsContainer {
        GuiSettingsContainer {
            reference_index: 0,
            sample_index: 0,
            selected_path: home_dir().unwrap_or_else(|| PathBuf::from("/")),
            log_plot: true,
            down_scaling: 1,
            normalize_fft: false,
            avg_in_fourier_space: true,
            water_lines_visible: false,
            phases_visible: false,
            frequency_resolution_temp: 0.001,
            frequency_resolution: 0.001,
            advanced_settings_window: false,
            debug: true,
            dark_mode: true,
            meta_data_edit: false,
            meta_data_unlocked: false,
            x: 1600.0,
            y: 900.0,
            chart_pitch: 0.3,
            chart_yaw: 0.9,
            chart_scale: 0.9,
            chart_pitch_vel: 0.0,
            chart_yaw_vel: 0.0,
            opacity_threshold: 0.1,
            last_progress_bar_update: HashMap::new(),
            progress_bars: HashMap::new(),
            progress_start_time: HashMap::new(),
            filter_ui_active: true,
            filter_info: HashMap::new(),
            theme_preference: ThemePreference::System,
            beam_shape: vec![],
            beam_shape_path: home_dir().unwrap_or_else(|| PathBuf::from("/")),
            psf: PSF::default(),
            tab: Tab::Pulse,
        }
    }
}

pub fn update_gui(
    mut scene_visibility: ResMut<SceneVisibility>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut query: Query<(&mut InstanceMaterialData, &mut Mesh3d)>,
    cube_preview_image: Res<RenderImage>,
    mut contexts: EguiContexts,
    mut explorer: NonSendMut<THzImageExplorer>,
    mut image_state: Local<ImageState>,
    mut opacity_threshold: ResMut<OpacityThreshold>,
    mut cam_input: ResMut<CameraInputAllowed>,
    mut thread_communication: ResMut<ThreadCommunication>,
) {
    if thread_communication.gui_settings.tab != Tab::ThreeD {
        if let Ok((mut instance_data, _)) = query.single_mut() {
            instance_data.instances.clear();
        }
    }

    scene_visibility.0 = thread_communication.gui_settings.tab == Tab::ThreeD;

    let cube_preview_texture_id = contexts.image_id(&cube_preview_image).unwrap();

    let ctx = contexts.ctx_mut();

    let left_panel_width = 300.0;
    let right_panel_width = 500.0;

    center_panel(
        &mut meshes,
        &mut query,
        &cube_preview_texture_id,
        &ctx,
        &right_panel_width,
        &left_panel_width,
        &mut explorer,
        &mut opacity_threshold,
        &mut cam_input,
        &mut thread_communication,
    );

    left_panel(
        ctx,
        &mut explorer,
        &left_panel_width,
        &mut image_state,
        &mut thread_communication,
    );

    right_panel(
        ctx,
        &mut explorer,
        &right_panel_width,
        &mut thread_communication,
    );

    thread::sleep(Duration::from_secs_f64(1.0 / 30.0));

    thread_communication.gui_settings.x = ctx.used_size().x;
    thread_communication.gui_settings.y = ctx.used_size().y;
}

/// Main application struct for the THz Image Explorer GUI.
///
/// This struct manages application state, handles GUI components, file dialogs,
/// and communication with other threads. It contains configuration options for
/// visualizations and includes methods for rendering the main GUI panels.
///
/// # Fields
/// - `fft_bounds`: Bounds for the FFT visualization.
/// - `fft_window_type`: Selected FFT window type.
/// - `filter_bounds`: Bounds for filter application.
/// - `time_window`: Time window for analysis.
/// - `pixel_selected`: Struct representing the currently selected pixel in the image.
/// - `val`: Coordinates of the current plot point.
/// - `mid_point`: Midpoint value for certain calculations.
/// - `bw`: Boolean for enabling black-and-white mode.
/// - `water_vapour_lines`: Preloaded water vapor line frequencies for reference plots.
/// - `wp`: An image object used in the right panel.
/// - `data`: Contains the experiment or scan data.
/// - `file_dialog_state`: Current state of the file dialog.
/// - `file_dialog`: Instance of the file dialog used for file operations.
/// - `information_panel`: Instance of the file information panel.
/// - `other_files`: List of other detected files for the file dialog.
/// - `selected_file_name`: Name of the currently selected file.
/// - `scroll_to_selection`: Boolean to indicate whether to scroll to the file selection.
/// - `thread_communication`: Shared thread communication object for GUI interactions.
/// - `settings_window_open`: Boolean indicating whether the settings window is open.
/// - `update_text`: Text displayed for updates.
/// - `new_release`: Optional field for new software updates (only used with "self_update" feature).
pub struct THzImageExplorer {
    pub(crate) cut_off: [f32; 2],
    pub(crate) fft_bounds: [f32; 2],
    pub(crate) fft_window_type: FftWindowType,
    pub(crate) pixel_selected: SelectedPixel,
    pub(crate) val: PlotPoint,
    pub(crate) mid_point: f32,
    pub(crate) bw: bool,
    pub(crate) water_vapour_lines: Vec<f64>,
    pub(crate) wp: &'static [u8],
    pub(crate) data: PlotDataContainer,
    pub(crate) file_dialog_state: FileDialogState,
    pub(crate) file_dialog: FileDialog,
    pub(crate) information_panel: InformationPanel,
    pub(crate) other_files: Vec<PathBuf>,
    pub(crate) selected_file_name: String,
    pub(crate) scroll_to_selection: bool,
    pub(crate) settings_window_open: bool,
    pub(crate) update_text: String,
    #[cfg(feature = "self_update")]
    pub(crate) new_release: Option<Release>,
    pub(crate) rois: HashMap<String, ROI>,
}

impl THzImageExplorer {
    /// Creates a new instance of `THzImageExplorer`.
    ///
    /// # Arguments
    /// - `cc`: The creation context for the application instance.
    /// - `thread_communication`: An instance of `ThreadCommunication` for managing
    ///   threaded GUI settings.
    ///
    /// # Returns
    /// A new `THzImageExplorer` struct with default settings and preloaded water vapor lines.
    #[allow(clippy::too_many_arguments)]
    pub fn new(thread_communication: ThreadCommunication) -> Self {
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
            //.default_file_filter("dotTHz files")
            ;
        // TODO: fix this!!
        // // Load the persistent data of the file dialog.
        // // Alternatively, you can also use the `FileDialog::storage` builder method.
        // if let Some(storage) = cc.storage {
        //     *file_dialog.storage_mut() =
        //         eframe::get_value(storage, "file_dialog_storage").unwrap_or_default()
        // }

        Self {
            water_vapour_lines,
            wp: include_bytes!("../../images/WP-Logo.png"),
            data: PlotDataContainer::default(),
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
                    if let Ok(file) = DotthzFile::open(&path.to_path_buf()) {
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
            cut_off: [0.0, 100.0],
            fft_bounds: [1.0, 7.0],
            fft_window_type: FftWindowType::AdaptedBlackman,
            pixel_selected: SelectedPixel::default(),
            val: PlotPoint { x: 0.0, y: 0.0 },
            mid_point: 50.0,
            bw: false,
            settings_window_open: false,
            update_text: "".to_string(),
            #[cfg(feature = "self_update")]
            new_release: None,
            rois: HashMap::new(),
        }
    }
}
