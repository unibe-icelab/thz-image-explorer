//! Source code example of how to create your own widget.
//! This is meant to be read as a tutorial, hence the plethora of comments.

use std::f64::consts::PI;

use bevy_egui::egui;
use bevy_egui::egui::epaint::{FontFamily, PathShape};
use bevy_egui::egui::Shape::Path;
use bevy_egui::egui::{pos2, Align2, Color32, FontId, Pos2, Stroke};

pub fn map(x: &f64, min: f64, max: f64, min_i: f64, max_i: f64) -> f64 {
    (*x - min_i) / (max_i - min_i) * (max - min) + min
}

pub fn gauge_ui(
    ui: &mut egui::Ui,
    value: &f64,
    min_i: f64,
    max_i: f64,
    size: f64,
    suffix: &str,
    text: &str,
) -> egui::Response {
    let min = -45;
    let max = 150;

    let desired_size = egui::vec2(size as f32, size as f32);

    let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::click());

    if ui.is_rect_visible(rect) {
        let visuals = ui.style().noninteractive();
        let rect = rect.expand(visuals.expansion);

        let mut color_values: Vec<Pos2> = Vec::new();
        let mut white_values: Vec<Pos2> = Vec::new();
        let mut major_tick_values: Vec<Vec<Pos2>> = Vec::new();
        let mut minor_tick_values: Vec<Vec<Pos2>> = Vec::new();
        let mut r = rect.height() / 2.0;
        let width_out = 2.0;
        let width_in = 10.0;

        let log_scale = suffix.contains("mbar");

        // Populate white arc
        for phi in min..=max {
            let phi = (phi as f64) / 180.0 * PI;
            white_values.push(Pos2 {
                x: rect.center().x - r * (phi as f32).cos(),
                y: rect.center().y - r * (phi as f32).sin(),
            });
        }

        // Generate ticks
        let mut generate_ticks = |step: usize, is_major: bool| {
            for phi in (min..=max).step_by(step) {
                let phi = (phi as f64) / 180.0 * PI;
                let tick_length = if is_major {
                    size as f32 * 0.035 + width_out
                } else {
                    size as f32 * 0.01 + width_out
                };
                let tick_pos = vec![
                    Pos2 {
                        x: rect.center().x - r * (phi as f32).cos(),
                        y: rect.center().y - r * (phi as f32).sin(),
                    },
                    Pos2 {
                        x: rect.center().x - (r + tick_length) * (phi as f32).cos(),
                        y: rect.center().y - (r + tick_length) * (phi as f32).sin(),
                    },
                ];
                if is_major {
                    major_tick_values.push(tick_pos);
                } else {
                    minor_tick_values.push(tick_pos);
                }
            }
        };

        if log_scale {
            // Logarithmic scaling for mbar
            generate_ticks(30, true); // Major ticks every 30 degrees
            generate_ticks(10, false); // Minor ticks every 10 degrees
        } else {
            // Linear scaling
            generate_ticks(50, true); // Major ticks every 50 degrees
            generate_ticks(10, false); // Minor ticks every 10 degrees
        }

        // generate color arc

        if log_scale {
            r = r - width_in / 2.0 - width_out / 2.0;
            for phi in min..(map(
                &value.log10(),
                min as f64,
                max as f64,
                min_i.log10(),
                max_i.log10(),
            ) as i32)
            {
                let phi = (phi as f64) / 180.0 * PI;
                color_values.push(Pos2 {
                    x: rect.center().x - r * (phi as f32).cos(),
                    y: rect.center().y - r * (phi as f32).sin(),
                });
            }
        } else {
            r = r - width_in / 2.0 - width_out / 2.0;
            for phi in min..(map(value, min as f64, max as f64, min_i, max_i) as i32) {
                let phi = (phi as f64) / 180.0 * PI;
                color_values.push(Pos2 {
                    x: rect.center().x - r * (phi as f32).cos(),
                    y: rect.center().y - r * (phi as f32).sin(),
                });
            }
        }

        // ui.painter()
        //    .circle(center, radius, visuals.bg_fill, visuals.bg_stroke);
        
        let color = if ui.visuals().dark_mode {
            Color32::WHITE
        } else {
            Color32::BLACK
        };

        let values_color = if (*value as f32) > min as f32 + (max - min) as f32 * 0.25
            || (*value as f32) < min as f32 + (max - min) as f32 * 0.75
        {
            Color32::GREEN
        } else if *value as f32 <= min as f32 + (max - min) as f32 * 0.25 {
            Color32::YELLOW
        } else {
            Color32::RED
        };

        ui.painter().add(Path(PathShape::line(
            white_values,
            Stroke::new(width_out, color),
        )));
        ui.painter().add(Path(PathShape::line(
            color_values,
            Stroke::new(width_in, values_color),
        )));

        for tick in major_tick_values.iter() {
            ui.painter().add(Path(PathShape::line(
                tick.clone(),
                Stroke::new(width_out, color),
            )));
        }

        let value_size = if log_scale {
            ui.painter()
                .text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    format!("{:.1e}", value),
                    FontId::new(14.0, FontFamily::Monospace),
                    color,
                )
                .size()
        } else {
            ui.painter()
                .text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    format!("{:.1}", value),
                    FontId::new(14.0, FontFamily::Monospace),
                    color,
                )
                .size()
        };

        let suffix_pos = pos2(rect.center().x, rect.center().y + value_size.y);
        ui.painter().text(
            suffix_pos,
            Align2::CENTER_CENTER,
            suffix.to_string(),
            FontId::new(12.0, FontFamily::Monospace),
            color,
        );

        let text_pos = pos2(rect.center().x, rect.center().y - value_size.y);
        ui.painter().text(
            text_pos,
            Align2::CENTER_CENTER,
            text,
            FontId::new(12.0, FontFamily::Monospace),
            color,
        );
    }

    response
}

// A wrapper that allows the more idiomatic usage pattern: `ui.add(gauge(&temperatue, "temperature"))`
pub fn gauge<'a>(
    value: &'a f64,
    min: f64,
    max: f64,
    size: f64,
    suffix: &'a str,
    text: &'a str,
) -> impl egui::Widget + 'a {
    move |ui: &mut egui::Ui| gauge_ui(ui, value, min, max, size, suffix, text)
}
