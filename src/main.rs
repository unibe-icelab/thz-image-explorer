#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
// hide console window on Windows in release

extern crate core;
extern crate csv;
extern crate preferences;
extern crate serde;

use crate::config::{ConfigCommand, GuiThreadCommunication, MainThreadCommunication};
use crate::data_container::DataPoint;
use crate::data_thread::main_thread;
use crate::filters::filter::FILTER_REGISTRY;
use crate::gui::application::{GuiSettingsContainer, THzImageExplorer};
use crate::gui::matrix_plot::SelectedPixel;
use dotthz::DotthzMetaData;
use eframe::egui::{vec2, ViewportBuilder, Visuals};
use eframe::{egui, icon_data};
use ndarray::Array2;
use preferences::{AppInfo, Preferences};
use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{mpsc, Arc, RwLock};
use std::sync::atomic::AtomicBool;
use std::thread;

mod config;
mod data_container;
mod data_thread;
mod filters;
mod gui;
mod io;
mod math_tools;
mod update;
mod cancellable_loops;

const APP_INFO: AppInfo = AppInfo {
    name: "THz Image Explorer",
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
                log::error!("error in saving gui_settings send: {err:?}");
            }
        }
    }

    gui_settings.meta_data_edit = false;
    gui_settings.meta_data_unlocked = false;

    if gui_settings.chart_scale <= 0.0 {
        gui_settings.chart_scale = 1.0;
    }

    let data_lock = Arc::new(RwLock::new(DataPoint::default()));
    let img_lock = Arc::new(RwLock::new(Array2::from_shape_fn((1, 1), |(_, _)| 0.0)));
    let pixel_lock = Arc::new(RwLock::new(SelectedPixel::default()));
    let scaling_lock = Arc::new(RwLock::new(1));
    let md_lock = Arc::new(RwLock::new(DotthzMetaData::default()));
    let mut progress_lock = HashMap::new();
    if let Ok(mut filters) = FILTER_REGISTRY.lock() {
        for filter in filters.iter_mut() {
            progress_lock.insert(filter.config().name, Arc::new(RwLock::new(None)));
            gui_settings
                .progress_bars
                .insert(filter.config().name, None);
        }
    }
    let (config_tx, config_rx): (Sender<ConfigCommand>, Receiver<ConfigCommand>) = mpsc::channel();

    let abort_flag = Arc::new(AtomicBool::new(false));

    let gui_communication = GuiThreadCommunication {
        abort_flag: abort_flag.clone(),
        md_lock: md_lock.clone(),
        data_lock: data_lock.clone(),
        pixel_lock: pixel_lock.clone(),
        scaling_lock: scaling_lock.clone(),
        img_lock: img_lock.clone(),
        progress_lock: progress_lock.clone(),
        gui_settings: gui_settings.clone(),
        config_tx,
    };

    let main_communication = MainThreadCommunication {
        abort_flag,
        md_lock,
        data_lock,
        pixel_lock,
        scaling_lock,
        img_lock,
        progress_lock,
        gui_settings: gui_settings.clone(),
        config_rx,
    };

    let _main_thread_handler = thread::spawn(|| {
        main_thread(main_communication);
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

    eframe::run_native(
        "THz Image Explorer",
        options,
        Box::new(|ctx| {
            let mut fonts = egui::FontDefinitions::default();
            egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
            ctx.egui_ctx.set_fonts(fonts);
            egui_extras::install_image_loaders(&ctx.egui_ctx);
            ctx.egui_ctx.set_visuals(Visuals::dark());
            Ok(Box::new(THzImageExplorer::new(ctx, gui_communication)))
        }),
    )
    .expect("Failed to launch GUI");
}
