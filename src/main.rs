#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
// hide console window on Windows in release
extern crate serde;
extern crate preferences;
extern crate core;
extern crate csv;

mod gui;
mod center_panel;
#[path = "teraflash-ctrl/src/toggle.rs"]
mod toggle;
mod io;
mod math_tools;
#[path = "teraflash-ctrl/src/errors.rs"]
mod errors;
#[path = "teraflash-ctrl/src/plot_slider.rs"]
mod plot_slider;
mod data_thread;
mod gauge;
mod left_panel;
mod matrix_plot;
mod data;

use std::thread;
use eframe::egui::{vec2, Visuals};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, mpsc, RwLock};
use eframe::HardwareAcceleration;
use itertools_num::linspace;
use preferences::{AppInfo, Preferences};
use crate::data::DataContainer;
use crate::data_thread::{main_thread, ScannedImage};

use crate::gui::{GuiSettingsContainer, GuiState, MyApp, Print, print_to_console, SelectedPixel, update_in_console};
use crate::io::save_to_csv;
use crate::math_tools::{make_fft, MovingAverage};

const APP_INFO: AppInfo = AppInfo { name: "COExplore", author: "Linus Leo Stöckli" };


fn main() {

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

    let pixel_lock = Arc::new(RwLock::new(SelectedPixel::new()));
    let data_lock = Arc::new(RwLock::new(DataContainer::default()));
    let img_lock = Arc::new(RwLock::new(ScannedImage::default()));
    let df_lock = Arc::new(RwLock::new(gui_settings.frequency_resolution));
    let log_mode_lock = Arc::new(RwLock::new(gui_settings.log_plot));
    let normalize_fft_lock = Arc::new(RwLock::new(gui_settings.normalize_fft));
    let fft_bounds_lock = Arc::new(RwLock::new([1.0, 7.0]));
    let status_lock = Arc::new(RwLock::new("".to_string()));
    let connected_lock = Arc::new(RwLock::new(0));
    let pixel_lock = Arc::new(RwLock::new(SelectedPixel::new()));
    let print_lock = Arc::new(RwLock::new(vec![Print::EMPTY]));

    let (save_tx, save_rx): (Sender<String>, Receiver<String>) = mpsc::channel();

    let main_data_lock = data_lock.clone();
    let main_print_lock = print_lock.clone();
    let main_log_mode_lock = log_mode_lock.clone();
    let main_df_lock = df_lock.clone();
    let main_normalize_fft_lock = normalize_fft_lock.clone();
    let main_fft_bounds_lock = fft_bounds_lock.clone();
    let main_pixel_lock = pixel_lock.clone();

    println!("starting main server..");
    let main_thread_handler = thread::spawn(|| {
        main_thread(main_data_lock,
                    main_df_lock,
                    main_log_mode_lock,
                    main_normalize_fft_lock,
                    main_fft_bounds_lock,
                    main_print_lock,
                    save_rx);
    });


    let options = eframe::NativeOptions {
        drag_and_drop_support: true,
        initial_window_size: Option::from(vec2(gui_settings.x, gui_settings.y)),
        // hardware_acceleration : HardwareAcceleration::Off,
        ..Default::default()
    };

    let gui_data_lock = data_lock.clone();
    let gui_df_lock = df_lock.clone();
    let gui_log_mode_lock = log_mode_lock.clone();
    let gui_normalize_fft_lock = normalize_fft_lock.clone();
    let gui_fft_bounds_lock = fft_bounds_lock.clone();
    let gui_status_lock = status_lock.clone();
    let gui_connected_lock = connected_lock.clone();
    let gui_print_lock = print_lock.clone();
    let gui_img_lock = img_lock.clone();
    let gui_pixel_lock = pixel_lock.clone();

    eframe::run_native(
        "COCoNuT Explore",
        options,
        Box::new(|_cc| {
            _cc.egui_ctx.set_visuals(Visuals::dark());
            Box::new(MyApp::new(
                gui_print_lock,
                gui_data_lock,
                gui_df_lock,
                gui_pixel_lock,
                gui_log_mode_lock,
                gui_img_lock,
                gui_normalize_fft_lock,
                gui_fft_bounds_lock,
                gui_settings,
                save_tx
            ))
        }),
    );

    main_thread_handler.join().unwrap();
}
