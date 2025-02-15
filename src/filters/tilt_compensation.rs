use crate::config::GuiThreadCommunication;
use crate::data_container::ScannedImage;
use crate::filters::filter::{Filter, FilterConfig, FilterDomain};
use crate::gui::application::GuiSettingsContainer;
use eframe::egui::{self, Ui};
use filter_macros::register_filter;
use ndarray::s;
use std::f32::consts::PI;

#[derive(Debug)]
#[register_filter]
pub struct TiltCompensation {
    pub tilt_x: f64,
    pub tilt_y: f64,
}

impl Filter for TiltCompensation {
    fn new() -> Self
    where
        Self: Sized,
    {
        TiltCompensation {
            tilt_x: 0.0,
            tilt_y: 0.0,
        }
    }

    fn config(&self) -> FilterConfig {
        FilterConfig {
            name: "Tilt Compensation".to_string(),
            domain: FilterDomain::Frequency,
        }
    }

    fn filter(&self, scan: &mut ScannedImage, gui_settings: &mut GuiSettingsContainer) {
        // only rotation around the center are implemented, offset rotations are still to be done.
        let time_shift_x = self.tilt_x as f32 / 180.0 * PI;
        let time_shift_y = self.tilt_y as f32 / 180.0 * PI;

        let (width, height, time_samples) = scan.raw_data.dim();
        let center_x = (width as f32 - 1.0) / 2.0;
        let center_y = (height as f32 - 1.0) / 2.0;
        let dt = 0.25;
        let c = 0.299792458_f64; // mm/ps

        dbg!(&time_shift_x);

        for i in 0..width {
            for j in 0..height {
                let x_offset = i as f32 - center_x;
                let y_offset = j as f32 - center_y;
                let delta = ((x_offset as f64 * scan.dx.unwrap() as f64 * time_shift_x as f64
                    + y_offset as f64 * scan.dy.unwrap() as f64 * time_shift_y as f64)
                    / c) as f32;

                let raw_trace = scan.raw_data.slice(s![i, j, ..]);
                let mut filtered_trace = ndarray::Array1::zeros(time_samples);

                for t in 0..time_samples {
                    let pos = t as f32 - delta / dt;
                    let t0 = pos.floor() as i32;
                    let t1 = t0 + 1;
                    let frac = pos - t0 as f32;

                    let value = if t0 >= 0 && t1 < time_samples as i32 {
                        let a = raw_trace[t0 as usize] as f32;
                        let b = raw_trace[t1 as usize] as f32;
                        a * (1.0 - frac) + b * frac
                    } else if t0 < 0 {
                        raw_trace[0] as f32
                    } else if t1 >= time_samples as i32 {
                        raw_trace[time_samples - 1] as f32
                    } else {
                        0.0
                    };

                    filtered_trace[t] = value;
                }

                scan.filtered_data
                    .slice_mut(s![i, j, ..])
                    .assign(&filtered_trace);
            }
        }
    }

    fn ui(&mut self, ui: &mut Ui, _thread_communication: &mut GuiThreadCommunication) {
        ui.horizontal(|ui| {
            ui.label("Tilt X: ");
            ui.add(egui::Slider::new(&mut self.tilt_x, -15.0..=15.0).suffix(" deg"));
        });
        ui.horizontal(|ui| {
            ui.label("Tilt Y: ");
            ui.add(egui::Slider::new(&mut self.tilt_y, -15.0..=15.0).suffix(" deg"));
        });
    }
}
