//! Source code example of how to create your own widget.
//! This is meant to be read as a tutorial, hence the plethora of comments.

use std::f64::consts::PI;

use eframe::egui;
use eframe::egui::Shape::Path;
use eframe::egui::{pos2, Align2, Color32, FontId, Pos2, Stroke, Visuals};
use eframe::epaint::{FontFamily, PathShape};

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
    // Widget code can be broken up in four steps:
    //  1. Decide a size for the widget
    //  2. Allocate space for it
    //  3. Handle interactions with the widget (if any)
    //  4. Paint the widget

    // 1. Deciding widget size:
    // You can query the `ui` how much space is available,
    // but in this example we have a fixed size widget based on the height of a standard button:
    let desired_size = egui::vec2(size as f32, size as f32);

    // 2. Allocating space:
    // This is where we get a region of the screen assigned.
    // We also tell the Ui to sense clicks in the allocated region.
    let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::click());

    // Attach some meta-data to the response which can be used by screen readers:
    //response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, *on, ""));

    // 4. Paint!
    // Make sure we need to paint:
    if ui.is_rect_visible(rect) {
        // Let's ask for a simple animation from egui.
        // egui keeps track of changes in the boolean associated with the id and
        // returns an animated value in the 0-1 range for how much "on" we are.
        // We will follow the current style by asking
        // "how should something that is being interacted with be painted?".
        // This will, for instance, give us different colors when the widget is hovered or clicked.
        let visuals = ui.style().noninteractive();
        // All coordinates are in absolute screen coordinates so we use `rect` to place the elements.
        let rect = rect.expand(visuals.expansion);

        let mut color_values: Vec<Pos2> = Vec::new();
        let mut white_values: Vec<Pos2> = Vec::new();
        let mut major_tick_values: Vec<Vec<Pos2>> = Vec::new();
        let mut minor_tick_values: Vec<Vec<Pos2>> = Vec::new();
        let mut r = rect.height() / 2.0;
        let width_out = 2.0;
        let width_in = 10.0;
        for phi in min..=max {
            let phi = (phi as f64) / 180.0 * PI;
            white_values.push(Pos2 {
                x: rect.center().x - r * (phi as f32).cos(),
                y: rect.center().y - r * (phi as f32).sin(),
            });
        }

        // TODO: add function for cool steps
        for phi in (min..=max).step_by(50) {
            let phi = (phi as f64) / 180.0 * PI;
            let tick_pos = vec![
                Pos2 {
                    x: rect.center().x - r * (phi as f32).cos(),
                    y: rect.center().y - r * (phi as f32).sin(),
                },
                Pos2 {
                    x: rect.center().x - (r + size as f32 * 0.035 + width_out) * (phi as f32).cos(),
                    y: rect.center().y - (r + size as f32 * 0.035 + width_out) * (phi as f32).sin(),
                },
            ];
            major_tick_values.push(tick_pos);
        }
        for phi in (min..=max).step_by(10) {
            let phi = (phi as f64) / 180.0 * PI;
            let tick_pos = vec![
                Pos2 {
                    x: rect.center().x - r * (phi as f32).cos(),
                    y: rect.center().y - r * (phi as f32).sin(),
                },
                Pos2 {
                    x: rect.center().x - (r + size as f32 * 0.01 + width_out) * (phi as f32).cos(),
                    y: rect.center().y - (r + size as f32 * 0.01 + width_out) * (phi as f32).sin(),
                },
            ];
            minor_tick_values.push(tick_pos);
        }

        r = r - width_in / 2.0 - width_out / 2.0;
        for phi in min..(map(value, min as f64, max as f64, min_i, max_i) as i32) {
            let phi = (phi as f64) / 180.0 * PI;
            color_values.push(Pos2 {
                x: rect.center().x - r * (phi as f32).cos(),
                y: rect.center().y - r * (phi as f32).sin(),
            });
        }

        // ui.painter()
        //    .circle(center, radius, visuals.bg_fill, visuals.bg_stroke);

        let color = if ui.visuals() == &Visuals::dark() {
            Color32::WHITE
        } else {
            Color32::BLACK
        };

        let values_color: Color32;
        if (*value as f32) > min as f32 + (max - min) as f32 * 0.25
            || (*value as f32) < min as f32 + (max - min) as f32 * 0.75
        {
            values_color = Color32::GREEN;
        } else if *value as f32 <= min as f32 + (max - min) as f32 * 0.25 {
            values_color = Color32::YELLOW;
        } else {
            values_color = Color32::RED;
        }

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
        // for tick in minor_tick_values.iter() {
        //     ui.painter().add(Path(PathShape::line(
        //         tick.clone(),
        //         Stroke::new(width_out, color),
        //     )));
        // }

        let value_size = ui
            .painter()
            .text(
                rect.center(),
                Align2::CENTER_CENTER,
                format!("{:.1}", value),
                FontId::new(20.0, FontFamily::Monospace),
                color,
            )
            .size();
        let suffix_pos = pos2(rect.center().x, rect.center().y + value_size.y);
        ui.painter().text(
            suffix_pos,
            Align2::CENTER_CENTER,
            suffix.to_string(),
            FontId::new(15.0, FontFamily::Monospace),
            color,
        );

        let text_pos = pos2(rect.center().x, rect.center().y - value_size.y);
        ui.painter().text(
            text_pos,
            Align2::CENTER_CENTER,
            text,
            FontId::new(15.0, FontFamily::Monospace),
            color,
        );
        // Paint the circle, animating it from left to right with `how_on`:
        //let circle_x = egui::lerp((rect.left() + radius)..=(rect.right() - radius), how_on);
        //ui.painter()
        //    .circle(center, 0.75 * radius, visuals.bg_fill, visuals.fg_stroke);
    }

    // All done! Return the interaction response so the user can check what happened
    // (hovered, clicked, ...) and maybe show a tooltip:
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
