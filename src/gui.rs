use core::f64;
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use eframe::{egui, Storage};
use egui_plot::PlotPoint;
use ndarray::Array2;
use preferences::Preferences;
use serde::{Deserialize, Serialize};

use crate::center_panel::center_panel;
use crate::config::Config;
use crate::data::DataPoint;
use crate::left_panel::left_panel;
use crate::matrix_plot::SelectedPixel;
use crate::right_panel::right_panel;
use crate::APP_INFO;

const MAX_FPS: f64 = 24.0;

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct GuiSettingsContainer {
    pub log_plot: bool,
    pub down_scaling: usize,
    pub normalize_fft: bool,
    pub signal_1_visible: bool,
    pub ref_1_visible: bool,
    pub filtered_signal_1_visible: bool,
    pub water_lines_visible: bool,
    pub phases_visible: bool,
    pub frequency_resolution_temp: f32,
    pub frequency_resolution: f32,
    pub advanced_settings_window: bool,
    pub debug: bool,
    pub dark_mode: bool,
    pub x: f32,
    pub y: f32,
}

impl GuiSettingsContainer {
    pub fn new() -> GuiSettingsContainer {
        return GuiSettingsContainer {
            log_plot: true,
            down_scaling: 1,
            normalize_fft: false,
            signal_1_visible: true,
            ref_1_visible: false,
            filtered_signal_1_visible: false,
            water_lines_visible: false,
            phases_visible: false,
            frequency_resolution_temp: 0.001,
            frequency_resolution: 0.001,
            advanced_settings_window: false,
            debug: true,
            dark_mode: true,
            x: 1600.0,
            y: 900.0,
        };
    }
}

pub struct MyApp<'a> {
    dark_mode: bool,
    fft_bounds: [f32; 2],
    filter_bounds: [f32; 2],
    time_window: [f32; 2],
    pixel_selected: SelectedPixel,
    val: PlotPoint,
    mid_point: f32,
    bw: bool,
    hacktica_light: egui::Image<'a>,
    hacktica_dark: egui::Image<'a>,
    connection_error_image_dark: egui::Image<'a>,
    connection_error_image_light: egui::Image<'a>,
    coconut_logo_dark: egui::Image<'a>,
    coconut_logo_light: egui::Image<'a>,
    water_vapour_lines: Vec<f64>,
    wp: egui::Image<'a>,
    dropped_files: Vec<egui::DroppedFile>,
    picked_path: String,
    data: DataPoint,
    gui_conf: GuiSettingsContainer,
    img_lock: Arc<RwLock<Array2<f32>>>,
    data_lock: Arc<RwLock<DataPoint>>,
    pixel_lock: Arc<RwLock<SelectedPixel>>,
    scaling_lock: Arc<RwLock<u8>>,
    config_tx: Sender<Config>,
    load_tx: Sender<PathBuf>,
}

impl<'a> MyApp<'a> {
    pub fn new(
        data_lock: Arc<RwLock<DataPoint>>,
        pixel_lock: Arc<RwLock<SelectedPixel>>,
        scaling_lock: Arc<RwLock<u8>>,
        img_lock: Arc<RwLock<Array2<f32>>>,
        gui_conf: GuiSettingsContainer,
        config_tx: Sender<Config>,
        load_tx: Sender<PathBuf>,
    ) -> Self {
        let mut water_vapour_lines: Vec<f64> = Vec::new();
        let buffered = include_str!("../resources/water_lines.csv");
        for line in buffered.lines() {
            water_vapour_lines.push(line.trim().parse().unwrap());
        }

        Self {
            dark_mode: true,
            hacktica_dark: egui::Image::from_bytes(
                "Hacktica Dark",
                include_bytes!("../images/hacktica_inv.png"),
            ),
            hacktica_light: egui::Image::from_bytes(
                "Hacktica",
                include_bytes!("../images/hacktica.png"),
            ),
            connection_error_image_dark: egui::Image::from_bytes(
                "Connection Error Dark",
                include_bytes!("../images/connection_error_inv.png"),
            ),
            connection_error_image_light: egui::Image::from_bytes(
                "Connection Error",
                include_bytes!("../images/connection_error.png"),
            ),
            coconut_logo_dark: egui::Image::from_bytes(
                "COCoNuT Dark",
                include_bytes!("../images/coconut_inv.png"),
            ),
            coconut_logo_light: egui::Image::from_bytes(
                "COCoNuT",
                include_bytes!("../images/coconut.png"),
            ),
            water_vapour_lines,
            wp: egui::Image::from_bytes("WP", include_bytes!("../images/WP-Logo.png")),

            dropped_files: vec![],
            picked_path: "".to_string(),
            data: DataPoint::default(),
            gui_conf,
            img_lock,
            data_lock,
            pixel_lock,
            scaling_lock,
            config_tx,
            load_tx,
            fft_bounds: [1.0, 7.0],
            filter_bounds: [0.0, 10.0],
            time_window: [1000.0, 1050.0],
            pixel_selected: SelectedPixel::default(),
            val: PlotPoint { x: 0.0, y: 0.0 },
            mid_point: 50.0,
            bw: false,
        }
    }
}

impl<'a> eframe::App for MyApp<'a> {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let left_panel_width = 300.0;
        let right_panel_width = 500.0;

        center_panel(
            &ctx,
            &right_panel_width,
            &left_panel_width,
            &mut self.gui_conf,
            &mut self.data,
            &self.data_lock,
            &self.config_tx,
            &self.water_vapour_lines,
        );

        left_panel(
            &ctx,
            &left_panel_width,
            &mut self.picked_path,
            &mut self.gui_conf,
            self.coconut_logo_light.clone(),
            self.coconut_logo_dark.clone(),
            &mut self.pixel_selected,
            &mut self.val,
            &mut self.mid_point,
            &mut self.bw,
            &self.img_lock,
            &self.data_lock,
            &self.pixel_lock,
            &self.scaling_lock,
            &self.config_tx,
            &self.load_tx,
        );

        right_panel(
            &ctx,
            &right_panel_width,
            &mut self.gui_conf,
            &mut self.filter_bounds,
            &mut self.fft_bounds,
            &mut self.time_window,
            &self.config_tx,
            &self.data_lock,
            &self.scaling_lock,
            self.hacktica_dark.clone(),
            self.hacktica_light.clone(),
            self.wp.clone(),
        );

        self.gui_conf.x = ctx.used_size().x;
        self.gui_conf.y = ctx.used_size().y;

        std::thread::sleep(Duration::from_millis((1000.0 / MAX_FPS) as u64));
    }

    fn save(&mut self, _storage: &mut dyn Storage) {
        let prefs_key = "config/gui";
        match self.gui_conf.save(&APP_INFO, prefs_key) {
            Ok(_) => {}
            Err(err) => {
                println!("error saving gui_conf: {err:?}");
            }
        }
    }
}
