use bevy_egui::egui;
use egui_plot::{Line, Plot, PlotPoints};
use ndarray::Array2;

use crate::psf_tool::fitting::{compute_intensity, error_function};

/// Window for visualizing individual fits at each frequency
pub struct IndividualFitsWindow {
    selected_filter: usize,
    total_filters: usize,
}

impl IndividualFitsWindow {
    pub fn new(total_filters: usize) -> Self {
        Self {
            selected_filter: 0,
            total_filters,
        }
    }

    /// Update the total number of filters (called when filters are recomputed).
    pub fn update_total_filters(&mut self, total_filters: usize) {
        self.total_filters = total_filters;
        if self.selected_filter >= self.total_filters {
            self.selected_filter = self.total_filters.saturating_sub(1);
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn show_in_panel(
        &mut self,
        ui: &mut egui::Ui,
        popt_xs_left: &Array2<f64>,
        popt_xs_right: &Array2<f64>,
        popt_ys_left: &Array2<f64>,
        popt_ys_right: &Array2<f64>,
        filtered_traces_x_left: &[Array2<f64>],
        filtered_traces_x_right: &[Array2<f64>],
        filtered_traces_y_left: &[Array2<f64>],
        filtered_traces_y_right: &[Array2<f64>],
        x_positions_left: &[f64],
        x_positions_right: &[f64],
        y_positions_left: &[f64],
        y_positions_right: &[f64],
        center_frequencies: &[f64],
    ) {
        self.show_content(
            ui,
            popt_xs_left,
            popt_xs_right,
            popt_ys_left,
            popt_ys_right,
            filtered_traces_x_left,
            filtered_traces_x_right,
            filtered_traces_y_left,
            filtered_traces_y_right,
            x_positions_left,
            x_positions_right,
            y_positions_left,
            y_positions_right,
            center_frequencies,
        );
    }

    pub fn show(
        &mut self,
        ctx: &egui::Context,
        popt_xs_left: &Array2<f64>,
        popt_xs_right: &Array2<f64>,
        popt_ys_left: &Array2<f64>,
        popt_ys_right: &Array2<f64>,
        filtered_traces_x_left: &[Array2<f64>],
        filtered_traces_x_right: &[Array2<f64>],
        filtered_traces_y_left: &[Array2<f64>],
        filtered_traces_y_right: &[Array2<f64>],
        x_positions_left: &[f64],
        x_positions_right: &[f64],
        y_positions_left: &[f64],
        y_positions_right: &[f64],
        center_frequencies: &[f64],
    ) {
        egui::CentralPanel::default().show(ctx, |ui| {
            self.show_content(
                ui,
                popt_xs_left,
                popt_xs_right,
                popt_ys_left,
                popt_ys_right,
                filtered_traces_x_left,
                filtered_traces_x_right,
                filtered_traces_y_left,
                filtered_traces_y_right,
                x_positions_left,
                x_positions_right,
                y_positions_left,
                y_positions_right,
                center_frequencies,
            );
        });
    }

    #[allow(clippy::too_many_arguments)]
    fn show_content(
        &mut self,
        ui: &mut egui::Ui,
        popt_xs_left: &Array2<f64>,
        popt_xs_right: &Array2<f64>,
        popt_ys_left: &Array2<f64>,
        popt_ys_right: &Array2<f64>,
        filtered_traces_x_left: &[Array2<f64>],
        filtered_traces_x_right: &[Array2<f64>],
        filtered_traces_y_left: &[Array2<f64>],
        filtered_traces_y_right: &[Array2<f64>],
        x_positions_left: &[f64],
        x_positions_right: &[f64],
        y_positions_left: &[f64],
        y_positions_right: &[f64],
        center_frequencies: &[f64],
    ) {
        ui.heading("Individual Fits per Frequency");
        ui.add_space(10.0);

        // Frequency selector
        ui.horizontal(|ui| {
            ui.label("Frequency:");
            if ui.button("◀").clicked() && self.selected_filter > 0 {
                self.selected_filter -= 1;
            }

            ui.add(
                egui::Slider::new(&mut self.selected_filter, 0..=(self.total_filters - 1))
                    .show_value(false),
            );

            if ui.button("▶").clicked() && self.selected_filter < self.total_filters - 1 {
                self.selected_filter += 1;
            }

            ui.label(format!(
                "{:.3} THz ({}/{})",
                center_frequencies[self.selected_filter],
                self.selected_filter + 1,
                self.total_filters
            ));
        });

        ui.add_space(10.0);

        // Display fit parameters
        ui.horizontal(|ui| {
            ui.group(|ui| {
                ui.label("X-axis fit:");
                ui.label(format!(
                    "Left: x₀ = {:.3} mm, w = {:.3} mm | Right: x₀ = {:.3} mm, w = {:.3} mm",
                    popt_xs_left[[self.selected_filter, 0]],
                    popt_xs_left[[self.selected_filter, 1]],
                    popt_xs_right[[self.selected_filter, 0]],
                    popt_xs_right[[self.selected_filter, 1]]
                ));
            });
        });
        ui.horizontal(|ui| {
            ui.group(|ui| {
                ui.label("Y-axis fit:");
                ui.label(format!(
                    "Left: y₀ = {:.3} mm, w = {:.3} mm | Right: y₀ = {:.3} mm, w = {:.3} mm",
                    popt_ys_left[[self.selected_filter, 0]],
                    popt_ys_left[[self.selected_filter, 1]],
                    popt_ys_right[[self.selected_filter, 0]],
                    popt_ys_right[[self.selected_filter, 1]]
                ));
            });
        });

        ui.add_space(15.0);

        // Calculate available space for plots ONCE to avoid oscillation
        let available_height = ui.available_height();
        let plot_height = (available_height - 100.0) / 2.0; // Account for headings, spacing, and bottom margin

        // Get filtered traces and fit parameters for X
        let filtered_trace_x_left = &filtered_traces_x_left[self.selected_filter];
        let intensity_x_left = compute_intensity(filtered_trace_x_left);
        let x0_left = popt_xs_left[[self.selected_filter, 0]];
        let wx_left = popt_xs_left[[self.selected_filter, 1]];

        let filtered_trace_x_right = &filtered_traces_x_right[self.selected_filter];
        let intensity_x_right = compute_intensity(filtered_trace_x_right);
        let x0_right = popt_xs_right[[self.selected_filter, 0]];
        let wx_right = popt_xs_right[[self.selected_filter, 1]];

        // X-axis plots (left and right)
        ui.heading("X-axis");
        ui.columns(2, |columns| {
            // Left plot
            columns[0].vertical(|ui| {
                ui.label("Left side");
                Plot::new("x_left_fit")
                    .height(plot_height)
                    .min_size(egui::vec2(200.0, plot_height))
                    .allow_boxed_zoom(false)
                    .x_axis_label("Position [mm]")
                    .y_axis_label("Intensity")
                    .legend(egui_plot::Legend::default())
                    .show(ui, |plot_ui| {
                        // Data points
                        let data_points: PlotPoints = x_positions_left
                            .iter()
                            .zip(intensity_x_left.as_slice().unwrap().iter())
                            .map(|(&x, &i)| [x, i])
                            .collect();
                        plot_ui.points(
                            egui_plot::Points::new("Data", data_points)
                                .color(egui::Color32::from_rgb(31, 119, 180))
                                .radius(3.0),
                        );

                        // Fit line
                        let x_min = x_positions_left.first().copied().unwrap_or(0.0);
                        let x_max = x_positions_left.last().copied().unwrap_or(1.0);
                        let fit_points: PlotPoints = (0..100)
                            .map(|i| {
                                let x = x_min + (x_max - x_min) * i as f64 / 99.0;
                                let y = error_function(x, x0_left, wx_left);
                                [x, y]
                            })
                            .collect();
                        plot_ui.line(
                            Line::new("Fit", fit_points)
                                .color(egui::Color32::from_rgb(255, 127, 14))
                                .width(2.0),
                        );
                    });
            });

            // Right plot
            columns[1].vertical(|ui| {
                ui.label("Right side");
                Plot::new("x_right_fit")
                    .height(plot_height)
                    .min_size(egui::vec2(200.0, plot_height))
                    .allow_boxed_zoom(false)
                    .x_axis_label("Position [mm]")
                    .y_axis_label("Intensity")
                    .legend(egui_plot::Legend::default())
                    .show(ui, |plot_ui| {
                        // Data points
                        let data_points: PlotPoints = x_positions_right
                            .iter()
                            .zip(intensity_x_right.as_slice().unwrap().iter())
                            .map(|(&x, &i)| [x, i])
                            .collect();
                        plot_ui.points(
                            egui_plot::Points::new("Data", data_points)
                                .color(egui::Color32::from_rgb(31, 119, 180))
                                .radius(3.0),
                        );

                        // Fit line
                        let x_min = x_positions_right.first().copied().unwrap_or(0.0);
                        let x_max = x_positions_right.last().copied().unwrap_or(1.0);
                        let fit_points: PlotPoints = (0..100)
                            .map(|i| {
                                let x = x_min + (x_max - x_min) * i as f64 / 99.0;
                                let y = error_function(x, x0_right, wx_right);
                                [x, y]
                            })
                            .collect();
                        plot_ui.line(
                            Line::new("Fit", fit_points)
                                .color(egui::Color32::from_rgb(255, 127, 14))
                                .width(2.0),
                        );
                    });
            });
        });

        ui.add_space(15.0);

        // Get filtered traces and fit parameters for Y
        let filtered_trace_y_left = &filtered_traces_y_left[self.selected_filter];
        let intensity_y_left = compute_intensity(filtered_trace_y_left);
        let y0_left = popt_ys_left[[self.selected_filter, 0]];
        let wy_left = popt_ys_left[[self.selected_filter, 1]];

        let filtered_trace_y_right = &filtered_traces_y_right[self.selected_filter];
        let intensity_y_right = compute_intensity(filtered_trace_y_right);
        let y0_right = popt_ys_right[[self.selected_filter, 0]];
        let wy_right = popt_ys_right[[self.selected_filter, 1]];

        // Y-axis plots (left and right)
        ui.heading("Y-axis");
        ui.columns(2, |columns| {
            // Left plot
            columns[0].vertical(|ui| {
                ui.label("Left side");
                Plot::new("y_left_fit")
                    .height(plot_height)
                    .min_size(egui::vec2(200.0, plot_height))
                    .allow_boxed_zoom(false)
                    .x_axis_label("Position [mm]")
                    .y_axis_label("Intensity")
                    .legend(egui_plot::Legend::default())
                    .show(ui, |plot_ui| {
                        // Data points
                        let data_points: PlotPoints = y_positions_left
                            .iter()
                            .zip(intensity_y_left.as_slice().unwrap().iter())
                            .map(|(&y, &i)| [y, i])
                            .collect();
                        plot_ui.points(
                            egui_plot::Points::new("Data", data_points)
                                .color(egui::Color32::from_rgb(214, 39, 40))
                                .radius(3.0),
                        );

                        // Fit line
                        let y_min = y_positions_left.first().copied().unwrap_or(0.0);
                        let y_max = y_positions_left.last().copied().unwrap_or(1.0);
                        let fit_points: PlotPoints = (0..100)
                            .map(|i| {
                                let y = y_min + (y_max - y_min) * i as f64 / 99.0;
                                let val = error_function(y, y0_left, wy_left);
                                [y, val]
                            })
                            .collect();
                        plot_ui.line(
                            Line::new("Fit", fit_points)
                                .color(egui::Color32::from_rgb(255, 127, 14))
                                .width(2.0),
                        );
                    });
            });

            // Right plot
            columns[1].vertical(|ui| {
                ui.label("Right side");
                Plot::new("y_right_fit")
                    .height(plot_height)
                    .min_size(egui::vec2(200.0, plot_height))
                    .allow_boxed_zoom(false)
                    .x_axis_label("Position [mm]")
                    .y_axis_label("Intensity")
                    .legend(egui_plot::Legend::default())
                    .show(ui, |plot_ui| {
                        // Data points
                        let data_points: PlotPoints = y_positions_right
                            .iter()
                            .zip(intensity_y_right.as_slice().unwrap().iter())
                            .map(|(&y, &i)| [y, i])
                            .collect();
                        plot_ui.points(
                            egui_plot::Points::new("Data", data_points)
                                .color(egui::Color32::from_rgb(214, 39, 40))
                                .radius(3.0),
                        );

                        // Fit line
                        let y_min = y_positions_right.first().copied().unwrap_or(0.0);
                        let y_max = y_positions_right.last().copied().unwrap_or(1.0);
                        let fit_points: PlotPoints = (0..100)
                            .map(|i| {
                                let y = y_min + (y_max - y_min) * i as f64 / 99.0;
                                let val = error_function(y, y0_right, wy_right);
                                [y, val]
                            })
                            .collect();
                        plot_ui.line(
                            Line::new("Fit", fit_points)
                                .color(egui::Color32::from_rgb(255, 127, 14))
                                .width(2.0),
                        );
                    });
            });
        });
    }
}
