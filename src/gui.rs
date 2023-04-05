use core::f64;
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use eframe::egui::plot::PlotPoint;
use eframe::{egui, Storage};
use egui_extras::RetainedImage;
use ndarray::Array2;
use preferences::Preferences;
use serde::{Deserialize, Serialize};

use crate::center_panel::center_panel;
use crate::config::Config;
use crate::data::DataPoint;
use crate::left_panel::left_panel;
use crate::right_panel::right_panel;
use crate::APP_INFO;

const MAX_FPS: f64 = 24.0;

#[derive(Clone)]
#[allow(unused)]
pub enum Print {
    EMPTY,
    MESSAGE(String),
    ERROR(String),
    DEBUG(String),
    TASK(String),
    OK(String),
}

pub fn print_to_console(print_lock: &Arc<RwLock<Vec<Print>>>, message: Print) -> usize {
    let mut index: usize = 0;
    if let Ok(mut write_guard) = print_lock.write() {
        write_guard.push(message);
        index = write_guard.len() - 1;
    }
    index
}

pub fn update_in_console(print_lock: &Arc<RwLock<Vec<Print>>>, message: Print, index: usize) {
    if let Ok(mut write_guard) = print_lock.write() {
        write_guard[index] = message;
    }
}

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

pub struct MyApp {
    dark_mode: bool,
    console: Vec<Print>,
    fft_bounds: [f32; 2],
    filter_bounds: [f32; 2],
    pixel_selected: SelectedPixel,
    val: PlotPoint,
    hacktica_light: RetainedImage,
    hacktica_dark: RetainedImage,
    connection_error_image_dark: RetainedImage,
    connection_error_image_light: RetainedImage,
    coconut_logo_dark: RetainedImage,
    coconut_logo_light: RetainedImage,
    water_vapour_lines: Vec<f64>,
    wp: RetainedImage,
    dropped_files: Vec<egui::DroppedFile>,
    picked_path: String,
    data: DataPoint,
    print_lock: Arc<RwLock<Vec<Print>>>,
    gui_conf: GuiSettingsContainer,
    img_lock: Arc<RwLock<Array2<f32>>>,
    waterfall_lock: Arc<RwLock<Array2<f32>>>,
    data_lock: Arc<RwLock<DataPoint>>,
    df_lock: Arc<RwLock<f32>>,
    log_mode_lock: Arc<RwLock<bool>>,
    normalize_fft_lock: Arc<RwLock<bool>>,
    fft_bounds_lock: Arc<RwLock<[f32; 2]>>,
    fft_filter_bounds_lock: Arc<RwLock<[f32; 2]>>,
    pixel_lock: Arc<RwLock<SelectedPixel>>,
    config_tx: Sender<Config>,
    load_tx: Sender<PathBuf>,
}

impl MyApp {
    pub fn new(
        print_lock: Arc<RwLock<Vec<Print>>>,
        data_lock: Arc<RwLock<DataPoint>>,
        df_lock: Arc<RwLock<f32>>,
        pixel_lock: Arc<RwLock<SelectedPixel>>,
        log_mode_lock: Arc<RwLock<bool>>,
        img_lock: Arc<RwLock<Array2<f32>>>,
        waterfall_lock: Arc<RwLock<Array2<f32>>>,
        normalize_fft_lock: Arc<RwLock<bool>>,
        fft_bounds_lock: Arc<RwLock<[f32; 2]>>,
        fft_filter_bounds_lock: Arc<RwLock<[f32; 2]>>,
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
            hacktica_dark: RetainedImage::from_image_bytes(
                "Hacktica",
                include_bytes!("../images/hacktica_inv.png"),
            )
            .unwrap(),
            hacktica_light: RetainedImage::from_image_bytes(
                "Hacktica",
                include_bytes!("../images/hacktica.png"),
            )
            .unwrap(),
            connection_error_image_dark: RetainedImage::from_image_bytes(
                "Hacktica",
                include_bytes!("../images/connection_error_inv.png"),
            )
            .unwrap(),
            connection_error_image_light: RetainedImage::from_image_bytes(
                "Hacktica",
                include_bytes!("../images/connection_error.png"),
            )
            .unwrap(),
            coconut_logo_dark: RetainedImage::from_image_bytes(
                "Hacktica",
                include_bytes!("../images/coconut_inv.png"),
            )
            .unwrap(),
            coconut_logo_light: RetainedImage::from_image_bytes(
                "Hacktica",
                include_bytes!("../images/coconut.png"),
            )
            .unwrap(),
            water_vapour_lines,
            wp: RetainedImage::from_image_bytes("WP", include_bytes!("../images/WP-Logo.png"))
                .unwrap(),

            dropped_files: vec![],
            picked_path: "".to_string(),
            data: DataPoint::default(),
            console: vec![Print::MESSAGE("".to_string())],
            print_lock,
            gui_conf,
            img_lock,
            waterfall_lock,
            data_lock,
            df_lock,
            log_mode_lock,
            normalize_fft_lock,
            fft_bounds_lock,
            fft_filter_bounds_lock,
            pixel_lock,
            config_tx,
            load_tx,
            fft_bounds: [1.0, 7.0],
            filter_bounds: [0.0, 10.0],
            pixel_selected: SelectedPixel::default(),
            val: PlotPoint { x: 0.0, y: 0.0 },
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let left_panel_width = 300.0;
        let right_panel_width = 500.0;

        center_panel(
            &ctx,
            &right_panel_width,
            &left_panel_width,
            &mut self.gui_conf,
            &mut self.data,
            &self.df_lock,
            &self.data_lock,
            &self.water_vapour_lines,
        );

        left_panel(
            &ctx,
            &left_panel_width,
            &mut self.picked_path,
            &mut self.gui_conf,
            &self.coconut_logo_light,
            &self.coconut_logo_dark,
            &mut self.pixel_selected,
            &mut self.val,
            &self.img_lock,
            &self.waterfall_lock,
            &self.data_lock,
            &self.print_lock,
            &self.pixel_lock,
            &self.config_tx,
            &self.load_tx,
        );

        right_panel(
            &ctx,
            &right_panel_width,
            &mut self.gui_conf,
            &mut self.console,
            &mut self.picked_path,
            &mut self.filter_bounds,
            &mut self.fft_bounds,
            &self.config_tx,
            &self.data_lock,
            &self.print_lock,
            &self.log_mode_lock,
            &self.normalize_fft_lock,
            &self.fft_bounds_lock,
            &self.fft_filter_bounds_lock,
            &self.hacktica_dark,
            &self.hacktica_light,
            &self.wp,
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
