use bevy_egui::egui;
use egui::{Color32, ColorImage};
use egui_plot::{PlotImage, PlotPoint};
use serde::{Deserialize, Serialize};

use crate::psf_tool::curve_fitting::CurveFits;

/// PSF Visualizer window state
#[derive(Clone, Serialize, Deserialize)]
pub struct PsfVisualizerWindow {
    pub frequency_thz: f64,
    #[serde(skip)]
    cached_image: Option<(f64, ColorImage)>, // (frequency, image)
    #[serde(skip)]
    texture: Option<egui::TextureHandle>, // kept alive to prevent GPU release before render
}

impl Default for PsfVisualizerWindow {
    fn default() -> Self {
        Self {
            frequency_thz: 1.0, // Start at 1 THz
            cached_image: None,
            texture: None,
        }
    }
}

impl std::fmt::Debug for PsfVisualizerWindow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PsfVisualizerWindow")
            .field("frequency_thz", &self.frequency_thz)
            .finish_non_exhaustive()
    }
}

impl PsfVisualizerWindow {
    pub fn new() -> Self {
        Self::default()
    }

    /// Generate 2D Gaussian PSF at given frequency
    fn generate_psf_image(
        curve_fits: &CurveFits,
        frequency_thz: f64,
        resolution: usize,
    ) -> ColorImage {
        // Evaluate fits at the given frequency
        let freqs = vec![frequency_thz];
        let wx = curve_fits.wx_fit.evaluate(&freqs)[0]; // mm
        let wy = curve_fits.wy_fit.evaluate(&freqs)[0]; // mm
        let x0 = curve_fits.x0_fit.evaluate_const_extrap(&freqs)[0]; // mm
        let y0 = curve_fits.y0_fit.evaluate_const_extrap(&freqs)[0]; // mm

        // Determine spatial extent (show ±4σ)
        let extent_x = 4.0 * wx;
        let extent_y = 4.0 * wy;
        let x_min = x0 - extent_x;
        let x_max = x0 + extent_x;
        let y_min = y0 - extent_y;
        let y_max = y0 + extent_y;

        // Generate 2D Gaussian
        let mut pixels = Vec::with_capacity(resolution * resolution);
        let mut max_intensity = 0.0f64;

        // First pass: compute intensities
        let mut intensities = Vec::with_capacity(resolution * resolution);
        for i in 0..resolution {
            for j in 0..resolution {
                let x = x_min + (j as f64 / (resolution - 1) as f64) * (x_max - x_min);
                let y = y_max - (i as f64 / (resolution - 1) as f64) * (y_max - y_min); // Flip y

                // 2D Gaussian: I(x,y) = I0 * exp(-2*((x-x0)²/wx² + (y-y0)²/wy²))
                let dx = x - x0;
                let dy = y - y0;
                let intensity = (-2.0 * (dx * dx / (wx * wx) + dy * dy / (wy * wy))).exp();

                intensities.push(intensity);
                if intensity > max_intensity {
                    max_intensity = intensity;
                }
            }
        }

        // Second pass: normalize and convert to colors
        for intensity in intensities {
            let normalized = (intensity / max_intensity).max(0.0).min(1.0);
            let color = intensity_to_color(normalized);
            pixels.push(color);
        }

        ColorImage {
            size: [resolution, resolution],
            source_size: egui::Vec2::new(resolution as f32, resolution as f32),
            pixels,
        }
    }

    /// Show content in a given `Ui` (for embedding in an `egui::Window`).
    pub fn show_in_panel(&mut self, ui: &mut egui::Ui, curve_fits: &CurveFits) {
        self.show_content(ui, curve_fits);
    }

    /// Show the PSF visualizer window as a native viewport
    pub fn show(&mut self, ctx: &egui::Context, curve_fits: &CurveFits) {
        egui::CentralPanel::default().show(ctx, |ui| {
            self.show_content(ui, curve_fits);
        });
    }

    fn show_content(&mut self, ui: &mut egui::Ui, curve_fits: &CurveFits) {
        ui.heading("2D PSF at Selected Frequency");
            ui.add_space(10.0);

            // Frequency slider
            ui.horizontal(|ui| {
                ui.label("Frequency:");
                let response = ui.add(
                    egui::Slider::new(&mut self.frequency_thz, 0.1..=10.0)
                        .text("THz")
                        .logarithmic(true)
                        .step_by(0.01),
                );

                // Show wavelength
                let wavelength_um = 299.792458 / self.frequency_thz;
                ui.label(format!("({:.1} µm)", wavelength_um));

                // Regenerate image if frequency changed
                if response.changed() {
                    self.cached_image = None;
                }
            });

            ui.add_space(5.0);

            // Generate or use cached image; keep TextureHandle alive across frames
            // to prevent bevy_egui from releasing the GPU texture before the render pass
            let needs_new = match &self.cached_image {
                Some((cached_freq, _)) => (cached_freq - self.frequency_thz).abs() >= 1e-6,
                None => true,
            };
            if needs_new || self.texture.is_none() {
                let img = if needs_new {
                    let img = Self::generate_psf_image(curve_fits, self.frequency_thz, 256);
                    self.cached_image = Some((self.frequency_thz, img.clone()));
                    img
                } else {
                    self.cached_image.as_ref().unwrap().1.clone()
                };
                self.texture = Some(ui.ctx().load_texture("psf_image", img, egui::TextureOptions::LINEAR));
            }
            let texture_id = self.texture.as_ref().map(|t| t.id());

            // Display PSF information
            let freqs = vec![self.frequency_thz];
            let wx = curve_fits.wx_fit.evaluate(&freqs)[0];
            let wy = curve_fits.wy_fit.evaluate(&freqs)[0];
            let x0 = curve_fits.x0_fit.evaluate_const_extrap(&freqs)[0];
            let y0 = curve_fits.y0_fit.evaluate_const_extrap(&freqs)[0];

            ui.horizontal(|ui| {
                ui.label(format!("w_x = {:.3} mm", wx));
                ui.separator();
                ui.label(format!("w_y = {:.3} mm", wy));
                ui.separator();
                ui.label(format!("x₀ = {:.3} mm", x0));
                ui.separator();
                ui.label(format!("y₀ = {:.3} mm", y0));
            });

            ui.add_space(10.0);

            // Display the PSF image
            let available_size = ui.available_size();
            let plot_size = available_size.x.min(available_size.y - 20.0);

            // Calculate spatial extent for axes
            let extent_x = 4.0 * wx;
            let extent_y = 4.0 * wy;
            let x_min = x0 - extent_x;
            let x_max = x0 + extent_x;
            let y_min = y0 - extent_y;
            let y_max = y0 + extent_y;

            egui_plot::Plot::new("psf_plot")
                .width(plot_size)
                .height(plot_size)
                .data_aspect(1.0)
                .x_axis_label("x [mm]")
                .y_axis_label("y [mm]")
                .show(ui, |plot_ui| {
                    if let Some(tex_id) = texture_id {
                        plot_ui.image(PlotImage::new(
                            "PSF",
                            tex_id,
                            PlotPoint::new((x_min + x_max) / 2.0, (y_min + y_max) / 2.0),
                            egui::Vec2::new((x_max - x_min) as f32, (y_max - y_min) as f32),
                        ));
                    }
                });

            ui.add_space(5.0);
            ui.horizontal(|ui| {
                ui.label("Colormap: Blue (low) -> Yellow -> Red (high)");
            });
    }
}

/// Convert intensity [0, 1] to color (jet-like colormap)
fn intensity_to_color(value: f64) -> Color32 {
    let value = value.clamp(0.0, 1.0);

    // Simple jet-like colormap
    let r: u8;
    let g: u8;
    let b: u8;

    if value < 0.25 {
        // Blue to Cyan
        let t = value / 0.25;
        r = 0;
        g = (t * 255.0) as u8;
        b = 255;
    } else if value < 0.5 {
        // Cyan to Green
        let t = (value - 0.25) / 0.25;
        r = 0;
        g = 255;
        b = ((1.0 - t) * 255.0) as u8;
    } else if value < 0.75 {
        // Green to Yellow
        let t = (value - 0.5) / 0.25;
        r = (t * 255.0) as u8;
        g = 255;
        b = 0;
    } else {
        // Yellow to Red
        let t = (value - 0.75) / 0.25;
        r = 255;
        g = ((1.0 - t) * 255.0) as u8;
        b = 0;
    }

    Color32::from_rgb(r, g, b)
}
