use eframe::egui;
use eframe::egui::{
    pos2, vec2, Color32, ColorImage, DragValue, FontId, RichText, Shape, Stroke, UiBuilder, Vec2,
};
use egui::TextureOptions;
use egui_double_slider::DoubleSlider;
use egui_plot::{Line, LineStyle, Plot, PlotImage, PlotPoint, PlotPoints, Polygon};
use ndarray::{Array2, Axis};

#[derive(Debug, Clone)]
pub struct ROI {
    pub polygon: Vec<[f64; 2]>,
    pub closed: bool,
    pub name: String,
}

impl Default for ROI {
    fn default() -> Self {
        Self {
            polygon: vec![],
            closed: false,
            name: "ROI 0".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SelectedPixel {
    pub selected: bool,
    pub rect: Vec<[f64; 2]>,
    pub x: usize,
    pub y: usize,
    pub id: String,
    pub rois: Vec<ROI>, // Store multiple ROIs
}

impl Default for SelectedPixel {
    fn default() -> Self {
        SelectedPixel {
            selected: false,
            rect: vec![[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
            x: 0,
            y: 0,
            id: "0000-0000".to_string(),
            rois: vec![ROI::default()],
        }
    }
}

pub fn make_dummy() -> Array2<f32> {
    let width = 20;
    let height = 20;
    Array2::from_shape_fn((width, height), |(i, _)| i as f32)
}

pub fn color_from_intensity(
    i: &f32,
    max_intensity: &f64,
    cut_off: &[f32; 2],
    midpoint_position: &f32,
    bw: &bool,
) -> Color32 {
    let normalized_y = (*i / *max_intensity as f32).clamp(0.0, 1.0);
    let hue = if normalized_y <= (*midpoint_position / 100.0) {
        (normalized_y / (*midpoint_position / 100.0)) * 0.667 // Blue to midpoint
    } else {
        0.667
            - ((normalized_y - (*midpoint_position / 100.0)) / (1.0 - (*midpoint_position / 100.0)))
                * 0.667 // Midpoint to red
    };

    let clamped_hue = if normalized_y * 100.0 <= cut_off[0] {
        0.667 // Force blue at min cutoff
    } else if normalized_y * 100.0 >= cut_off[1] {
        0.0 // Force red at max cutoff
    } else {
        hue // Interpolated hue
    };

    if *bw {
        egui::ecolor::Hsva {
            h: 0.0,
            s: 0.0,
            v: normalized_y,
            a: 1.0,
        }
        .into()
    } else {
        egui::ecolor::Hsva {
            h: clamped_hue,
            s: 1.0,
            v: 1.0,
            a: 1.0,
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

#[allow(clippy::too_many_arguments)]
pub fn plot_matrix(
    ui: &mut egui::Ui,
    data: &Array2<f32>,
    plot_width: &f64,
    plot_height: &f64,
    cut_off: &mut [f32; 2],
    val: &mut PlotPoint,
    pixel_selected: &mut SelectedPixel,
    midpoint_position: &mut f32,
    bw: &mut bool,
) -> bool {
    let mut pixel_clicked = false;
    let max = data
        .iter()
        .fold(f64::NEG_INFINITY, |ai, &bi| ai.max(bi as f64));

    ui.label("Clipping:");
    let mut cut_off_low = cut_off[0];
    let mut cut_off_high = cut_off[1];
    ui.horizontal(|ui| {
        if ui
            .add(
                DoubleSlider::new(&mut cut_off_low, &mut cut_off_high, 0.0..=100.0)
                    .separation_distance(5.0)
                    .width((*plot_width as f32) * 0.95),
            )
            .on_hover_text(egui::RichText::new(format!(
                "{} Adjust the clipping of the image. Double-click to reset.",
                egui_phosphor::regular::INFO
            )))
            .double_clicked()
        {
            cut_off_low = 0.0;
            cut_off_high = 100.0;
        };
    });
    ui.horizontal(|ui| {
        ui.add(DragValue::new(&mut cut_off_low));

        ui.add_space((0.65 * *plot_width) as f32);

        ui.add(DragValue::new(&mut cut_off_high));
    });
    *cut_off = [cut_off_low, cut_off_high];

    let width = data.len_of(Axis(0));
    let height = data.len_of(Axis(1));
    let size = [plot_width / width as f64, plot_height / height as f64]
        .iter()
        .fold(f64::INFINITY, |ai, &bi| ai.min(bi));

    let mut img = ColorImage::new([width, height], Color32::TRANSPARENT);
    let mut intensity_matrix = vec![vec![0.0; height]; width];
    let mut id_matrix = vec![vec!["".to_string(); height]; width];

    for y in 0..height {
        for x in 0..width {
            if let Some(i) = data.get((x, y)) {
                img[(x, y)] = color_from_intensity(i, &max, cut_off, midpoint_position, bw);
                intensity_matrix[x][height - 1 - y] = *i as f64 / max * 100.0;
                id_matrix[x][height - 1 - y] = format!("{:05}-{:05}", x, y);
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

            // Draw selected single pixel
            if pixel_selected.selected {
                plot_ui.line(
                    Line::new(PlotPoints::from(pixel_selected.rect.clone()))
                        .highlight(true)
                        .color(Color32::GRAY),
                );
            }

            // Draw all ROIs
            for (i, roi) in pixel_selected.rois.iter().enumerate() {
                let line = Line::new(PlotPoints::from(roi.polygon.clone()))
                    .color(Color32::WHITE)
                    .width(2.0);
                plot_ui.line(line);

                if roi.closed {
                    let screen_points: Vec<[f64; 2]> = roi
                        .polygon
                        .iter()
                        .map(|p| {
                            let _point = plot_ui
                                .transform()
                                .position_from_point(&PlotPoint::new(p[0], p[1]));
                            [p[0], p[1]]
                        })
                        .collect();
                    plot_ui.polygon(
                        Polygon::new(PlotPoints::from(screen_points))
                            .fill_color(Color32::WHITE.gamma_multiply(0.5))
                            .highlight(false)
                            .style(LineStyle::Solid)
                            .width(2.0)
                            .name("roi"),
                    );
                }
            }

            // Track pointer position
            if let Some(v) = plot_ui.pointer_coordinate() {
                if (0.0..img.width() as f64).contains(&v.x)
                    && (0.0..img.height() as f64).contains(&v.y)
                {
                    *val = v;
                }
            }
        });

        if plot_response.response.clicked() {
            let modifiers = ui.input(|i| i.modifiers);
            if modifiers.shift {
                dbg!(&pixel_selected.rois.len());
                // Handle multiple polygon ROIs
                let plot_x = val.x;
                let plot_y = val.y;
                let _pixel_x = plot_x.floor() as usize;
                let _pixel_y = height - 1 - plot_y.floor() as usize;

                if (!pixel_selected.rois.is_empty() && pixel_selected.rois.last().unwrap().closed)
                    || pixel_selected.rois.is_empty()
                {
                    // If last ROI is closed, start a new one
                    let mut roi = ROI::default();
                    roi.name = format!("ROI {}", pixel_selected.rois.len() + 1);
                    pixel_selected.rois.push(roi);
                }
                dbg!(&pixel_selected.rois.len());

                if let Some(current_roi) = pixel_selected.rois.last_mut() {
                    if current_roi.polygon.is_empty() {
                        current_roi.polygon.push([plot_x, plot_y]);
                    } else {
                        // Check distance to first point
                        let first = current_roi.polygon.first().unwrap();
                        let dx = plot_x - first[0];
                        let dy = plot_y - first[1];
                        let dist = (dx * dx + dy * dy).sqrt();

                        if dist < width.min(height) as f64 * 0.05 && current_roi.polygon.len() > 1 {
                            // Close polygon
                            current_roi.closed = true;
                        } else {
                            // Add new point
                            current_roi.polygon.push([plot_x, plot_y]);
                        }
                    }
                }
                pixel_clicked = true;
            } else {
                // Handle single pixel selection

                if pixel_selected.x == val.x.floor() as usize
                    && pixel_selected.y == height - 1 - val.y.floor() as usize
                    && pixel_selected.selected
                {
                    pixel_selected.selected = false;
                } else {
                    pixel_selected.selected = true;
                    pixel_selected.rect = vec![
                        [val.x.floor(), val.y.floor()],
                        [val.x.floor() + 1.0, val.y.floor()],
                        [val.x.floor() + 1.0, val.y.floor() + 1.0],
                        [val.x.floor(), val.y.floor() + 1.0],
                        [val.x.floor(), val.y.floor()],
                    ];
                    pixel_selected.x = val.x.floor() as usize;
                    pixel_selected.y = height - 1 - val.y.floor() as usize;
                    pixel_selected.id = id_matrix[pixel_selected.x][pixel_selected.y].clone();
                }
                pixel_clicked = true;
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
