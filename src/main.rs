use crate::config::{ConfigCommand, ThreadCommunication};
use crate::data_container::DataPoint;
use crate::data_thread::main_thread;
use crate::gui::application::{update_gui, GuiSettingsContainer, THzImageExplorer};
use crate::gui::center_panel::rotator_system;
use crate::gui::matrix_plot::SelectedPixel;
use crate::gui::threed_plot::{plot_3d_camera_controller, setup_plot_3d_render};
use bevy::prelude::*;
use bevy_egui::{EguiContextPass, EguiContexts, EguiPlugin};
use crossbeam_channel::{Receiver, Sender};
use dotthz::DotthzMetaData;
use eframe::egui::{vec2, ViewportBuilder, Visuals};
use eframe::{egui, icon_data};
use ndarray::{Array2, Array3};
use preferences::{AppInfo, Preferences};
use std::sync::{Arc, RwLock};
use std::thread;

mod config;
mod data_container;
mod data_thread;
mod filters;
mod gui;
mod io;
mod math_tools;
mod update;

const APP_INFO: AppInfo = AppInfo {
    name: "THz Image Explorer",
    author: "Linus Leo Stöckli",
};

fn spawn_data_thread(mut state: ResMut<ThreadCommunication>) {
    let state = state.clone(); // If ThreadCommunication is Arc/Mutex or cloneable
    thread::spawn(move || {
        main_thread(state);
    });
}

fn setup_fonts(mut contexts: EguiContexts) {
    let mut fonts = egui::FontDefinitions::default();
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
    contexts.ctx_mut().set_fonts(fonts);
    egui_extras::install_image_loaders(&contexts.ctx_mut());
    contexts.ctx_mut().set_visuals(Visuals::dark());
}

// --- Main ---
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
    let filtered_data_lock = Arc::new(RwLock::new(Array3::from_shape_fn(
        (1, 1, 1),
        |(_, _, _)| 0.0,
    )));
    let pixel_lock = Arc::new(RwLock::new(SelectedPixel::default()));
    let scaling_lock = Arc::new(RwLock::new(1));
    let md_lock = Arc::new(RwLock::new(DotthzMetaData::default()));

    let (config_tx, config_rx): (Sender<ConfigCommand>, Receiver<ConfigCommand>) =
        crossbeam_channel::unbounded();
    let thread_communication = ThreadCommunication {
        md_lock: md_lock.clone(),
        data_lock: data_lock.clone(),
        filtered_data_lock: filtered_data_lock.clone(),
        pixel_lock: pixel_lock.clone(),
        scaling_lock: scaling_lock.clone(),
        img_lock: img_lock.clone(),
        gui_settings: gui_settings.clone(),
        config_tx,
        config_rx,
    };
    // Start Bevy app
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "THz Image Explorer".into(),
                resolution: (gui_settings.x, gui_settings.y).into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(EguiPlugin {
            enable_multipass_for_primary_context: true,
        })
        .insert_resource(thread_communication.clone())
        .insert_non_send_resource(THzImageExplorer::new(thread_communication))
        .add_systems(Startup, setup_plot_3d_render)
        .add_systems(Startup, setup_fonts)
        .add_systems(Startup, spawn_data_thread)
        .add_systems(EguiContextPass, update_gui)
        //.add_systems(Update, rotator_system)
        .add_systems(Update, plot_3d_camera_controller)
        .run();
}
