use std::f64::{INFINITY, NEG_INFINITY};
use eframe::egui;
use eframe::egui::{Color32, ColorImage, FontId, RichText, vec2, Vec2};
use eframe::egui::plot::{Line, Plot, PlotImage, PlotPoint, PlotPoints};
use eframe::egui_glow::painter::TextureFilter;
use ndarray::{Array2, Axis};
use crate::gui::SelectedPixel;


pub fn make_dummy() -> Array2<f64> {
    let width = 20;
    let height = 10;
    let data = Array2::from_shape_fn((width, height), |(i, _)| {
        i as f64
    });
    data
}

pub fn color_from_intensity(i: f64, max_intensity: f64, cut_off: f64) -> Color32 {
    let h = i / max_intensity * 0.667; // only go from red to blue
    if h > cut_off / max_intensity * 0.667 {
        egui::color::Hsva { h: h as f32, s: 1.0, v: 1.0, a: 1.0 }.into()
    } else {
        egui::color::Hsva { h: h as f32, s: 1.0, v: 0.0, a: 0.0 }.into()
    }
}

fn colorbar(ui: &mut egui::Ui, width: &f64, height: &f64) {
    let plot = Plot::new("colorbar")
        .height(*height as f32)
        .width(*width as f32 * 0.5)
        .show_axes([false, false])
        .set_margin_fraction(Vec2 { x: 0.0, y: 0.0 })
        .allow_zoom(false)
        .allow_scroll(false)
        .allow_boxed_zoom(false)
        .allow_drag(false)
        .show_x(false)
        .show_y(false);

    let mut img = egui::ColorImage::new([1, 100], Color32::TRANSPARENT);
    for y in 0..100 {
        img[(0, y)] = egui::color::Hsva { h: y as f32 / 100.0 * 0.667, s: 1.0, v: 1.0, a: 1.0 }.into()
    }

    let texture = ui.ctx().load_texture("image", img.clone(), TextureFilter::Nearest);
    let im = PlotImage::new(
        &texture,
        PlotPoint::new((img.width() as f64) / 2.0, (img.height() as f64) / 2.0),
        img.height() as f32 * vec2(texture.aspect_ratio(), 1.0),
    );
    plot.show(ui, |plot_ui| {
        plot_ui.image(im);
    });
}

pub fn plot_matrix(ui: &mut egui::Ui,
                   data: &Array2<f64>,
                   plot_width: &f64,
                   plot_height: &f64,
                   cut_off: &mut f64,
                   val: &mut PlotPoint,
                   pixel_selected: &mut SelectedPixel) -> ColorImage {
    let max = data.iter().fold(NEG_INFINITY, |ai, &bi| ai.max(bi));
    ui.horizontal(|ui| {
        ui.label("Noise gate:");
        ui.add(egui::Slider::new(cut_off, 0.0..=100.0));
    });
    let width = data.len_of(Axis(0));
    let height = data.len_of(Axis(1));
    let size = [plot_width / width as f64, plot_height / height as f64].iter().fold(INFINITY, |ai, &bi| ai.min(bi));
    let mut img = ColorImage::new([width, height], Color32::TRANSPARENT);
    let mut intensity_matrix = vec![vec![0.0; height]; width];
    let mut id_matrix = vec![vec!["".to_string(); height]; width];
    for y in 0..height {
        for x in 0..width {
            match data.get((x, y)) {
                Some(i) => {
                    img[(x, y)] = color_from_intensity(*i / max * 100.0, 100.0, *cut_off);
                    intensity_matrix[x][height - 1 - y] = *i / max * 100.0;
                    id_matrix[x][height - 1 - y] = format!("{:05}-{:05}", x, y);
                }
                None => {}
            }
        }
    }
    let texture = ui.ctx().load_texture("image", img.clone(), TextureFilter::Nearest);
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
            //.set_margin_fraction(Vec2 { x: 0.0, y: 0.0 })
            .allow_drag(false);
        let plot_response = plot.show(ui, |plot_ui| {
            plot_ui.image(im);
            if pixel_selected.selected {
                plot_ui.line(Line::new(PlotPoints::from(pixel_selected.rect.clone()))
                    .highlight(true)
                    .color(Color32::GRAY));
            }
            match plot_ui.pointer_coordinate() {
                None => {}
                Some(v) => {
                    if 0.0 < v.x && v.x < img.width() as f64 && 0.0 < v.y && v.y < img.height() as f64 {
                        *val = v;
                    }
                }
            }
        });
        ui.add_space(0.01 * &(width as f32 * size as f32));
        colorbar(ui, &(0.1 * width as f64 * size as f64), &(0.78 * height as f64 * size as f64));
        ui.vertical(|ui| {
            for i in 0..11 {
                ui.label(RichText::new(format!("{:.1}%",  (i as f64) / (11.0 - 1.0) * (100.0)))
                    .font(FontId::proportional(1.35 * 10.0)));
            }
        });
        if plot_response.response.clicked() {
            // display spectrum!
            if pixel_selected.x == val.x.floor() && pixel_selected.y == val.y.floor() &&
                pixel_selected.selected {
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
                pixel_selected.x = val.x.floor();
                pixel_selected.y = height as f64 - 1.0 - val.y.floor();
                pixel_selected.id = id_matrix[pixel_selected.x as usize][pixel_selected.y as usize].clone();
            }
        }
    });

    let x = val.x.floor() as usize;
    let y = val.y.floor() as usize;
    ui.label(format!("ID = {}: i = {:.2}%", id_matrix[x][y], intensity_matrix[x][y]));
    img
}
