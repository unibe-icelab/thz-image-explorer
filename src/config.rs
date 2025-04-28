//! This module defines the configuration and communication structures
//! used in the application for managing data flow between threads
//! and processing FFT settings.

use crate::data_container::DataPoint;
use crate::gui::application::GuiSettingsContainer;
use crate::gui::matrix_plot::SelectedPixel;
use crate::math_tools::FftWindowType;
use bevy::prelude::Resource;
use dotthz::DotthzMetaData;
use ndarray::{Array1, Array2, Array3};
use std::path::PathBuf;
use crossbeam_channel::{Receiver, Sender};
use std::sync::{Arc, RwLock};

/// Enum representing the various commands sent to the configuration thread.
///
/// These commands are used to control various aspects of the application processing,
/// such as opening files, setting FFT parameters, updating filtering windows, etc.
pub enum ConfigCommand {
    /// Command to open a specified file.
    /// The file is identified using a `PathBuf`, and its type is determined based on its extension.
    OpenFile(PathBuf),

    /// The file is identified using a `PathBuf`, and its type is determined based on its extension.
    SaveFile(PathBuf),

    /// The file is identified using a `PathBuf`.
    LoadMetaData(PathBuf),

    /// The file is identified using a `PathBuf`.
    UpdateMetaData(PathBuf),

    /// Command to set the lower bound of the FFT window.
    SetFFTWindowLow(f32),

    /// Command to set the upper bound of the FFT window.
    SetFFTWindowHigh(f32),

    /// Command to set the lower bound of the FFT filter.
    SetFFTFilterLow(f32),

    /// Command to set the upper bound of the FFT filter.
    SetFFTFilterHigh(f32),

    /// Command to set the time window for signal analysis.
    /// The window is specified as an array of two values, `[start, stop]`.
    SetTimeWindow([f32; 2]),

    /// Command to toggle logarithmic plotting of FFT data.
    /// A `true` value enables logarithmic plotting, while `false` disables it.
    SetFFTLogPlot(bool),

    /// Command to enable or disable normalization of FFT data.
    /// A `true` value enables normalization, while `false` disables it.
    SetFFTNormalization(bool),

    /// Command to set the FFT frequency resolution.
    /// The resolution is specified as a `f32` value in Hz.
    SetFFTResolution(f32),

    /// Command to set the type of FFT window to be used.
    /// The window type is represented by the [`FftWindowType`] enum.
    SetFftWindowType(FftWindowType),

    /// Command to adjust the downscaling factor.
    /// This affects the resolution of the processed image and data.
    SetDownScaling,

    /// Command to update the currently selected pixel in the image.
    /// The selected pixel is represented by the [`SelectedPixel`] structure.
    SetSelectedPixel(SelectedPixel),

    /// Update Custom Filters
    UpdateFilters,
}

/// A container for storing configuration settings related to FFT and filtering processes.
///
/// This struct stores parameters such as FFT window bounds, filters, time windows,
/// as well as options for plotting, normalization, and resolution.
#[derive(Clone)]
pub struct ConfigContainer {
    /// Lower and upper bounds of the FFT window used for processing.
    pub fft_window: [f32; 2],

    /// Lower and upper bounds of the FFT filter applied to the frequency spectrum.
    pub fft_filter: [f32; 2],

    /// Start and end times of the time window applied to the signal data.
    pub time_window: [f32; 2],

    /// Type of FFT window function to be used.
    /// See [`FftWindowType`] for details.
    pub fft_window_type: FftWindowType,

    /// Flag indicating whether to use logarithmic plotting for FFT results.
    pub fft_log_plot: bool,

    /// Flag indicating whether to normalize the FFT results.
    pub normalize_fft: bool,

    /// The frequency resolution (distance between frequency bins) for the FFT.
    pub fft_df: f32,
}

impl Default for ConfigContainer {
    /// Provides default values for the configuration container.
    ///
    /// The defaults are:
    /// - `fft_window`: `[1.0, 7.0]`
    /// - `fft_filter`: `[0.0, 10.0]`
    /// - `time_window`: `[1000.0, 1050.0]`
    /// - `fft_window_type`: `FftWindowType::AdaptedBlackman`
    /// - `fft_log_plot`: `false`
    /// - `normalize_fft`: `false`
    /// - `fft_df`: `1.0`
    fn default() -> Self {
        ConfigContainer {
            fft_window: [1.0, 7.0],
            fft_filter: [0.0, 10.0],
            time_window: [1000.0, 1050.0],
            fft_window_type: FftWindowType::AdaptedBlackman,
            fft_log_plot: false,
            normalize_fft: false,
            fft_df: 1.0,
        }
    }
}

/// Structure for handling communication related to the main thread.
///
/// This struct is used for managing the reception of configuration commands (`ConfigCommand`)
/// and sharing data locks between the GUI and the main processing thread.
#[derive(Resource, Clone)]
pub struct ThreadCommunication {
    /// Lock for the metadata (`DotthzMetaData`) shared across threads.
    pub md_lock: Arc<RwLock<DotthzMetaData>>,

    /// Lock for the [`DataPoint`] containing signal data.
    pub data_lock: Arc<RwLock<DataPoint>>,

    /// Lock for the filtered_data containing the filtered 3D matrix.
    pub filtered_data_lock: Arc<RwLock<Array3<f32>>>,

    /// Lock for the time containing the time array of the filtered data.
    pub filtered_time_lock: Arc<RwLock<Array1<f32>>>,

    /// Lock for the currently selected pixel in the image.
    pub pixel_lock: Arc<RwLock<SelectedPixel>>,

    /// Lock for the image scaling factor (used for downscaling).
    pub scaling_lock: Arc<RwLock<u8>>,

    /// Lock for the 2D array representing the intensity image.
    pub img_lock: Arc<RwLock<Array2<f32>>>,

    /// GUI-specific settings stored in the [`GuiSettingsContainer`].
    pub gui_settings: GuiSettingsContainer,

    pub config_tx: Sender<ConfigCommand>,
    pub config_rx: Receiver<ConfigCommand>,
}
