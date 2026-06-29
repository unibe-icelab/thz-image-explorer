//! Infrastructure for native OS secondary windows using bevy_egui's multi-pass schedule.
//!
//! Each secondary window is associated with:
//! - A dedicated [`ScheduleLabel`] (e.g. [`SettingsContextPass`])
//! - A marker [`Component`] on its camera (e.g. [`SettingsWindowCamera`])
//! - A Bevy system registered on that schedule label that draws the egui UI
//!
//! Window spawn/despawn is managed inside `update_gui` (which already holds
//! `NonSendMut<THzImageExplorer>`) so there is no separate `Update` system
//! that would create scheduling ambiguities.

use crate::config::ThreadCommunication;
use crate::filters::psf::{CubicSplineCoeffs, HybridFit, PSF};
use bevy::ecs::schedule::ScheduleLabel;
use bevy::prelude::*;
use bevy_egui::egui;
use bevy_egui::EguiContext;
use ndarray::Array1;

// ─── Settings window ────────────────────────────────────────────────────────

/// [`ScheduleLabel`] for the Settings native OS window egui context pass.
#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
pub struct SettingsContextPass;

/// Marker component attached to the camera that drives the Settings OS window.
#[derive(Component)]
pub struct SettingsWindowCamera;

// ─── PSF tool window ─────────────────────────────────────────────────────────

/// [`ScheduleLabel`] for the PSF Tool native OS window egui context pass.
#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
pub struct PsfToolContextPass;

/// Marker component attached to the camera that drives the PSF Tool OS window.
#[derive(Component)]
pub struct PsfToolWindowCamera;
// ─── PSF tool sub-windows ───────────────────────────────────────────────

#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
pub struct PsfDiagnosticsContextPass;
#[derive(Component)]
pub struct PsfDiagnosticsWindowCamera;

#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
pub struct PsfVisualizerContextPass;
#[derive(Component)]
pub struct PsfVisualizerWindowCamera;

#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
pub struct PsfIndividualFitsContextPass;
#[derive(Component)]
pub struct PsfIndividualFitsWindowCamera;
// ─── Shared secondary-window state ──────────────────────────────────────────

/// Bevy resource that tracks the entity IDs of any spawned secondary windows
/// and their associated cameras.
#[derive(Resource, Default)]
pub struct SecondaryWindowState {
    pub settings_window_entity: Option<Entity>,
    pub settings_camera_entity: Option<Entity>,
    pub psf_tool_window_entity: Option<Entity>,
    pub psf_tool_camera_entity: Option<Entity>,
    pub psf_diagnostics_window_entity: Option<Entity>,
    pub psf_diagnostics_camera_entity: Option<Entity>,
    pub psf_visualizer_window_entity: Option<Entity>,
    pub psf_visualizer_camera_entity: Option<Entity>,
    pub psf_individual_fits_window_entity: Option<Entity>,
    pub psf_individual_fits_camera_entity: Option<Entity>,
    /// Cameras queued for despawn — processed by `cleanup_closed_windows` AFTER
    /// `run_egui_context_pass_loop_system` so the loop never sees a mid-iteration
    /// entity disappear (which would cause a bevy_egui panic).
    pub cameras_to_despawn: Vec<Entity>,
}

// ─── Font / style initialisation ────────────────────────────────────────────

/// Runs in `Update`. Fires once for every newly created egui context that
/// belongs to a secondary window (detected by `Added<EguiContext>` + marker).
/// Installs Phosphor icons, image loaders, the current theme and the rounded

// ─── Theme helper ────────────────────────────────────────────────────────────

/// Apply the current theme preference to any secondary egui context every frame.
/// This handles both manual preference changes (via the ThemeSwitch in settings)
/// and OS-level system theme changes.
pub fn sync_ctx_theme(ctx: &egui::Context, pref: egui::ThemePreference) {
    // For System preference: detect OS changes every frame (cached at 500ms)
    crate::system_theme::apply_system_theme_if_needed(ctx, pref);
    // If the stored preference differs (e.g. user changed it in settings),
    // force-apply visuals so the change is visible immediately.
    let current = ctx.options(|o| o.theme_preference);
    if current != pref {
        ctx.set_theme(pref);
        match pref {
            egui::ThemePreference::Dark => {
                ctx.set_visuals(egui::Visuals::dark());
                ctx.global_style_mut(|s| s.visuals.handle_shape = egui::style::HandleShape::Circle);
            }
            egui::ThemePreference::Light => {
                ctx.set_visuals(egui::Visuals::light());
                ctx.global_style_mut(|s| s.visuals.handle_shape = egui::style::HandleShape::Circle);
            }
            egui::ThemePreference::System => {
                crate::system_theme::apply_system_theme_if_needed(ctx, pref);
            }
        }
    }
}

// ─── Font + initial theme setup (runs once per new context) ──────────────────

/// Bevy system that runs once per new secondary egui context (detected via
/// `Added<EguiContext>`). Installs fonts, image loaders, and the current theme.
pub fn setup_secondary_context_fonts(
    mut new_contexts: Query<
        &mut EguiContext,
        (
            Added<EguiContext>,
            Or<(
                With<SettingsWindowCamera>,
                With<PsfToolWindowCamera>,
                With<PsfDiagnosticsWindowCamera>,
                With<PsfVisualizerWindowCamera>,
                With<PsfIndividualFitsWindowCamera>,
            )>,
        ),
    >,
    thread_communication: Res<ThreadCommunication>,
) {
    for mut ctx in new_contexts.iter_mut() {
        let ctx = ctx.get_mut();
        let mut fonts = egui::FontDefinitions::default();
        egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
        ctx.set_fonts(fonts);
        egui_extras::install_image_loaders(ctx);
        ctx.set_theme(thread_communication.gui_settings.theme_preference);
        match thread_communication.gui_settings.theme_preference {
            egui::ThemePreference::Dark => {
                ctx.set_visuals(egui::Visuals::dark());
            }
            egui::ThemePreference::Light => {
                ctx.set_visuals(egui::Visuals::light());
            }
            egui::ThemePreference::System => {
                let is_dark = crate::system_theme::is_system_dark_mode();
                ctx.set_visuals(if is_dark {
                    egui::Visuals::dark()
                } else {
                    egui::Visuals::light()
                });
            }
        }
        ctx.all_styles_mut(|styles| {
            styles.visuals.handle_shape = egui::style::HandleShape::Circle;
        });
    }
}

// ─── PSF tool Bevy system ────────────────────────────────────────────────────

/// Bevy system registered on [`PsfToolContextPass`].
/// Renders the PSF tool UI and, when computation is complete, converts the
/// resulting [`CurveFits`] into the main app's [`PSF`] struct so that
/// deconvolution can use it immediately.
pub fn psf_tool_system(
    mut egui_ctx: Query<&mut EguiContext, With<PsfToolWindowCamera>>,
    mut explorer: NonSendMut<crate::gui::application::THzImageExplorer>,
    mut thread_communication: ResMut<ThreadCommunication>,
) {
    let Ok(mut ctx_guard) = egui_ctx.single_mut() else {
        return;
    };
    let ctx = ctx_guard.get_mut();
    sync_ctx_theme(ctx, thread_communication.gui_settings.theme_preference);

    explorer.psf_tool.show_ui(ctx);

    // Apply curve fits to the main PSF only when the user explicitly requested it
    if explorer.psf_tool.apply_to_deconv_requested {
        explorer.psf_tool.apply_to_deconv_requested = false;
        if let Some(curve_fits) = explorer.psf_tool.curve_fits() {
            let psf = curve_fits_to_psf(curve_fits);
            // Send via channel so the data thread's local copy is also updated,
            // and also update psf_lock so the settings-window sync stays consistent
            let _ = thread_communication
                .config_tx
                .send(crate::config::ConfigCommand::ApplyPSF(psf.clone()));
            if let Ok(mut guard) = thread_communication.psf_lock.write() {
                *guard = (std::path::PathBuf::from("PSF Tool"), psf.clone());
            }
            thread_communication.gui_settings.psf = psf;
            thread_communication.gui_settings.beam_shape_path =
                std::path::PathBuf::from("PSF Tool");
        }
    }
}

/// Converts PSF tool [`CurveFits`] (f64) into the main app [`PSF`] struct (f32).
fn curve_fits_to_psf(cf: &crate::psf_tool::curve_fitting::CurveFits) -> PSF {
    fn spline_to_coeffs(s: &crate::psf_tool::curve_fitting::CubicSpline) -> CubicSplineCoeffs {
        CubicSplineCoeffs {
            knots: Array1::from_vec(s.x.iter().map(|&v| v as f32).collect()),
            values: Array1::from_vec(s.y.iter().map(|&v| v as f32).collect()),
            coeff_a: Array1::from_vec(s.coeffs.iter().map(|c| c[0] as f32).collect()),
            coeff_b: Array1::from_vec(s.coeffs.iter().map(|c| c[1] as f32).collect()),
            coeff_c: Array1::from_vec(s.coeffs.iter().map(|c| c[2] as f32).collect()),
            coeff_d: Array1::from_vec(s.coeffs.iter().map(|c| c[3] as f32).collect()),
        }
    }

    fn hybrid_to_main(h: &crate::psf_tool::curve_fitting::HybridFit) -> HybridFit {
        HybridFit {
            base_a: h.a as f32,
            base_b: h.b as f32,
            correction: spline_to_coeffs(&h.correction),
        }
    }

    PSF {
        wx_fit: hybrid_to_main(&cf.wx_fit),
        wy_fit: hybrid_to_main(&cf.wy_fit),
        x0_spline: spline_to_coeffs(&cf.x0_fit),
        y0_spline: spline_to_coeffs(&cf.y0_fit),
    }
}

// ─── PSF sub-window Bevy systems ─────────────────────────────────────────────

pub fn psf_diagnostics_system(
    mut egui_ctx: Query<&mut EguiContext, With<PsfDiagnosticsWindowCamera>>,
    mut explorer: NonSendMut<crate::gui::application::THzImageExplorer>,
    thread_communication: Res<ThreadCommunication>,
) {
    let Ok(mut ctx_guard) = egui_ctx.single_mut() else {
        return;
    };
    let ctx = ctx_guard.get_mut();
    sync_ctx_theme(ctx, thread_communication.gui_settings.theme_preference);
    if let Some(dw) = &mut explorer.psf_tool.diagnostics_window {
        dw.show(ctx);
    }
}

pub fn psf_visualizer_system(
    mut egui_ctx: Query<&mut EguiContext, With<PsfVisualizerWindowCamera>>,
    mut explorer: NonSendMut<crate::gui::application::THzImageExplorer>,
    thread_communication: Res<ThreadCommunication>,
) {
    let Ok(mut ctx_guard) = egui_ctx.single_mut() else {
        return;
    };
    let ctx = ctx_guard.get_mut();
    sync_ctx_theme(ctx, thread_communication.gui_settings.theme_preference);
    let curve_fits = explorer.psf_tool.curve_fits.clone();
    if let (Some(pv), Some(cf)) = (&mut explorer.psf_tool.psf_visualizer_window, curve_fits) {
        pv.show(ctx, &cf);
    }
}

pub fn psf_individual_fits_system(
    mut egui_ctx: Query<&mut EguiContext, With<PsfIndividualFitsWindowCamera>>,
    mut explorer: NonSendMut<crate::gui::application::THzImageExplorer>,
    thread_communication: Res<ThreadCommunication>,
) {
    let Ok(mut ctx_guard) = egui_ctx.single_mut() else {
        return;
    };
    let ctx = ctx_guard.get_mut();
    sync_ctx_theme(ctx, thread_communication.gui_settings.theme_preference);

    let bx = explorer.psf_tool.beam_fits_x.clone();
    let by = explorer.psf_tool.beam_fits_y.clone();
    let filters = explorer.psf_tool.filters.clone();

    if let (Some(bx), Some(by), Some(filters)) = (bx, by, filters) {
        if let (
            Some(popt_xs_left),
            Some(popt_xs_right),
            Some(popt_ys_left),
            Some(popt_ys_right),
            Some(ftx_left),
            Some(ftx_right),
            Some(fty_left),
            Some(fty_right),
            Some(xp_left),
            Some(xp_right),
            Some(yp_left),
            Some(yp_right),
        ) = (
            &bx.popt_xs_left,
            &bx.popt_xs_right,
            &by.popt_ys_left,
            &by.popt_ys_right,
            &bx.filtered_traces_x_left,
            &bx.filtered_traces_x_right,
            &by.filtered_traces_y_left,
            &by.filtered_traces_y_right,
            &bx.x_positions_left,
            &bx.x_positions_right,
            &by.y_positions_left,
            &by.y_positions_right,
        ) {
            let freqs = filters.center_frequencies.clone();
            if let Some(ifw) = &mut explorer.psf_tool.individual_fits_window {
                ifw.show(
                    ctx,
                    popt_xs_left,
                    popt_xs_right,
                    popt_ys_left,
                    popt_ys_right,
                    ftx_left,
                    ftx_right,
                    fty_left,
                    fty_right,
                    xp_left,
                    xp_right,
                    yp_left,
                    yp_right,
                    &freqs,
                );
            }
        }
    }
}

// ─── Deferred camera cleanup ─────────────────────────────────────────────────

/// Runs in `PostUpdate` **after** `EguiPostUpdateSet::EndPass`.
///
/// `run_egui_context_pass_loop_system` collects all multipass camera entities at
/// the start of the frame and writes back to them after each schedule run.  If a
/// camera entity is despawned *inside* one of those schedules (e.g. from
/// `update_gui`), the write-back panics with "previously queried context:
/// EntityDoesNotExist".
///
/// The fix: every place that used to call `commands.entity(cam).despawn()` from
/// within a multipass schedule now pushes the entity into
/// `SecondaryWindowState::cameras_to_despawn` instead.  This system then drains
/// that queue and performs the actual despawns once the bevy_egui loop is done.
pub fn cleanup_closed_windows(
    mut sec_win_state: ResMut<SecondaryWindowState>,
    mut commands: Commands,
) {
    for cam in sec_win_state.cameras_to_despawn.drain(..) {
        if let Ok(mut ec) = commands.get_entity(cam) {
            ec.despawn();
        }
    }
}
