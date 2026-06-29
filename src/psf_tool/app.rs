//! Main PSF tool application state and UI, adapted from thz-point-spread-function-tool.
//! Instead of implementing eframe::App, this exposes `show_ui()` to be called from a
//! bevy_egui system running on the secondary window's EguiContextPass schedule.

use bevy_egui::egui;
#[cfg(not(target_os = "macos"))]
use egui_file_dialog::FileDialog;
use egui_plot::{Legend, Line, Plot, PlotPoints};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;

use super::curve_fitting::CurveFits;
use super::data_loader::{
    load_knife_edge_measurements, split_and_flip_measurement, KnifeEdgeMeasurement,
};
use super::diagnostic_window::DiagnosticWindow;
use super::diagnostics::DiagnosticResults;
use super::export;
use super::filters::{create_filters, frequency_response, FilterParams, Filters, FrequencySpacing};
use super::fitting::{fit_beam_widths, fit_mean_beam, BeamFitParams, BeamWidthFits, MeanBeamFit};
use super::individual_fits_window::IndividualFitsWindow;
use super::psf_visualizer::PsfVisualizerWindow;
use super::warnings::{check_transition_width, WarningType};

// ─── Persistence ─────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
struct AppState {
    knife_edge_x_path: String,
    knife_edge_y_path: String,
    filter_params: FilterParams,
    fit_params: BeamFitParams,
    show_filter_response: bool,
    show_intensity: bool,
    show_beam_widths: bool,
    show_beam_centers: bool,
    use_wavelength: bool,
}

impl AppState {
    fn save(&self) {
        if let Ok(json) = serde_json::to_string_pretty(self) {
            if let Ok(path) = Self::config_path() {
                let _ = std::fs::create_dir_all(path.parent().unwrap());
                let _ = std::fs::write(path, json);
            }
        }
    }

    fn load() -> Option<Self> {
        let path = Self::config_path().ok()?;
        let json = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&json).ok()
    }

    fn config_path() -> anyhow::Result<PathBuf> {
        let config_dir =
            dirs::config_dir().ok_or_else(|| anyhow::anyhow!("No config directory"))?;
        Ok(config_dir
            .join("thz_image_explorer")
            .join("psf_tool_state.json"))
    }
}

// ─── Computation types ───────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum ComputationState {
    Idle,
    Computing {
        progress_x: Option<AxisProgress>,
        progress_y: Option<AxisProgress>,
    },
    Complete,
    Error(String),
}

#[derive(Debug, Clone)]
pub struct AxisProgress {
    phase: ComputationPhase,
    current_filter: usize,
    total_filters: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ComputationPhase {
    Loading,
    Processing,
    Fitting,
}

enum ComputeMessage {
    Start {
        x_path: Option<PathBuf>,
        y_path: Option<PathBuf>,
        filter_params: FilterParams,
        fit_params: BeamFitParams,
        cancel_flag: Arc<AtomicBool>,
    },
}

enum ComputeResult {
    ProgressX {
        phase: ComputationPhase,
        current_filter: usize,
        total_filters: usize,
    },
    ProgressY {
        phase: ComputationPhase,
        current_filter: usize,
        total_filters: usize,
    },
    Complete {
        x_measurement: Option<Arc<KnifeEdgeMeasurement>>,
        y_measurement: Option<Arc<KnifeEdgeMeasurement>>,
        filters: Arc<Filters>,
        mean_fit_x: Option<MeanBeamFit>,
        mean_fit_y: Option<MeanBeamFit>,
        beam_fits_x: Option<BeamWidthFits>,
        beam_fits_y: Option<BeamWidthFits>,
        warnings: Vec<WarningType>,
    },
    Error(String),
}

enum DialogResult {
    XFile(Option<PathBuf>),
    YFile(Option<PathBuf>),
    ExportFile(Option<PathBuf>),
}

#[cfg(not(target_os = "macos"))]
#[derive(Clone, Copy, PartialEq, Eq)]
enum PsfDialogState {
    None,
    OpenX,
    OpenY,
    SaveExport,
}

#[derive(Debug, Clone)]
struct FilterResponseCache {
    curves_hz: Vec<Vec<[f64; 2]>>,
    curves_wavelength_um: Vec<Vec<[f64; 2]>>,
}

// ─── Main app state ──────────────────────────────────────────────────────────

pub struct ThzPsfApp {
    // File paths
    pub knife_edge_x_path: String,
    pub knife_edge_y_path: String,

    // Parameters
    pub filter_params: FilterParams,
    pub fit_params: BeamFitParams,
    last_params_hash: u64,

    // Data
    x_measurement: Option<Arc<KnifeEdgeMeasurement>>,
    y_measurement: Option<Arc<KnifeEdgeMeasurement>>,
    pub filters: Option<Arc<Filters>>,
    mean_fit_x: Option<MeanBeamFit>,
    mean_fit_y: Option<MeanBeamFit>,
    pub beam_fits_x: Option<BeamWidthFits>,
    pub beam_fits_y: Option<BeamWidthFits>,
    pub curve_fits: Option<CurveFits>,
    filter_response_cache: Option<FilterResponseCache>,

    // Computation
    computation_state: ComputationState,
    cancel_flag: Arc<AtomicBool>,
    compute_tx: Option<Sender<ComputeMessage>>,
    result_rx: Option<Receiver<ComputeResult>>,
    dialog_tx: Sender<DialogResult>,
    dialog_rx: Receiver<DialogResult>,
    #[cfg(not(target_os = "macos"))]
    file_dialog: FileDialog,
    #[cfg(not(target_os = "macos"))]
    file_dialog_state: PsfDialogState,

    // Display options
    show_filter_response: bool,
    show_intensity: bool,
    show_beam_widths: bool,
    show_beam_centers: bool,
    use_wavelength: bool,

    // Sub-windows (egui floating windows)
    pub diagnostics_window: Option<DiagnosticWindow>,
    pub show_diagnostics: bool,
    pub psf_visualizer_window: Option<PsfVisualizerWindow>,
    pub show_psf_visualizer: bool,
    pub individual_fits_window: Option<IndividualFitsWindow>,
    pub show_individual_fits: bool,

    // Warnings
    active_warnings: Vec<WarningType>,
    /// Inline status messages shown in the UI instead of toast notifications.
    status_message: Option<String>,

    control_panel_width: f32,

    /// Set to true by the bevy system when the window is first shown; cleared on next frame.
    pub first_show: bool,

    /// Set to true when the user clicks "Use for deconvolution"; consumed by `psf_tool_system`.
    pub apply_to_deconv_requested: bool,
    /// True once the current curve_fits has been applied to the main app.
    pub psf_applied: bool,
}

impl Default for ThzPsfApp {
    fn default() -> Self {
        let (dialog_tx, dialog_rx) = channel();
        Self {
            knife_edge_x_path: String::new(),
            knife_edge_y_path: String::new(),
            filter_params: FilterParams::default(),
            fit_params: BeamFitParams::default(),
            last_params_hash: 0,
            x_measurement: None,
            y_measurement: None,
            filters: None,
            mean_fit_x: None,
            mean_fit_y: None,
            beam_fits_x: None,
            beam_fits_y: None,
            curve_fits: None,
            filter_response_cache: None,
            computation_state: ComputationState::Idle,
            cancel_flag: Arc::new(AtomicBool::new(false)),
            compute_tx: None,
            result_rx: None,
            dialog_tx,
            dialog_rx,
            #[cfg(not(target_os = "macos"))]
            file_dialog: FileDialog::default()
                .default_size([640.0, 440.0])
                .add_file_filter(
                    "THz files",
                    std::sync::Arc::new(|p| {
                        p.extension().unwrap_or_default().to_ascii_lowercase() == "thz"
                    }),
                )
                .add_file_filter(
                    "NumPy archive",
                    std::sync::Arc::new(|p| {
                        p.extension().unwrap_or_default().to_ascii_lowercase() == "npz"
                    }),
                ),
            #[cfg(not(target_os = "macos"))]
            file_dialog_state: PsfDialogState::None,
            show_filter_response: true,
            show_intensity: true,
            show_beam_widths: true,
            show_beam_centers: true,
            use_wavelength: false,
            diagnostics_window: None,
            show_diagnostics: false,
            psf_visualizer_window: None,
            show_psf_visualizer: false,
            individual_fits_window: None,
            show_individual_fits: false,
            active_warnings: Vec::new(),
            status_message: None,
            control_panel_width: 350.0,
            first_show: true,
            apply_to_deconv_requested: false,
            psf_applied: false,
        }
    }
}

impl ThzPsfApp {
    pub fn new() -> Self {
        if let Some(state) = AppState::load() {
            Self {
                knife_edge_x_path: state.knife_edge_x_path,
                knife_edge_y_path: state.knife_edge_y_path,
                filter_params: state.filter_params,
                fit_params: state.fit_params,
                show_filter_response: state.show_filter_response,
                show_intensity: state.show_intensity,
                show_beam_widths: state.show_beam_widths,
                show_beam_centers: state.show_beam_centers,
                use_wavelength: state.use_wavelength,
                ..Default::default()
            }
        } else {
            Self::default()
        }
    }

    fn save_state(&self) {
        AppState {
            knife_edge_x_path: self.knife_edge_x_path.clone(),
            knife_edge_y_path: self.knife_edge_y_path.clone(),
            filter_params: self.filter_params.clone(),
            fit_params: self.fit_params.clone(),
            show_filter_response: self.show_filter_response,
            show_intensity: self.show_intensity,
            show_beam_widths: self.show_beam_widths,
            show_beam_centers: self.show_beam_centers,
            use_wavelength: self.use_wavelength,
        }
        .save();
    }

    /// Reset all parameters and display options to their default values.
    pub fn reset_parameters(&mut self) {
        self.filter_params = FilterParams::default();
        self.fit_params = BeamFitParams::default();
        self.show_filter_response = true;
        self.show_intensity = true;
        self.show_beam_widths = true;
        self.show_beam_centers = true;
        self.use_wavelength = false;
        self.x_measurement = None;
        self.y_measurement = None;
        self.filters = None;
        self.mean_fit_x = None;
        self.mean_fit_y = None;
        self.beam_fits_x = None;
        self.beam_fits_y = None;
        self.curve_fits = None;
        self.filter_response_cache = None;
        self.computation_state = ComputationState::Idle;
        self.cancel_flag = Arc::new(AtomicBool::new(false));
        self.last_params_hash = 0;
        self.active_warnings.clear();
        self.status_message = None;
        self.save_state();
    }

    fn compute_params_hash(&self) -> u64 {
        let mut h = DefaultHasher::new();
        self.knife_edge_x_path.hash(&mut h);
        self.knife_edge_y_path.hash(&mut h);
        self.filter_params.n_filters.hash(&mut h);
        self.filter_params.low_cut.to_bits().hash(&mut h);
        self.filter_params.high_cut.to_bits().hash(&mut h);
        self.filter_params.start_freq.to_bits().hash(&mut h);
        self.filter_params.end_freq.to_bits().hash(&mut h);
        self.filter_params.win_width.to_bits().hash(&mut h);
        self.filter_params.frequency_spacing.hash(&mut h);
        self.fit_params.w_max.to_bits().hash(&mut h);
        self.fit_params.use_monotonicity_constraint.hash(&mut h);
        h.finish()
    }

    fn should_compute(&self) -> bool {
        let has_data = !self.knife_edge_x_path.is_empty() || !self.knife_edge_y_path.is_empty();
        let can_compute = matches!(
            self.computation_state,
            ComputationState::Idle | ComputationState::Complete | ComputationState::Error(_)
        );
        has_data && can_compute
    }

    fn start_computation(&mut self) {
        self.active_warnings.clear();
        self.status_message = None;
        self.cancel_flag.store(true, Ordering::Relaxed);
        self.cancel_flag = Arc::new(AtomicBool::new(false));

        let x_path =
            (!self.knife_edge_x_path.is_empty()).then(|| PathBuf::from(&self.knife_edge_x_path));
        let y_path =
            (!self.knife_edge_y_path.is_empty()).then(|| PathBuf::from(&self.knife_edge_y_path));

        let has_x = x_path.is_some();
        let has_y = y_path.is_some();

        if self.compute_tx.is_none() || self.result_rx.is_none() {
            let (tx, rx) = channel();
            let (result_tx, result_rx) = channel();
            self.compute_tx = Some(tx);
            self.result_rx = Some(result_rx);
            std::thread::spawn(move || Self::compute_psf(rx, result_tx));
        }

        if let Some(tx) = &self.compute_tx {
            tx.send(ComputeMessage::Start {
                x_path,
                y_path,
                filter_params: self.filter_params.clone(),
                fit_params: self.fit_params.clone(),
                cancel_flag: Arc::clone(&self.cancel_flag),
            })
            .ok();
        }

        self.last_params_hash = self.compute_params_hash();
        self.computation_state = ComputationState::Computing {
            progress_x: has_x.then(|| AxisProgress {
                phase: ComputationPhase::Loading,
                current_filter: 0,
                total_filters: self.filter_params.n_filters,
            }),
            progress_y: has_y.then(|| AxisProgress {
                phase: ComputationPhase::Loading,
                current_filter: 0,
                total_filters: self.filter_params.n_filters,
            }),
        };
    }

    fn compute_psf(rx: Receiver<ComputeMessage>, result_tx: Sender<ComputeResult>) {
        while let Ok(msg) = rx.recv() {
            match msg {
                ComputeMessage::Start {
                    x_path,
                    y_path,
                    filter_params,
                    fit_params,
                    cancel_flag,
                } => {
                    if x_path.is_none() && y_path.is_none() {
                        result_tx
                            .send(ComputeResult::Error("No files selected".to_string()))
                            .ok();
                        continue;
                    }

                    if x_path.is_some() {
                        result_tx
                            .send(ComputeResult::ProgressX {
                                phase: ComputationPhase::Loading,
                                current_filter: 0,
                                total_filters: filter_params.n_filters,
                            })
                            .ok();
                    }
                    if y_path.is_some() {
                        result_tx
                            .send(ComputeResult::ProgressY {
                                phase: ComputationPhase::Loading,
                                current_filter: 0,
                                total_filters: filter_params.n_filters,
                            })
                            .ok();
                    }

                    if cancel_flag.load(Ordering::Relaxed) {
                        continue;
                    }

                    // Load measurements
                    let (x_meas_raw, y_meas_raw) = if x_path.is_some() && y_path.is_some() {
                        match load_knife_edge_measurements(
                            x_path.as_ref().unwrap(),
                            y_path.as_ref().unwrap(),
                        ) {
                            Ok(m) => (Some(m.0), Some(m.1)),
                            Err(e) => {
                                result_tx
                                    .send(ComputeResult::Error(format!(
                                        "Failed to load data: {}",
                                        e
                                    )))
                                    .ok();
                                continue;
                            }
                        }
                    } else if let Some(ref path) = x_path {
                        match load_knife_edge_measurements(path, path) {
                            Ok(m) => (Some(m.0), None),
                            Err(e) => {
                                result_tx
                                    .send(ComputeResult::Error(format!(
                                        "Failed to load X data: {}",
                                        e
                                    )))
                                    .ok();
                                continue;
                            }
                        }
                    } else if let Some(ref path) = y_path {
                        match load_knife_edge_measurements(path, path) {
                            Ok(m) => (None, Some(m.1)),
                            Err(e) => {
                                result_tx
                                    .send(ComputeResult::Error(format!(
                                        "Failed to load Y data: {}",
                                        e
                                    )))
                                    .ok();
                                continue;
                            }
                        }
                    } else {
                        (None, None)
                    };

                    if cancel_flag.load(Ordering::Relaxed) {
                        continue;
                    }

                    if x_path.is_some() {
                        result_tx
                            .send(ComputeResult::ProgressX {
                                phase: ComputationPhase::Processing,
                                current_filter: 0,
                                total_filters: filter_params.n_filters,
                            })
                            .ok();
                    }
                    if y_path.is_some() {
                        result_tx
                            .send(ComputeResult::ProgressY {
                                phase: ComputationPhase::Processing,
                                current_filter: 0,
                                total_filters: filter_params.n_filters,
                            })
                            .ok();
                    }

                    let times = if let Some(ref x) = x_meas_raw {
                        &x.times
                    } else if let Some(ref y) = y_meas_raw {
                        &y.times
                    } else {
                        result_tx
                            .send(ComputeResult::Error("No data available".to_string()))
                            .ok();
                        continue;
                    };

                    let filters = Arc::new(create_filters(&filter_params, times));
                    let n_filters = filter_params.n_filters;

                    if cancel_flag.load(Ordering::Relaxed) {
                        continue;
                    }

                    let results: Vec<_> = vec![x_meas_raw, y_meas_raw]
                        .into_par_iter()
                        .enumerate()
                        .map(|(axis_idx, meas_opt)| {
                            let meas_raw = meas_opt?;
                            let (left, right) = split_and_flip_measurement(&meas_raw);
                            let progress_counter = Arc::new(AtomicUsize::new(0));

                            let (result_left, result_right) = rayon::join(
                                || {
                                    let mean_fit = fit_mean_beam(
                                        &left.positions,
                                        &left.positions,
                                        &left.time_traces,
                                        &left.time_traces,
                                    )
                                    .ok()?;
                                    if cancel_flag.load(Ordering::Relaxed) {
                                        return None;
                                    }
                                    let pc = Arc::clone(&progress_counter);
                                    let beam_fits = fit_beam_widths(
                                        &mean_fit,
                                        &left.positions,
                                        &left.positions,
                                        &left.time_traces,
                                        &left.time_traces,
                                        &filters.coefficients,
                                        &fit_params,
                                        |_cur, tot| {
                                            let p = pc.fetch_add(1, Ordering::Relaxed) + 1;
                                            let msg = if axis_idx == 0 {
                                                ComputeResult::ProgressX {
                                                    phase: ComputationPhase::Fitting,
                                                    current_filter: p,
                                                    total_filters: tot * 2,
                                                }
                                            } else {
                                                ComputeResult::ProgressY {
                                                    phase: ComputationPhase::Fitting,
                                                    current_filter: p,
                                                    total_filters: tot * 2,
                                                }
                                            };
                                            result_tx.send(msg).ok();
                                            !cancel_flag.load(Ordering::Relaxed)
                                        },
                                    )
                                    .ok()?;
                                    Some((mean_fit, beam_fits))
                                },
                                || {
                                    let mean_fit = fit_mean_beam(
                                        &right.positions,
                                        &right.positions,
                                        &right.time_traces,
                                        &right.time_traces,
                                    )
                                    .ok()?;
                                    if cancel_flag.load(Ordering::Relaxed) {
                                        return None;
                                    }
                                    let pc = Arc::clone(&progress_counter);
                                    let beam_fits = fit_beam_widths(
                                        &mean_fit,
                                        &right.positions,
                                        &right.positions,
                                        &right.time_traces,
                                        &right.time_traces,
                                        &filters.coefficients,
                                        &fit_params,
                                        |_cur, tot| {
                                            let p = pc.fetch_add(1, Ordering::Relaxed) + 1;
                                            let msg = if axis_idx == 0 {
                                                ComputeResult::ProgressX {
                                                    phase: ComputationPhase::Fitting,
                                                    current_filter: p,
                                                    total_filters: tot * 2,
                                                }
                                            } else {
                                                ComputeResult::ProgressY {
                                                    phase: ComputationPhase::Fitting,
                                                    current_filter: p,
                                                    total_filters: tot * 2,
                                                }
                                            };
                                            result_tx.send(msg).ok();
                                            !cancel_flag.load(Ordering::Relaxed)
                                        },
                                    )
                                    .ok()?;
                                    Some((mean_fit, beam_fits))
                                },
                            );

                            if cancel_flag.load(Ordering::Relaxed) {
                                return None;
                            }

                            let (mean_fit_left, beam_fits_left) = result_left?;
                            let (mean_fit_right, beam_fits_right) = result_right?;

                            // Average left and right results
                            let mut popt_avg = beam_fits_left.popt_xs.clone();
                            for i in 0..n_filters {
                                popt_avg[[i, 0]] = ((-beam_fits_left.popt_xs[[i, 0]])
                                    + beam_fits_right.popt_xs[[i, 0]])
                                    / 2.0;
                                popt_avg[[i, 1]] = (beam_fits_left.popt_xs[[i, 1]]
                                    + beam_fits_right.popt_xs[[i, 1]])
                                    / 2.0;
                            }
                            let mean_pos = (0..n_filters).map(|i| popt_avg[[i, 0]]).sum::<f64>()
                                / n_filters as f64;
                            for i in 0..n_filters {
                                popt_avg[[i, 0]] -= mean_pos;
                            }

                            let filtered_traces_x_avg: Vec<_> = (0..n_filters)
                                .map(|i| {
                                    (&beam_fits_left.filtered_traces_x[i]
                                        + &beam_fits_right.filtered_traces_x[i])
                                        / 2.0
                                })
                                .collect();
                            let filtered_traces_y_avg: Vec<_> = (0..n_filters)
                                .map(|i| {
                                    (&beam_fits_left.filtered_traces_y[i]
                                        + &beam_fits_right.filtered_traces_y[i])
                                        / 2.0
                                })
                                .collect();

                            let beam_fits = BeamWidthFits {
                                popt_xs: popt_avg.clone(),
                                popt_ys: popt_avg,
                                filtered_traces_x: filtered_traces_x_avg,
                                filtered_traces_y: filtered_traces_y_avg,
                                x_positions: beam_fits_left.x_positions.clone(),
                                y_positions: beam_fits_left.y_positions.clone(),
                                popt_xs_left: Some(beam_fits_left.popt_xs.clone()),
                                popt_xs_right: Some(beam_fits_right.popt_xs.clone()),
                                popt_ys_left: Some(beam_fits_left.popt_ys.clone()),
                                popt_ys_right: Some(beam_fits_right.popt_ys.clone()),
                                filtered_traces_x_left: Some(
                                    beam_fits_left.filtered_traces_x.clone(),
                                ),
                                filtered_traces_x_right: Some(
                                    beam_fits_right.filtered_traces_x.clone(),
                                ),
                                filtered_traces_y_left: Some(
                                    beam_fits_left.filtered_traces_y.clone(),
                                ),
                                filtered_traces_y_right: Some(
                                    beam_fits_right.filtered_traces_y.clone(),
                                ),
                                x_positions_left: Some(left.positions.clone()),
                                x_positions_right: Some(right.positions.clone()),
                                y_positions_left: Some(left.positions.clone()),
                                y_positions_right: Some(right.positions.clone()),
                            };

                            let mean_fit = MeanBeamFit {
                                x0: ((-mean_fit_left.x0) + mean_fit_right.x0) / 2.0 - mean_pos,
                                y0: 0.0,
                                popt_x: mean_fit_right.popt_x,
                                popt_y: mean_fit_right.popt_y,
                            };

                            Some((Arc::new(meas_raw), mean_fit, beam_fits))
                        })
                        .collect();

                    if cancel_flag.load(Ordering::Relaxed) {
                        continue;
                    }

                    let (x_measurement, mean_fit_x, beam_fits_x) = results[0]
                        .as_ref()
                        .map(|(m, mf, bf)| {
                            (Some(Arc::clone(m)), Some(mf.clone()), Some(bf.clone()))
                        })
                        .unwrap_or((None, None, None));
                    let (y_measurement, mean_fit_y, beam_fits_y) = results[1]
                        .as_ref()
                        .map(|(m, mf, bf)| {
                            (Some(Arc::clone(m)), Some(mf.clone()), Some(bf.clone()))
                        })
                        .unwrap_or((None, None, None));

                    let mut warnings = Vec::new();
                    if let Some(w) = check_transition_width(
                        filter_params.start_freq,
                        filter_params.end_freq,
                        filter_params.win_width,
                    ) {
                        warnings.push(w);
                    }

                    result_tx
                        .send(ComputeResult::Complete {
                            x_measurement,
                            y_measurement,
                            filters,
                            mean_fit_x,
                            mean_fit_y,
                            beam_fits_x,
                            beam_fits_y,
                            warnings,
                        })
                        .ok();
                }
            }
        }
    }

    fn check_results(&mut self) {
        let mut should_update_curve_fits = false;

        if let Some(rx) = &self.result_rx {
            while let Ok(result) = rx.try_recv() {
                match result {
                    ComputeResult::ProgressX {
                        phase,
                        current_filter,
                        total_filters,
                    } => {
                        if let ComputationState::Computing { progress_x, .. } =
                            &mut self.computation_state
                        {
                            *progress_x = Some(AxisProgress {
                                phase,
                                current_filter,
                                total_filters,
                            });
                        }
                    }
                    ComputeResult::ProgressY {
                        phase,
                        current_filter,
                        total_filters,
                    } => {
                        if let ComputationState::Computing { progress_y, .. } =
                            &mut self.computation_state
                        {
                            *progress_y = Some(AxisProgress {
                                phase,
                                current_filter,
                                total_filters,
                            });
                        }
                    }
                    ComputeResult::Complete {
                        x_measurement,
                        y_measurement,
                        filters,
                        mean_fit_x,
                        mean_fit_y,
                        beam_fits_x,
                        beam_fits_y,
                        warnings,
                    } => {
                        // Only accept results from the currently active computation.
                        // A stale Complete from a cancelled run must not overwrite state.
                        if matches!(self.computation_state, ComputationState::Computing { .. }) {
                            self.x_measurement = x_measurement;
                            self.y_measurement = y_measurement;
                            self.filters = Some(filters);
                            self.filter_response_cache = None;
                            self.mean_fit_x = mean_fit_x;
                            self.mean_fit_y = mean_fit_y;
                            self.beam_fits_x = beam_fits_x;
                            self.beam_fits_y = beam_fits_y;
                            self.computation_state = ComputationState::Complete;
                            self.active_warnings = warnings;
                            should_update_curve_fits = true;
                        }
                    }
                    ComputeResult::Error(err) => {
                        // Same guard: ignore errors from cancelled computations.
                        if matches!(self.computation_state, ComputationState::Computing { .. }) {
                            self.computation_state = ComputationState::Error(err);
                        }
                    }
                }
            }
        }

        if should_update_curve_fits {
            self.compute_curve_fits();
            self.psf_applied = false; // new result — needs explicit re-apply
            if self.show_diagnostics {
                self.update_diagnostics();
            }
        }
    }

    fn check_dialog_results(&mut self) {
        while let Ok(result) = self.dialog_rx.try_recv() {
            match result {
                DialogResult::XFile(Some(path)) => {
                    self.knife_edge_x_path = path.to_string_lossy().to_string();
                    self.save_state();
                    self.last_params_hash = 0;
                }
                DialogResult::YFile(Some(path)) => {
                    self.knife_edge_y_path = path.to_string_lossy().to_string();
                    self.save_state();
                    self.last_params_hash = 0;
                }
                DialogResult::ExportFile(Some(path)) => {
                    if let Some(curve_fits) = &self.curve_fits {
                        match export::export_to_npz(&path, curve_fits) {
                            Ok(_) => {
                                self.status_message =
                                    Some(format!("Exported to {}", path.display()));
                            }
                            Err(e) => {
                                self.status_message = Some(format!("Export failed: {}", e));
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    fn check_file_dialog_results(&mut self, ctx: &egui::Context) {
        if self.file_dialog_state == PsfDialogState::None {
            return;
        }

        if let Some(path) = self
            .file_dialog
            .update_with_right_panel_ui(ctx, &mut |_ui, _dia| {})
            .picked()
        {
            match self.file_dialog_state {
                PsfDialogState::OpenX => {
                    self.knife_edge_x_path = path.to_string_lossy().to_string();
                    self.save_state();
                    self.last_params_hash = 0;
                }
                PsfDialogState::OpenY => {
                    self.knife_edge_y_path = path.to_string_lossy().to_string();
                    self.save_state();
                    self.last_params_hash = 0;
                }
                PsfDialogState::SaveExport => {
                    if let Some(curve_fits) = &self.curve_fits {
                        match export::export_to_npz(&path, curve_fits) {
                            Ok(_) => {
                                self.status_message =
                                    Some(format!("Exported to {}", path.display()));
                            }
                            Err(e) => {
                                self.status_message = Some(format!("Export failed: {}", e));
                            }
                        }
                    }
                }
                PsfDialogState::None => {}
            }
            self.file_dialog_state = PsfDialogState::None;
        }
    }

    fn compute_curve_fits(&mut self) {
        let frequencies_thz = match &self.filters {
            Some(f) => f.center_frequencies.clone(),
            None => return,
        };

        let (wx, wy, x0, y0) = match (&self.beam_fits_x, &self.beam_fits_y) {
            (Some(fx), Some(fy)) => {
                let wx = (0..frequencies_thz.len())
                    .map(|i| fx.popt_xs[[i, 1]].abs())
                    .collect();
                let wy = (0..frequencies_thz.len())
                    .map(|i| fy.popt_ys[[i, 1]].abs())
                    .collect();
                let x0 = (0..frequencies_thz.len())
                    .map(|i| fx.popt_xs[[i, 0]])
                    .collect();
                let y0 = (0..frequencies_thz.len())
                    .map(|i| fy.popt_ys[[i, 0]])
                    .collect();
                (wx, wy, x0, y0)
            }
            (Some(fx), None) => {
                let wx: Vec<f64> = (0..frequencies_thz.len())
                    .map(|i| fx.popt_xs[[i, 1]].abs())
                    .collect();
                let x0: Vec<f64> = (0..frequencies_thz.len())
                    .map(|i| fx.popt_xs[[i, 0]])
                    .collect();
                (wx.clone(), wx, x0.clone(), x0)
            }
            (None, Some(fy)) => {
                let wy: Vec<f64> = (0..frequencies_thz.len())
                    .map(|i| fy.popt_ys[[i, 1]].abs())
                    .collect();
                let y0: Vec<f64> = (0..frequencies_thz.len())
                    .map(|i| fy.popt_ys[[i, 0]])
                    .collect();
                (wy.clone(), wy, y0.clone(), y0)
            }
            _ => return,
        };

        match CurveFits::fit_from_data(&frequencies_thz, &wx, &wy, &x0, &y0) {
            Ok(fits) => self.curve_fits = Some(fits),
            Err(e) => {
                log::error!("Failed to compute curve fits: {}", e);
                self.curve_fits = None;
            }
        }
    }

    fn update_diagnostics(&mut self) {
        let curve_fits = match &self.curve_fits {
            Some(f) => f,
            None => return,
        };
        let n = 200usize;
        let freqs: Vec<f64> = (0..n)
            .map(|i| 0.1 + (i as f64 / (n - 1) as f64) * 9.9)
            .collect();
        let w0x = curve_fits.wx_fit.evaluate(&freqs);
        let w0y = curve_fits.wy_fit.evaluate(&freqs);
        if let Ok(diag) = DiagnosticResults::compute(&freqs, &w0x, &w0y) {
            self.diagnostics_window = Some(DiagnosticWindow::new(diag));
        }
    }

    fn ensure_filter_response_cache(&mut self) {
        let filters = match &self.filters {
            Some(f) => f,
            None => {
                self.filter_response_cache = None;
                return;
            }
        };

        let expected_len = filters.center_frequencies.len();
        let cache_is_valid = self
            .filter_response_cache
            .as_ref()
            .map(|cache| cache.curves_hz.len() == expected_len)
            .unwrap_or(false);

        if cache_is_valid {
            return;
        }

        let mut curves_hz = Vec::with_capacity(expected_len);
        let mut curves_wavelength_um = Vec::with_capacity(expected_len);

        for i in 0..expected_len {
            let row = filters.coefficients.row(i).to_vec();
            let (freqs, mags) = frequency_response(&row, 512, filters.fs);

            let mut points_hz = Vec::with_capacity(freqs.len());
            let mut points_wavelength = Vec::with_capacity(freqs.len());

            for (&f, &m) in freqs.iter().zip(mags.iter()) {
                points_hz.push([f, m]);
                if f > 0.0 {
                    points_wavelength.push([300.0 / f, m]);
                }
            }

            curves_hz.push(points_hz);
            curves_wavelength_um.push(points_wavelength);
        }

        self.filter_response_cache = Some(FilterResponseCache {
            curves_hz,
            curves_wavelength_um,
        });
    }

    // ─── Public API ─────────────────────────────────────────────────────────

    /// Returns the computed curve fits if available (used by deconvolution).
    pub fn curve_fits(&self) -> Option<&CurveFits> {
        self.curve_fits.as_ref()
    }

    /// Main UI entry point called by the bevy system on each frame.
    pub fn show_ui(&mut self, ctx: &egui::Context) {
        self.check_results();
        self.check_dialog_results();
        #[cfg(not(target_os = "macos"))]
        self.check_file_dialog_results(ctx);

        if self.show_filter_response && self.filters.is_some() {
            self.ensure_filter_response_cache();
        }

        // Auto-trigger computation when parameters or paths change
        let current_hash = self.compute_params_hash();
        if current_hash != self.last_params_hash && self.should_compute() {
            self.cancel_flag.store(true, Ordering::Relaxed);
            self.start_computation();
        }

        // Request repaint while computing (progress animation)
        if matches!(self.computation_state, ComputationState::Computing { .. }) {
            ctx.request_repaint();
        }

        let mut viewport_ui = crate::gui::utils::viewport_ui(ctx);

        // ── Top panel ──────────────────────────────────────────────────────
        egui::Panel::top("psf_top_panel").show(&mut viewport_ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading("THz PSF Tool");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("🔬 PSF Visualizer").clicked() {
                        if self.curve_fits.is_some() {
                            if self.psf_visualizer_window.is_none() {
                                self.psf_visualizer_window = Some(PsfVisualizerWindow::new());
                            }
                            self.show_psf_visualizer = true;
                        }
                    }
                    if ui.button("📈 Individual Fits").clicked() {
                        if (self.beam_fits_x.is_some() || self.beam_fits_y.is_some())
                            && self.filters.is_some()
                        {
                            let total = self.filters.as_ref().unwrap().center_frequencies.len();
                            if let Some(window) = &mut self.individual_fits_window {
                                window.update_total_filters(total);
                            } else {
                                self.individual_fits_window =
                                    Some(IndividualFitsWindow::new(total));
                            }
                            self.show_individual_fits = true;
                        }
                    }
                    if ui.button("📊 Diagnostics").clicked() {
                        if self.curve_fits.is_some() {
                            self.update_diagnostics();
                            self.show_diagnostics = true;
                        }
                    }
                });
            });
        });

        // ── Left panel (parameters) ────────────────────────────────────────
        egui::Panel::left("psf_control_panel")
            .default_size(self.control_panel_width)
            .resizable(true)
            .show(&mut viewport_ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.heading("Parameters");

                    ui.group(|ui| {
                        ui.label("Input Files");
                        ui.horizontal(|ui| {
                            ui.label("X measurement:");
                            if ui.button(egui_phosphor::regular::FOLDER_OPEN).clicked() {
                                #[cfg(target_os = "macos")]
                                {
                                    let dialog_tx = self.dialog_tx.clone();
                                    std::thread::spawn(move || {
                                        let task = rfd::AsyncFileDialog::new()
                                            .add_filter("THz files", &["thz"])
                                            .pick_file();
                                        futures::executor::block_on(async {
                                            let path =
                                                task.await.map(|file| file.path().to_path_buf());
                                            let _ = dialog_tx.send(DialogResult::XFile(path));
                                        });
                                    });
                                }
                                #[cfg(not(target_os = "macos"))]
                                {
                                    self.file_dialog.pick_file();
                                    self.file_dialog_state = PsfDialogState::OpenX;
                                }
                            }
                            ui.add(egui::TextEdit::singleline(&mut self.knife_edge_x_path)
                                .hint_text("path/to/x_measurement.thz"));
                        });
                        ui.horizontal(|ui| {
                            ui.label("Y measurement:");
                            if ui.button(egui_phosphor::regular::FOLDER_OPEN).clicked() {
                                #[cfg(target_os = "macos")]
                                {
                                    let dialog_tx = self.dialog_tx.clone();
                                    std::thread::spawn(move || {
                                        let task = rfd::AsyncFileDialog::new()
                                            .add_filter("THz files", &["thz"])
                                            .pick_file();
                                        futures::executor::block_on(async {
                                            let path =
                                                task.await.map(|file| file.path().to_path_buf());
                                            let _ = dialog_tx.send(DialogResult::YFile(path));
                                        });
                                    });
                                }
                                #[cfg(not(target_os = "macos"))]
                                {
                                    self.file_dialog.pick_file();
                                    self.file_dialog_state = PsfDialogState::OpenY;
                                }
                            }
                            ui.add(egui::TextEdit::singleline(&mut self.knife_edge_y_path)
                                .hint_text("path/to/y_measurement.thz"));
                        });
                    });

                    ui.add_space(8.0);

                    ui.group(|ui| {
                        ui.label("Filter Parameters");
                        let mut changed = false;

                        ui.horizontal(|ui| {
                            ui.label("N filters:").on_hover_text("Number of bandpass filters");
                            changed |= ui.add(egui::Slider::new(&mut self.filter_params.n_filters, 5..=200)).changed();
                        });
                        ui.horizontal(|ui| {
                            ui.label("Spacing:");
                            let prev_spacing = self.filter_params.frequency_spacing;
                            egui::ComboBox::from_id_salt("freq_spacing_combo")
                                .selected_text(match self.filter_params.frequency_spacing {
                                    FrequencySpacing::Log => "Logarithmic",
                                    FrequencySpacing::Linear => "Linear",
                                })
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(
                                        &mut self.filter_params.frequency_spacing,
                                        FrequencySpacing::Log,
                                        "Logarithmic",
                                    );
                                    ui.selectable_value(
                                        &mut self.filter_params.frequency_spacing,
                                        FrequencySpacing::Linear,
                                        "Linear",
                                    );
                                });
                            changed |= self.filter_params.frequency_spacing != prev_spacing;
                        });
                        ui.horizontal(|ui| {
                            ui.label("Low cutoff [THz]:");
                            changed |= ui.add(egui::DragValue::new(&mut self.filter_params.low_cut).speed(0.05).range(0.01..=5.0)).changed();
                        });
                        ui.horizontal(|ui| {
                            ui.label("High cutoff [THz]:");
                            changed |= ui.add(egui::DragValue::new(&mut self.filter_params.high_cut).speed(0.05).range(0.1..=20.0)).changed();
                        });
                        ui.horizontal(|ui| {
                            ui.label("Start freq. [THz]:");
                            let min_start = (self.filter_params.low_cut + 0.01).max(0.01);
                            changed |= ui.add(egui::DragValue::new(&mut self.filter_params.start_freq).speed(0.05).range(min_start..=self.filter_params.high_cut)).changed();
                        });
                        ui.horizontal(|ui| {
                            ui.label("End freq. [THz]:");
                            let max_end = (self.filter_params.high_cut - 0.01).min(20.0);
                            let min_end = (self.filter_params.low_cut + 0.01).max(0.01);
                            changed |= ui.add(egui::DragValue::new(&mut self.filter_params.end_freq).speed(0.1).range(min_end..=max_end)).changed();
                        });
                        ui.horizontal(|ui| {
                            ui.label("Transition width [THz]:").on_hover_text("Transition band width (0.1–1.0 THz)");
                            changed |= ui.add(egui::DragValue::new(&mut self.filter_params.win_width).speed(0.05).range(0.1..=1.0)).changed();
                        });

                        if changed {
                            // Clamp start_freq: must be > low_cut (at least low_cut + 0.01)
                            // and <= high_cut
                            let min_start = (self.filter_params.low_cut + 0.01).max(0.01);
                            let max_start = self.filter_params.high_cut;
                            self.filter_params.start_freq =
                                self.filter_params.start_freq.clamp(min_start, max_start);

                            self.save_state();
                            if matches!(self.computation_state, ComputationState::Computing { .. }) {
                                self.cancel_flag.store(true, Ordering::Relaxed);
                                self.computation_state = ComputationState::Idle;
                            }
                            self.last_params_hash = 0;
                        }
                    });

                    ui.add_space(8.0);

                    ui.group(|ui| {
                        ui.label("Fitting Parameters");
                        let mut changed = false;
                        ui.horizontal(|ui| {
                            ui.label("Max beam width [mm]:").on_hover_text("Upper bound for beam width optimization");
                            changed |= ui.add(egui::DragValue::new(&mut self.fit_params.w_max).speed(1.0).range(1.0..=200.0)).changed();
                        });
                        ui.horizontal(|ui| {
                            changed |= ui.checkbox(&mut self.fit_params.use_monotonicity_constraint, "Monotonicity constraint")
                                .on_hover_text("Constrain each frequency fit using the previous result")
                                .changed();
                        });
                        if changed {
                            self.save_state();
                            if matches!(self.computation_state, ComputationState::Computing { .. }) {
                                self.cancel_flag.store(true, Ordering::Relaxed);
                                self.computation_state = ComputationState::Idle;
                            }
                            self.last_params_hash = 0;
                        }
                    });

                    ui.add_space(12.0);

                    // Progress / status
                    match &self.computation_state {
                        ComputationState::Idle => {
                            ui.label("Idle — waiting for input files");
                        }
                        ComputationState::Computing { progress_x, progress_y } => {
                            ui.label("Computing PSF…");
                            for (label, progress) in [("X", progress_x), ("Y", progress_y)] {
                                if let Some(p) = progress {
                                    let frac = p.current_filter as f32 / p.total_filters as f32;
                                    let phase = match p.phase {
                                        ComputationPhase::Loading => format!("Loading {}", label),
                                        ComputationPhase::Processing => format!("Processing {}", label),
                                        ComputationPhase::Fitting => format!("Fitting {} ({}/{})", label, p.current_filter, p.total_filters),
                                    };
                                    ui.label(phase);
                                    ui.add(egui::ProgressBar::new(frac).show_percentage().animate(true));
                                }
                            }
                        }
                        ComputationState::Complete => {
                            let color = if ctx.global_style().visuals.dark_mode { egui::Color32::GREEN } else { egui::Color32::from_rgb(0, 120, 0) };
                            ui.colored_label(color, "✔ Computation complete");

                            // Warnings
                            for w in &self.active_warnings {
                                let warn_color = if ctx.global_style().visuals.dark_mode {
                                    egui::Color32::YELLOW
                                } else {
                                    egui::Color32::from_rgb(160, 100, 0)
                                };
                                ui.colored_label(warn_color, format!("⚠ {}", w.message()));
                            }
                        }
                        ComputationState::Error(err) => {
                            ui.colored_label(egui::Color32::RED, format!("✘ Error: {}", err));
                        }
                    }

                    ui.add_space(12.0);

                    ui.group(|ui| {
                        ui.label("Display Options");
                        ui.checkbox(&mut self.show_filter_response, "Filter response");
                        ui.checkbox(&mut self.show_intensity, "Intensity");
                        ui.checkbox(&mut self.show_beam_widths, "Beam widths");
                        ui.checkbox(&mut self.show_beam_centers, "Beam centers");
                        ui.separator();
                        if ui.checkbox(&mut self.use_wavelength, "Use wavelength (µm)").changed() {
                            self.save_state();
                        }
                    });

                    ui.add_space(12.0);

                    // Apply to deconvolution button
                    let has_fits = self.curve_fits.is_some();
                    ui.add_enabled_ui(has_fits, |ui| {
                        let label = if self.psf_applied {
                            "✔ PSF applied to deconvolution"
                        } else {
                            "Use PSF for deconvolution"
                        };
                        if ui.button(label)
                            .on_hover_text("Apply the computed PSF profile to the deconvolution in the main application.")
                            .clicked()
                        {
                            self.apply_to_deconv_requested = true;
                            self.psf_applied = true;
                            self.status_message = Some("PSF applied.".to_string());
                        }
                    });

                    ui.add_space(6.0);

                    // Reset Parameters button
                    if ui.button("🔄 Reset Parameters")
                        .on_hover_text("Reset all filter and fitting parameters to default.").clicked() {
                        self.reset_parameters();
                    }

                    // Export button
                    if ui.button("💾 Export PSF to .npz").clicked() {
                        if let Some(curve_fits) = &self.curve_fits {
                            let _ = curve_fits;
                            #[cfg(target_os = "macos")]
                            {
                                let dialog_tx = self.dialog_tx.clone();
                                std::thread::spawn(move || {
                                    let task = rfd::AsyncFileDialog::new()
                                        .set_file_name("psf_coefficients.npz")
                                        .add_filter("NumPy Archive", &["npz"])
                                        .save_file();
                                    futures::executor::block_on(async {
                                        let path =
                                            task.await.map(|file| file.path().to_path_buf());
                                        let _ = dialog_tx.send(DialogResult::ExportFile(path));
                                    });
                                });
                            }
                            #[cfg(not(target_os = "macos"))]
                            {
                                self.file_dialog.save_file();
                                self.file_dialog_state = PsfDialogState::SaveExport;
                            }
                        } else {
                            self.status_message = Some("No PSF data computed yet".to_string());
                        }
                    }

                    if let Some(msg) = &self.status_message {
                        ui.label(msg);
                    }
                });
            });

        // ── Central panel (plots) ──────────────────────────────────────────
        egui::CentralPanel::default().show(&mut viewport_ui, |ui| {
            if self.filters.is_none()
                && self.x_measurement.is_none()
                && self.y_measurement.is_none()
            {
                ui.centered_and_justified(|ui| {
                    ui.label("Load X and/or Y knife-edge measurement files to compute the PSF.");
                });
                return;
            }

            let available_height = ui.available_height();
            let mut n_plots = 0usize;
            if self.show_filter_response && self.filters.is_some() {
                n_plots += 1;
            }
            if self.show_intensity && (self.x_measurement.is_some() || self.y_measurement.is_some())
            {
                n_plots += 1;
            }
            if self.show_beam_widths && (self.beam_fits_x.is_some() || self.beam_fits_y.is_some()) {
                n_plots += 1;
            }
            if self.show_beam_centers && (self.beam_fits_x.is_some() || self.beam_fits_y.is_some())
            {
                n_plots += 1;
            }
            let plot_height = if n_plots > 0 {
                (available_height / n_plots as f32).max(150.0)
            } else {
                150.0
            };

            egui::ScrollArea::vertical().show(ui, |ui| {
                // Filter frequency response
                if self.show_filter_response {
                    if let (Some(filters), Some(cache)) =
                        (&self.filters, &self.filter_response_cache)
                    {
                        ui.label("Filter frequency response:");
                        let x_axis = self
                            .use_wavelength
                            .then(|| "Wavelength (µm)")
                            .unwrap_or("Frequency (THz)");
                        let use_wavelength = self.use_wavelength;
                        Plot::new("filter_response")
                            .height(plot_height)
                            .x_axis_label(x_axis)
                            .y_axis_label("Amplitude")
                            .legend(Legend::default())
                            .show(ui, |plot_ui| {
                                for i in 0..filters.center_frequencies.len() {
                                    let points = if use_wavelength {
                                        cache.curves_wavelength_um[i].clone()
                                    } else {
                                        cache.curves_hz[i].clone()
                                    };
                                    let label = format!("{:.2} THz", filters.center_frequencies[i]);
                                    plot_ui.line(Line::new(label, points));
                                }
                            });
                    }
                }

                // Beam widths
                if self.show_beam_widths {
                    if let (Some(filters), Some(fits_x)) = (&self.filters, &self.beam_fits_x) {
                        let n = filters.center_frequencies.len();
                        let freqs = &filters.center_frequencies;
                        ui.label("Beam width vs frequency:");
                        Plot::new("beam_widths")
                            .height(plot_height)
                            .x_axis_label(if self.use_wavelength {
                                "Wavelength (µm)"
                            } else {
                                "Frequency (THz)"
                            })
                            .y_axis_label("Beam width (mm)")
                            .show(ui, |plot_ui| {
                                let x_vals: PlotPoints = (0..n)
                                    .map(|i| {
                                        let f = freqs[i];
                                        let x = if self.use_wavelength {
                                            3e5 / f / 1e6 * 1000.0
                                        } else {
                                            f
                                        };
                                        [x, fits_x.popt_xs[[i, 1]].abs()]
                                    })
                                    .collect();
                                plot_ui.line(Line::new("wx", x_vals).color(egui::Color32::BLUE));

                                if let Some(fits_y) = &self.beam_fits_y {
                                    let y_vals: PlotPoints = (0..n)
                                        .map(|i| {
                                            let f = freqs[i];
                                            let x = if self.use_wavelength {
                                                3e5 / f / 1e6 * 1000.0
                                            } else {
                                                f
                                            };
                                            [x, fits_y.popt_ys[[i, 1]].abs()]
                                        })
                                        .collect();
                                    plot_ui.line(
                                        Line::new("wy", y_vals)
                                            .color(egui::Color32::from_rgb(0, 160, 0)),
                                    );
                                }
                            });
                    }
                }

                // Beam centers
                if self.show_beam_centers {
                    if let (Some(filters), Some(fits_x)) = (&self.filters, &self.beam_fits_x) {
                        let n = filters.center_frequencies.len();
                        let freqs = &filters.center_frequencies;
                        ui.label("Beam center vs frequency:");
                        Plot::new("beam_centers")
                            .height(plot_height)
                            .x_axis_label(if self.use_wavelength {
                                "Wavelength (µm)"
                            } else {
                                "Frequency (THz)"
                            })
                            .y_axis_label("Center position (mm)")
                            .show(ui, |plot_ui| {
                                let x_vals: PlotPoints = (0..n)
                                    .map(|i| {
                                        let f = freqs[i];
                                        let x = if self.use_wavelength {
                                            3e5 / f / 1e6 * 1000.0
                                        } else {
                                            f
                                        };
                                        [x, fits_x.popt_xs[[i, 0]]]
                                    })
                                    .collect();
                                plot_ui.line(Line::new("x0", x_vals).color(egui::Color32::BLUE));

                                if let Some(fits_y) = &self.beam_fits_y {
                                    let y_vals: PlotPoints = (0..n)
                                        .map(|i| {
                                            let f = freqs[i];
                                            let x = if self.use_wavelength {
                                                3e5 / f / 1e6 * 1000.0
                                            } else {
                                                f
                                            };
                                            [x, fits_y.popt_ys[[i, 0]]]
                                        })
                                        .collect();
                                    plot_ui.line(
                                        Line::new("y0", y_vals)
                                            .color(egui::Color32::from_rgb(0, 160, 0)),
                                    );
                                }
                            });
                    }
                }
            });
        });
    }
}
