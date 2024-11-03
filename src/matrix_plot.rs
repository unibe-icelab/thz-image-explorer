use std::f64::{INFINITY, NEG_INFINITY};

use eframe::egui;
use eframe::egui::{
    pos2, vec2, Color32, ColorImage, FontId, RichText, Shape, Stroke, UiBuilder, Vec2,
};
use egui::TextureOptions;
use egui_plot::{Line, Plot, PlotImage, PlotPoint, PlotPoints};
use ndarray::{Array2, Axis};

#[derive(Debug, Clone)]
pub struct SelectedPixel {
    pub selected: bool,
    pub rect: Vec<[f64; 2]>,
    pub x: f64,
    pub y: f64,
    pub id: String,
}

impl Default for SelectedPixel {
    fn default() -> Self {
        SelectedPixel {
            selected: false,
            rect: vec![],
            x: 0.0,
            y: 0.0,
            id: "0000-0000".to_string(),
        }
    }
}

pub fn make_dummy() -> Array2<f32> {
    let width = 20;
    let height = 20;
    let data = Array2::from_shape_fn((width, height), |(i, _)| i as f32);
    data
}

pub fn color_from_intensity(
    i: &f32,
    max_intensity: &f64,
    cut_off: &f64,
    midpoint_position: &f32,
    bw: &bool,
) -> Color32 {
    // Normalize the intensity based on the midpoint and cut-off
    let normalized_y = *i / *max_intensity as f32;
    let hue = if normalized_y <= (*midpoint_position / 100.0) {
        // Lower section stretched
        (normalized_y / (*midpoint_position / 100.0)) * 0.5
    } else {
        // Upper section stretched
        0.5 + ((normalized_y - (*midpoint_position / 100.0)) / (1.0 - (*midpoint_position / 100.0)))
            * 0.5
    };
    if hue * 100.0 >= *cut_off as f32 {
        if *bw {
            egui::ecolor::Hsva {
                h: 0.0,
                s: 0.0,
                v: hue,
                a: 1.0,
            }
            .into()
        } else {
            egui::ecolor::Hsva {
                h: 0.667 - hue * 0.667, // Map to color hue (0 to 0.667 range)
                s: 1.0,
                v: 1.0,
                a: 1.0,
            }
            .into()
        }
    } else {
        egui::ecolor::Hsva {
            h: 0.667 - hue * 0.667, // Map to color hue (0 to 0.667 range)
            s: 1.0,
            v: 0.0,
            a: 0.0,
        }
        .into()
    }
}

fn colorbar_with_midpoint_slider(
    ui: &mut egui::Ui,
    width: &f64,
    height: &f64,
    midpoint_position: &mut f32,
    bw: &bool,
) {
    let triangle_radius = 10.0; // Radius for triangle sides

    ui.vertical(|ui| {
        ui.add_space(*height as f32 * 0.03);

        // Plot the colorbar
        let plot = Plot::new("colorbar")
            .height(*height as f32 * 0.92)
            .width(*width as f32 * 0.5)
            .show_axes([false, false])
            .set_margin_fraction(Vec2 { x: 0.0, y: 0.0 })
            .allow_zoom(false)
            .allow_scroll(false)
            .allow_boxed_zoom(false)
            .allow_drag(false)
            .show_grid(false)
            .show_x(false)
            .show_y(false);

        let mut img = egui::ColorImage::new([1, 100], Color32::TRANSPARENT);

        for y in 0..100 {
            let normalized_y = y as f32 / 100.0;
            let hue_position = if normalized_y <= (*midpoint_position / 100.0) {
                // Lower section stretched
                (normalized_y / (*midpoint_position / 100.0)) * 0.5
            } else {
                // Upper section stretched
                0.5 + ((normalized_y - (*midpoint_position / 100.0))
                    / (1.0 - (*midpoint_position / 100.0)))
                    * 0.5
            };
            if *bw {
                img[(0, y)] = egui::ecolor::Hsva {
                    h: 0.0,
                    s: 0.0,
                    v: hue_position,
                    a: 1.0,
                }
                .into();
            } else {
                img[(0, y)] = egui::ecolor::Hsva {
                    h: 0.667 - hue_position * 0.667, // Map to color hue (0 to 0.667 range)
                    s: 1.0,
                    v: 1.0,
                    a: 1.0,
                }
                .into();
            }
        }

        let texture = ui
            .ctx()
            .load_texture("image", img.clone(), TextureOptions::NEAREST);
        let im = PlotImage::new(
            &texture,
            PlotPoint::new((img.width() as f64) / 2.0, (img.height() as f64) / 2.0),
            img.height() as f32 * vec2(texture.aspect_ratio(), 1.0),
        );

        let mut val_y = 0.0;
        let plot_response = plot.show(ui, |plot_ui| {
            plot_ui.image(im);
            match plot_ui.pointer_coordinate() {
                None => {}
                Some(v) => {
                    if 0.0 < v.x
                        && v.x < img.width() as f64
                        && 0.0 < v.y
                        && v.y < img.height() as f64
                    {
                        val_y = img.height() as f32 - v.y as f32;
                    }
                }
            }
        });

        if plot_response.response.double_clicked() {
            *midpoint_position = 50.0;
        } else if plot_response.response.clicked() {
            *midpoint_position = val_y;
        }

        // Get colorbar rectangle bounds
        let colorbar_rect = ui.min_rect();

        // Get the X position for the triangle
        let colorbar_x = colorbar_rect.right() + 5.0;
        let colorbar_y_start = colorbar_rect.top() + 0.03 * *height as f32;
        let colorbar_y_end = colorbar_rect.bottom();

        // Map the midpoint_position (0.0 to 1.0) to the colorbar's vertical bounds
        let triangle_y =
            colorbar_y_start + (colorbar_y_end - colorbar_y_start) * (*midpoint_position / 100.0);

        // Draw the draggable triangle slider
        let triangle_shape = vec![
            pos2(colorbar_x - triangle_radius, triangle_y), // Tip of the triangle
            pos2(
                colorbar_x + triangle_radius / 2.0,
                triangle_y + triangle_radius / 2.0,
            ), // Bottom right
            pos2(
                colorbar_x + triangle_radius / 2.0,
                triangle_y - triangle_radius / 2.0,
            ), // Bottom left
        ];

        let visuals = ui.visuals().clone();
        ui.painter().add(Shape::convex_polygon(
            triangle_shape,
            Color32::WHITE,
            Stroke::new(visuals.window_stroke.width, Color32::DARK_GRAY),
        ));

        // Handle the dragging logic for the triangle
        let response = ui.interact(
            egui::Rect::from_center_size(pos2(colorbar_x, triangle_y), vec2(30.0, 30.0)),
            egui::Id::new("midpoint_slider"),
            egui::Sense::drag(),
        );

        // If dragged, adjust the midpoint position accordingly
        if response.dragged() {
            let delta_y = response.drag_delta().y;
            let new_midpoint_position = (*midpoint_position
                + delta_y / (colorbar_y_end - colorbar_y_start) * 100.0)
                .clamp(0.0, 100.0);
            *midpoint_position = new_midpoint_position;
        }
    });
    // Create a vertical container for the labels next to the colorbar
    let colorbar_height = *height as f32; // Total height of the colorbar
    let label_width = 40.0; // Width of the label area
    let label_x_offset = ui.min_rect().right() + 5.0; // Constant X position (adjust the offset if needed)

    // Dynamically adjust step size based on the height of the colorbar
    let step_size = if colorbar_height < 150.0 {
        20 // Small colorbar, use 20% steps
    } else if colorbar_height > 250.0 {
        5 // Large colorbar, use 5% steps
    } else {
        10 // Medium colorbar, use 10% steps
    };

    // Calculate the number of labels based on the step size
    let num_labels = (100 / step_size) + 1; // Number of labels, including 0 and 100%

    // Draw the labels, removing decimal places from the percentages
    ui.vertical(|ui| {
        for i in 0..num_labels {
            let percentage = i as f32 * step_size as f32;

            // Calculate label position. We want the first label at the top (0%) and last at the bottom (100%).
            let label_position = (percentage / 100.0) * (colorbar_height - 20.0); // Subtract 20.0 to align the 100% label with the bottom edge
            let label_text = format!("{:.0}%", percentage); // Removed decimals

            // Create a fixed-size rectangle to hold the label at the correct height, using a constant x position
            ui.allocate_new_ui(
                UiBuilder::new().max_rect(egui::Rect::from_min_size(
                    egui::pos2(
                        label_x_offset + triangle_radius,
                        ui.min_rect().top() + label_position,
                    ),
                    egui::vec2(label_width, 20.0), // Fixed size for label
                )),
                |ui| {
                    ui.label(RichText::new(label_text).font(FontId::proportional(1.35 * 10.0)));
                },
            );
        }
    });
}

pub fn plot_matrix(
    ui: &mut egui::Ui,
    data: &Array2<f32>,
    plot_width: &f64,
    plot_height: &f64,
    cut_off: &mut f64,
    val: &mut PlotPoint,
    pixel_selected: &mut SelectedPixel,
    scaling: u8,
    midpoint_position: &mut f32,
    bw: &mut bool,
) -> bool {
    let mut pixel_clicked = false;

    let max = data.iter().fold(NEG_INFINITY, |ai, &bi| ai.max(bi as f64));

    ui.horizontal(|ui| {
        ui.label("Noise gate:");
        ui.add(egui::Slider::new(cut_off, 0.0..=100.0));
    });

    let width = data.len_of(Axis(0));
    let height = data.len_of(Axis(1));
    let size = [plot_width / width as f64, plot_height / height as f64]
        .iter()
        .fold(INFINITY, |ai, &bi| ai.min(bi));

    let mut img = ColorImage::new([width, height], Color32::TRANSPARENT);
    let mut intensity_matrix = vec![vec![0.0; height]; width];
    let mut id_matrix = vec![vec!["".to_string(); height]; width];

    for y in 0..height {
        for x in 0..width {
            match data.get((x, y)) {
                Some(i) => {
                    img[(x, y)] = color_from_intensity(i, &max, cut_off, midpoint_position, bw);
                    intensity_matrix[x][height - 1 - y] = *i as f64 / max * 100.0;
                    id_matrix[x][height - 1 - y] = format!("{:05}-{:05}", x, y);
                }
                None => {}
            }
        }
    }

    let texture = ui
        .ctx()
        .load_texture("image", img.clone(), TextureOptions::NEAREST);
    let im = PlotImage::new(
        &texture,
        PlotPoint::new((img.width() as f64) / 2.0, (img.height() as f64) / 2.0),
        img.height() as f32 * vec2(texture.aspect_ratio(), 1.0),
    );

    ui.horizontal(|ui| {
        let plot = Plot::new("image")
            .height(0.75 * height as f32 * size as f32)
            .width(0.75 * width as f32 * size as f32)
            .show_axes([false, false])
            .show_x(false)
            .show_y(false)
            .set_margin_fraction(Vec2 { x: 0.0, y: 0.0 })
            .allow_drag(false);

        let plot_response = plot.show(ui, |plot_ui| {
            plot_ui.image(im);
            if pixel_selected.selected {
                plot_ui.line(
                    Line::new(PlotPoints::from(pixel_selected.rect.clone()))
                        .highlight(true)
                        .color(Color32::GRAY),
                );
            }
            match plot_ui.pointer_coordinate() {
                None => {}
                Some(v) => {
                    if 0.0 < v.x
                        && v.x < img.width() as f64
                        && 0.0 < v.y
                        && v.y < img.height() as f64
                    {
                        *val = v;
                    }
                }
            }
        });

        if plot_response.response.clicked() {
            pixel_clicked = true;
            if pixel_selected.x == val.x.floor() * scaling as f64
                && pixel_selected.y == height as f64 - 1.0 - val.y.floor() * scaling as f64
                && pixel_selected.selected
            {
                println!("pixel unselected");
                pixel_selected.selected = false;
            } else {
                pixel_selected.selected = true;
                pixel_selected.rect = vec![
                    [
                        val.x.floor() * scaling as f64,
                        val.y.floor() * scaling as f64,
                    ],
                    [
                        val.x.floor() * scaling as f64 + 1.0,
                        val.y.floor() * scaling as f64,
                    ],
                    [val.x.floor() + 1.0, val.y.floor() * scaling as f64 + 1.0],
                    [
                        val.x.floor() * scaling as f64,
                        val.y.floor() * scaling as f64 + 1.0,
                    ],
                    [
                        val.x.floor() * scaling as f64,
                        val.y.floor() * scaling as f64,
                    ],
                ];
                pixel_selected.x = val.x.floor() * scaling as f64;
                pixel_selected.y = height as f64 - 1.0 - val.y.floor() * scaling as f64;
                pixel_selected.id = id_matrix[pixel_selected.x as usize / scaling as usize]
                    [pixel_selected.y as usize / scaling as usize]
                    .clone();
                println!("pixel selected");
            }
        }

        ui.add_space(0.01 * &(width as f32 * size as f32));
        colorbar_with_midpoint_slider(
            ui,
            &(0.1 * width as f64 * size),
            &(0.75 * height as f64 * size),
            midpoint_position,
            bw,
        );
    });

    pixel_clicked
}
