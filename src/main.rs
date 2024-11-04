#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
// hide console window on Windows in release

extern crate core;
extern crate csv;
extern crate preferences;
extern crate serde;

use std::sync::mpsc::{Receiver, Sender};
use std::sync::{mpsc, Arc, RwLock};
use std::thread;

use crate::config::Config;
use eframe::egui::{vec2, ViewportBuilder, Visuals};
use eframe::{egui, icon_data};
use ndarray::Array2;
use preferences::{AppInfo, Preferences};

use crate::data::DataPoint;
use crate::data_thread::main_thread;
use crate::gui::{GuiSettingsContainer, MyApp};
use crate::matrix_plot::SelectedPixel;

mod center_panel;
mod config;
mod data;
mod data_thread;
mod filters;
mod gauge;
mod gui;
mod io;
mod left_panel;
mod math_tools;
mod matrix_plot;
mod right_panel;
mod toggle;

const APP_INFO: AppInfo = AppInfo {
    name: "COExplore",
    author: "Linus Leo Stöckli",
};

fn main() {
    egui_logger::builder().init().unwrap();

    let mut gui_settings = GuiSettingsContainer::new();
    let prefs_key = "config/gui";
    let load_result = GuiSettingsContainer::load(&APP_INFO, prefs_key);
    if load_result.is_ok() {
        gui_settings = load_result.unwrap();
    } else {
        // save default settings
        match gui_settings.save(&APP_INFO, prefs_key) {
            Ok(_) => {}
            Err(err) => {
                println!("error in saving gui_settings send: {err:?}");
            }
        }
    }

    let data_lock = Arc::new(RwLock::new(DataPoint::default()));
    let img_lock = Arc::new(RwLock::new(Array2::from_shape_fn((1, 1), |(_, _)| 0.0)));
    let pixel_lock = Arc::new(RwLock::new(SelectedPixel::default()));
    let scaling_lock = Arc::new(RwLock::new(1));

    let (config_tx, config_rx): (Sender<Config>, Receiver<Config>) = mpsc::channel();

    let main_data_lock = data_lock.clone();
    let main_img_lock = img_lock.clone();
    let main_scaling_lock = scaling_lock.clone();
    let main_pixel_lock = pixel_lock.clone();

    println!("starting main server..");
    let _main_thread_handler = thread::spawn(|| {
        main_thread(
            main_data_lock,
            main_img_lock,
            config_rx,
            main_scaling_lock,
            main_pixel_lock,
        );
    });

    let options = eframe::NativeOptions {
        viewport: ViewportBuilder::default()
            .with_drag_and_drop(true)
            .with_inner_size(vec2(gui_settings.x, gui_settings.y))
            .with_icon(
                icon_data::from_png_bytes(&include_bytes!("../icons/icon.png")[..]).unwrap(),
            ),
        ..Default::default()
    };

    let gui_data_lock = data_lock.clone();
    let gui_img_lock = img_lock.clone();
    let gui_pixel_lock = pixel_lock.clone();
    let gui_scaling_lock = scaling_lock.clone();

    eframe::run_native(
        "COCoNuT Explore",
        options,
        Box::new(|_cc| {
            let mut fonts = egui::FontDefinitions::default();
            egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
            _cc.egui_ctx.set_fonts(fonts);
            egui_extras::install_image_loaders(&_cc.egui_ctx);
            _cc.egui_ctx.set_visuals(Visuals::dark());
            Ok(Box::new(MyApp::new(
                gui_data_lock,
                gui_pixel_lock,
                gui_scaling_lock,
                gui_img_lock,
                gui_settings,
                config_tx,
            )))
        }),
    )
    .expect("Failed to launch GUI");
}
