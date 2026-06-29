use crate::gui::utils::viewport_ui;
use crate::psf_tool::diagnostics::DiagnosticResults;
use bevy_egui::egui;
use egui_plot::{Legend, Line, Plot, PlotPoints};

/// Diagnostic window showing PSF analysis
pub struct DiagnosticWindow {
    diagnostics: Option<DiagnosticResults>,
    // User-editable parameters
    focal_length_mm: String,
    ref_frequency_thz: String,
    aperture_d_mm: String,
    // Original input data for recalculation
    frequencies_thz: Vec<f64>,
    w0x_mm: Vec<f64>,
    w0y_mm: Vec<f64>,
}

impl DiagnosticWindow {
    pub fn new(diagnostics: DiagnosticResults) -> Self {
        // Extract input data for potential recalculation
        let frequencies_thz = diagnostics.frequencies_thz.clone();
        let w0x_mm = diagnostics.w0x_mm.clone();
        let w0y_mm = diagnostics.w0y_mm.clone();

        Self {
            diagnostics: Some(diagnostics),
            focal_length_mm: String::from("152.4"),
            ref_frequency_thz: String::from("1.0"),
            aperture_d_mm: String::new(),
            frequencies_thz,
            w0x_mm,
            w0y_mm,
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) {
        let mut viewport_ui = viewport_ui(ctx);
        egui::CentralPanel::default().show_inside(&mut viewport_ui, |ui| {
            self.show_ui(ui, ctx);
        });
    }

    pub fn show_ui(&mut self, ui: &mut egui::Ui, _ctx: &egui::Context) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading("PSF Diagnostics");
            ui.separator();

            // Parameter input section
            ui.group(|ui| {
                ui.heading("\u{2699} Optical Parameters");
                ui.add_space(5.0);

                let mut params_changed = false;

                egui::Grid::new("param_grid")
                    .num_columns(2)
                    .spacing([10.0, 8.0])
                    .show(ui, |ui| {
                        ui.label("Focal Length F (mm):");
                        let focal_response = ui.add(
                            egui::TextEdit::singleline(&mut self.focal_length_mm)
                                .hint_text("152.4 (6 inches)"),
                        );
                        if focal_response.changed() {
                            params_changed = true;
                        }
                        ui.end_row();

                        ui.label("Reference Frequency (THz):");
                        let freq_response = ui.add(
                            egui::TextEdit::singleline(&mut self.ref_frequency_thz)
                                .hint_text("1.0 (1 THz)"),
                        );
                        if freq_response.changed() {
                            params_changed = true;
                        }
                        ui.end_row();

                        ui.label("Aperture D (mm):");
                        let aperture_response = ui.add(
                            egui::TextEdit::singleline(&mut self.aperture_d_mm)
                                .hint_text("Leave empty to estimate"),
                        );
                        if aperture_response.changed() {
                            params_changed = true;
                        }
                        ui.end_row();
                    });

                ui.add_space(5.0);
                ui.label("💡 If values are not provided, automatic estimation will be performed.");

                // Auto-recalculate when parameters change
                if params_changed {
                    self.recalculate_diagnostics();
                }
            });

            ui.add_space(15.0);
            ui.separator();

            if let Some(diag) = &self.diagnostics {
                // Summary section
                ui.heading("📊 Summary");
                ui.add_space(5.0);
                ui.group(|ui| {
                    egui::ScrollArea::vertical()
                        .max_height(200.0)
                        .show(ui, |ui| {
                            ui.monospace(diag.summary());
                        });
                });

                ui.add_space(20.0);
                ui.separator();

                // Plot 1: w0 vs Frequency
                ui.heading("📈 1. Beam Waist w0 vs Frequency");
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    // X-axis plot
                    ui.vertical(|ui| {
                        ui.label("X-axis");
                        let points_x: PlotPoints = diag
                            .frequencies_thz
                            .iter()
                            .zip(diag.w0x_mm.iter())
                            .map(|(f, w)| [*f, *w])
                            .collect();

                        let theory_x: PlotPoints = diag
                            .frequencies_thz
                            .iter()
                            .zip(diag.w0_theory_x_mm.iter())
                            .map(|(f, w)| [*f, *w])
                            .collect();

                        Plot::new("w0x_vs_freq")
                            .legend(Legend::default())
                            .height(300.0)
                            .width(550.0)
                            .x_axis_label("Frequency (THz)")
                            .y_axis_label("w0x (mm)")
                            .show(ui, |plot_ui| {
                                plot_ui
                                    .line(Line::new("w0x(f)", points_x).color(egui::Color32::BLUE));
                                plot_ui.line(
                                    Line::new("w0x theory", theory_x)
                                        .color(egui::Color32::from_rgb(100, 100, 255))
                                        .style(egui_plot::LineStyle::Dashed { length: 10.0 }),
                                );
                            });
                    });

                    ui.add_space(10.0);

                    // Y-axis plot
                    ui.vertical(|ui| {
                        ui.label("Y-axis");
                        let points_y: PlotPoints = diag
                            .frequencies_thz
                            .iter()
                            .zip(diag.w0y_mm.iter())
                            .map(|(f, w)| [*f, *w])
                            .collect();

                        let theory_y: PlotPoints = diag
                            .frequencies_thz
                            .iter()
                            .zip(diag.w0_theory_y_mm.iter())
                            .map(|(f, w)| [*f, *w])
                            .collect();

                        Plot::new("w0y_vs_freq")
                            .legend(Legend::default())
                            .height(300.0)
                            .width(550.0)
                            .x_axis_label("Frequency (THz)")
                            .y_axis_label("w0y (mm)")
                            .show(ui, |plot_ui| {
                                plot_ui.line(
                                    Line::new("w0y(f)", points_y)
                                        .color(egui::Color32::from_rgb(0, 130, 0)),
                                );
                                plot_ui.line(
                                    Line::new("w0y theory", theory_y)
                                        .color(egui::Color32::from_rgb(100, 200, 100))
                                        .style(egui_plot::LineStyle::Dashed { length: 10.0 }),
                                );
                            });
                    });
                });

                ui.add_space(20.0);
                ui.separator();

                // Plot 2: w0 vs Wavelength with models
                ui.heading("📈 2. Beam Waist w0 vs Wavelength (with models)");
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    // X-axis plot
                    ui.vertical(|ui| {
                        ui.label("X-axis");
                        let measured_x: PlotPoints = diag
                            .wavelengths_um
                            .iter()
                            .zip(diag.w0x_mm.iter())
                            .map(|(l, w)| [*l, *w])
                            .collect();

                        let fit_x: PlotPoints = diag
                            .wavelengths_um
                            .iter()
                            .zip(diag.w0_fit_x_mm.iter())
                            .map(|(l, w)| [*l, *w])
                            .collect();

                        let theory_x: PlotPoints = diag
                            .wavelengths_um
                            .iter()
                            .zip(diag.w0_theory_x_mm.iter())
                            .map(|(l, w)| [*l, *w])
                            .collect();

                        Plot::new("w0x_vs_wavelength")
                            .legend(Legend::default())
                            .height(300.0)
                            .width(550.0)
                            .x_axis_label("Wavelength λ (µm)")
                            .y_axis_label("w0x (mm)")
                            .show(ui, |plot_ui| {
                                plot_ui.line(
                                    Line::new("Measured", measured_x).color(egui::Color32::BLUE),
                                );
                                plot_ui.line(
                                    Line::new(format!("Fit: w0={:.3}·λ", diag.a_x * 1e3), fit_x)
                                        .color(egui::Color32::RED)
                                        .style(egui_plot::LineStyle::Dashed { length: 10.0 }),
                                );
                                plot_ui.line(
                                    Line::new(
                                        format!("Theory (D_eff={:.2}mm)", diag.d_eff_x_theory_mm),
                                        theory_x,
                                    )
                                    .color(egui::Color32::from_rgb(255, 0, 255))
                                    .style(egui_plot::LineStyle::Dotted { spacing: 5.0 }),
                                );
                            });
                    });

                    ui.add_space(10.0);

                    // Y-axis plot
                    ui.vertical(|ui| {
                        ui.label("Y-axis");
                        let measured_y: PlotPoints = diag
                            .wavelengths_um
                            .iter()
                            .zip(diag.w0y_mm.iter())
                            .map(|(l, w)| [*l, *w])
                            .collect();

                        let fit_y: PlotPoints = diag
                            .wavelengths_um
                            .iter()
                            .zip(diag.w0_fit_y_mm.iter())
                            .map(|(l, w)| [*l, *w])
                            .collect();

                        let theory_y: PlotPoints = diag
                            .wavelengths_um
                            .iter()
                            .zip(diag.w0_theory_y_mm.iter())
                            .map(|(l, w)| [*l, *w])
                            .collect();

                        Plot::new("w0y_vs_wavelength")
                            .legend(Legend::default())
                            .height(300.0)
                            .width(550.0)
                            .x_axis_label("Wavelength λ (µm)")
                            .y_axis_label("w0y (mm)")
                            .show(ui, |plot_ui| {
                                plot_ui.line(
                                    Line::new("Measured", measured_y)
                                        .color(egui::Color32::from_rgb(0, 130, 0)),
                                );
                                plot_ui.line(
                                    Line::new(format!("Fit: w0={:.3}·λ", diag.a_y * 1e3), fit_y)
                                        .color(egui::Color32::RED)
                                        .style(egui_plot::LineStyle::Dashed { length: 10.0 }),
                                );
                                plot_ui.line(
                                    Line::new(
                                        format!("Theory (D_eff={:.2}mm)", diag.d_eff_y_theory_mm),
                                        theory_y,
                                    )
                                    .color(egui::Color32::from_rgb(255, 0, 255))
                                    .style(egui_plot::LineStyle::Dotted { spacing: 5.0 }),
                                );
                            });
                    });
                });

                ui.add_space(20.0);
                ui.separator();

                // Plot 3: π·w0/λ ratio
                ui.heading("📈 3. Ratio π·w0/λ (should be constant)");
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    // X-axis
                    ui.vertical(|ui| {
                        ui.label("X-axis");
                        let ratio_x_points: PlotPoints = diag
                            .wavelengths_um
                            .iter()
                            .zip(diag.ratio_x.iter())
                            .map(|(l, r)| [*l, *r])
                            .collect();

                        Plot::new("ratio_x")
                            .legend(Legend::default())
                            .height(300.0)
                            .width(550.0)
                            .x_axis_label("Wavelength λ (µm)")
                            .y_axis_label("π·w0/λ")
                            .show(ui, |plot_ui| {
                                plot_ui.line(
                                    Line::new("Measured", ratio_x_points)
                                        .color(egui::Color32::BLUE),
                                );
                                // Add mean line (all frequencies)
                                let mean_line: PlotPoints = vec![
                                    [diag.wavelengths_um[0], diag.ratio_x_mean],
                                    [*diag.wavelengths_um.last().unwrap(), diag.ratio_x_mean],
                                ]
                                .into();
                                plot_ui.line(
                                    Line::new(
                                        format!("Mean (all): {:.4}", diag.ratio_x_mean),
                                        mean_line,
                                    )
                                    .color(egui::Color32::RED)
                                    .style(egui_plot::LineStyle::Dashed { length: 10.0 }),
                                );

                                // Add filtered mean line (< 1 THz)
                                let filtered_mean_line: PlotPoints = vec![
                                    [diag.wavelengths_um[0], diag.ratio_x_mean_filtered],
                                    [
                                        *diag.wavelengths_um.last().unwrap(),
                                        diag.ratio_x_mean_filtered,
                                    ],
                                ]
                                .into();
                                plot_ui.line(
                                    Line::new(
                                        format!("Mean (<1THz): {:.4}", diag.ratio_x_mean_filtered),
                                        filtered_mean_line,
                                    )
                                    .color(egui::Color32::from_rgb(255, 140, 0))
                                    .style(egui_plot::LineStyle::Solid),
                                );
                            });
                    });

                    ui.add_space(10.0);

                    // Y-axis
                    ui.vertical(|ui| {
                        ui.label("Y-axis");
                        let ratio_y_points: PlotPoints = diag
                            .wavelengths_um
                            .iter()
                            .zip(diag.ratio_y.iter())
                            .map(|(l, r)| [*l, *r])
                            .collect();

                        Plot::new("ratio_y")
                            .legend(Legend::default())
                            .height(300.0)
                            .width(550.0)
                            .x_axis_label("Wavelength λ (µm)")
                            .y_axis_label("π·w0/λ")
                            .show(ui, |plot_ui| {
                                plot_ui.line(
                                    Line::new("Measured", ratio_y_points)
                                        .color(egui::Color32::from_rgb(0, 130, 0)),
                                );
                                // Add mean line (all frequencies)
                                let mean_line: PlotPoints = vec![
                                    [diag.wavelengths_um[0], diag.ratio_y_mean],
                                    [*diag.wavelengths_um.last().unwrap(), diag.ratio_y_mean],
                                ]
                                .into();
                                plot_ui.line(
                                    Line::new(
                                        format!("Mean (all): {:.4}", diag.ratio_y_mean),
                                        mean_line,
                                    )
                                    .color(egui::Color32::RED)
                                    .style(egui_plot::LineStyle::Dashed { length: 10.0 }),
                                );

                                // Add filtered mean line (< 1 THz)
                                let filtered_mean_line: PlotPoints = vec![
                                    [diag.wavelengths_um[0], diag.ratio_y_mean_filtered],
                                    [
                                        *diag.wavelengths_um.last().unwrap(),
                                        diag.ratio_y_mean_filtered,
                                    ],
                                ]
                                .into();
                                plot_ui.line(
                                    Line::new(
                                        format!("Mean (<1THz): {:.4}", diag.ratio_y_mean_filtered),
                                        filtered_mean_line,
                                    )
                                    .color(egui::Color32::from_rgb(255, 140, 0))
                                    .style(egui_plot::LineStyle::Solid),
                                );
                            });
                    });
                });

                ui.add_space(20.0);
                ui.separator();

                // Plot 4: D_eff
                ui.heading("📈 4. Effective Aperture D_eff(λ)");
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    // X-axis
                    ui.vertical(|ui| {
                        ui.label("X-axis");
                        let d_eff_x_points: PlotPoints = diag
                            .wavelengths_um
                            .iter()
                            .zip(diag.d_eff_x_mm.iter())
                            .map(|(l, d)| [*l, *d])
                            .collect();

                        Plot::new("d_eff_x")
                            .legend(Legend::default())
                            .height(300.0)
                            .width(550.0)
                            .x_axis_label("Wavelength λ (µm)")
                            .y_axis_label("D_eff (mm)")
                            .show(ui, |plot_ui| {
                                plot_ui.line(
                                    Line::new("Measured", d_eff_x_points)
                                        .color(egui::Color32::BLUE),
                                );
                                // Mean line (all frequencies)
                                let mean_line: PlotPoints = vec![
                                    [diag.wavelengths_um[0], diag.d_eff_x_mean_mm],
                                    [*diag.wavelengths_um.last().unwrap(), diag.d_eff_x_mean_mm],
                                ]
                                .into();
                                plot_ui.line(
                                    Line::new(
                                        format!("Mean (all): {:.2} mm", diag.d_eff_x_mean_mm),
                                        mean_line,
                                    )
                                    .color(egui::Color32::RED)
                                    .style(egui_plot::LineStyle::Dashed { length: 10.0 }),
                                );

                                // Filtered mean line (< 1 THz)
                                let filtered_mean_line: PlotPoints = vec![
                                    [diag.wavelengths_um[0], diag.d_eff_x_mean_filtered_mm],
                                    [
                                        *diag.wavelengths_um.last().unwrap(),
                                        diag.d_eff_x_mean_filtered_mm,
                                    ],
                                ]
                                .into();
                                plot_ui.line(
                                    Line::new(
                                        format!(
                                            "Mean (<1THz): {:.2} mm",
                                            diag.d_eff_x_mean_filtered_mm
                                        ),
                                        filtered_mean_line,
                                    )
                                    .color(egui::Color32::from_rgb(255, 140, 0))
                                    .style(egui_plot::LineStyle::Solid),
                                );
                                // Theory line
                                let theory_line: PlotPoints = vec![
                                    [diag.wavelengths_um[0], diag.d_eff_x_theory_mm],
                                    [*diag.wavelengths_um.last().unwrap(), diag.d_eff_x_theory_mm],
                                ]
                                .into();
                                plot_ui.line(
                                    Line::new(
                                        format!("Theory @ 1THz: {:.2} mm", diag.d_eff_x_theory_mm),
                                        theory_line,
                                    )
                                    .color(egui::Color32::from_rgb(255, 0, 255))
                                    .style(egui_plot::LineStyle::Dotted { spacing: 5.0 }),
                                );
                            });
                    });

                    ui.add_space(10.0);

                    // Y-axis
                    ui.vertical(|ui| {
                        ui.label("Y-axis");
                        let d_eff_y_points: PlotPoints = diag
                            .wavelengths_um
                            .iter()
                            .zip(diag.d_eff_y_mm.iter())
                            .map(|(l, d)| [*l, *d])
                            .collect();

                        Plot::new("d_eff_y")
                            .legend(Legend::default())
                            .height(300.0)
                            .width(550.0)
                            .x_axis_label("Wavelength λ (µm)")
                            .y_axis_label("D_eff (mm)")
                            .show(ui, |plot_ui| {
                                plot_ui.line(
                                    Line::new("Measured", d_eff_y_points)
                                        .color(egui::Color32::from_rgb(0, 130, 0)),
                                );
                                // Mean line (all frequencies)
                                let mean_line: PlotPoints = vec![
                                    [diag.wavelengths_um[0], diag.d_eff_y_mean_mm],
                                    [*diag.wavelengths_um.last().unwrap(), diag.d_eff_y_mean_mm],
                                ]
                                .into();
                                plot_ui.line(
                                    Line::new(
                                        format!("Mean (all): {:.2} mm", diag.d_eff_y_mean_mm),
                                        mean_line,
                                    )
                                    .color(egui::Color32::RED)
                                    .style(egui_plot::LineStyle::Dashed { length: 10.0 }),
                                );

                                // Filtered mean line (< 1 THz)
                                let filtered_mean_line: PlotPoints = vec![
                                    [diag.wavelengths_um[0], diag.d_eff_y_mean_filtered_mm],
                                    [
                                        *diag.wavelengths_um.last().unwrap(),
                                        diag.d_eff_y_mean_filtered_mm,
                                    ],
                                ]
                                .into();
                                plot_ui.line(
                                    Line::new(
                                        format!(
                                            "Mean (<1THz): {:.2} mm",
                                            diag.d_eff_y_mean_filtered_mm
                                        ),
                                        filtered_mean_line,
                                    )
                                    .color(egui::Color32::from_rgb(255, 140, 0))
                                    .style(egui_plot::LineStyle::Solid),
                                );
                                // Theory line
                                let theory_line: PlotPoints = vec![
                                    [diag.wavelengths_um[0], diag.d_eff_y_theory_mm],
                                    [*diag.wavelengths_um.last().unwrap(), diag.d_eff_y_theory_mm],
                                ]
                                .into();
                                plot_ui.line(
                                    Line::new(
                                        format!("Theory @ 1THz: {:.2} mm", diag.d_eff_y_theory_mm),
                                        theory_line,
                                    )
                                    .color(egui::Color32::from_rgb(255, 0, 255))
                                    .style(egui_plot::LineStyle::Dotted { spacing: 5.0 }),
                                );
                            });
                    });
                });

                ui.add_space(20.0);
                ui.separator();

                // Plot 5: Rayleigh range z_R
                ui.heading("📈 5. Rayleigh Range z_R(λ)");
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    // X-axis
                    ui.vertical(|ui| {
                        ui.label("X-axis");
                        let measured_x: PlotPoints = diag
                            .wavelengths_um
                            .iter()
                            .zip(diag.z_r_x_mm.iter())
                            .map(|(l, z)| [*l, *z])
                            .collect();

                        let theory_x: PlotPoints = diag
                            .wavelengths_um
                            .iter()
                            .zip(diag.z_r_theory_x_mm.iter())
                            .map(|(l, z)| [*l, *z])
                            .collect();

                        let fit_x: PlotPoints = diag
                            .wavelengths_um
                            .iter()
                            .zip(diag.z_r_fit_x_mm.iter())
                            .map(|(l, z)| [*l, *z])
                            .collect();

                        Plot::new("z_r_x")
                            .legend(Legend::default())
                            .height(300.0)
                            .width(550.0)
                            .x_axis_label("Wavelength λ (µm)")
                            .y_axis_label("z_R (mm)")
                            .show(ui, |plot_ui| {
                                plot_ui.line(
                                    Line::new("Measured (π·w0²/λ)", measured_x)
                                        .color(egui::Color32::BLUE),
                                );
                                plot_ui.line(
                                    Line::new("Theory (D_eff const @ 1THz)", theory_x)
                                        .color(egui::Color32::from_rgb(255, 0, 255))
                                        .style(egui_plot::LineStyle::Dotted { spacing: 5.0 }),
                                );
                                plot_ui.line(
                                    Line::new("Fit (π·A²·λ linear)", fit_x)
                                        .color(egui::Color32::RED)
                                        .style(egui_plot::LineStyle::Dashed { length: 10.0 }),
                                );
                            });
                    });

                    ui.add_space(10.0);

                    // Y-axis
                    ui.vertical(|ui| {
                        ui.label("Y-axis");
                        let measured_y: PlotPoints = diag
                            .wavelengths_um
                            .iter()
                            .zip(diag.z_r_y_mm.iter())
                            .map(|(l, z)| [*l, *z])
                            .collect();

                        let theory_y: PlotPoints = diag
                            .wavelengths_um
                            .iter()
                            .zip(diag.z_r_theory_y_mm.iter())
                            .map(|(l, z)| [*l, *z])
                            .collect();

                        let fit_y: PlotPoints = diag
                            .wavelengths_um
                            .iter()
                            .zip(diag.z_r_fit_y_mm.iter())
                            .map(|(l, z)| [*l, *z])
                            .collect();

                        Plot::new("z_r_y")
                            .legend(Legend::default())
                            .height(300.0)
                            .width(550.0)
                            .x_axis_label("Wavelength λ (µm)")
                            .y_axis_label("z_R (mm)")
                            .show(ui, |plot_ui| {
                                plot_ui.line(
                                    Line::new("Measured (π·w0²/λ)", measured_y)
                                        .color(egui::Color32::from_rgb(0, 130, 0)),
                                );
                                plot_ui.line(
                                    Line::new("Theory (D_eff const @ 1THz)", theory_y)
                                        .color(egui::Color32::from_rgb(255, 0, 255))
                                        .style(egui_plot::LineStyle::Dotted { spacing: 5.0 }),
                                );
                                plot_ui.line(
                                    Line::new("Fit (π·A²·λ linear)", fit_y)
                                        .color(egui::Color32::RED)
                                        .style(egui_plot::LineStyle::Dashed { length: 10.0 }),
                                );
                            });
                    });
                });
            } else {
                ui.label("No diagnostic data available");
            }
        });
    }

    fn recalculate_diagnostics(&mut self) {
        // Parse user inputs
        let focal_length = self.focal_length_mm.parse::<f64>().ok();
        let ref_freq = self.ref_frequency_thz.parse::<f64>().ok();
        let aperture = if self.aperture_d_mm.is_empty() {
            None
        } else {
            self.aperture_d_mm.parse::<f64>().ok()
        };

        // Use defaults if parsing fails
        let focal_length = focal_length.unwrap_or(152.4);
        let ref_freq = ref_freq.unwrap_or(1.0);

        // Recompute diagnostics
        match DiagnosticResults::compute_with_params(
            &self.frequencies_thz,
            &self.w0x_mm,
            &self.w0y_mm,
            focal_length,
            ref_freq,
            aperture,
        ) {
            Ok(new_diag) => {
                self.diagnostics = Some(new_diag);
            }
            Err(e) => {
                eprintln!("[ERROR] Failed to recalculate diagnostics: {}", e);
            }
        }
    }
}
