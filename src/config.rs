//! This module defines the configuration and communication structures
//! used in the application for managing data flow between threads
//! and processing FFT settings.
//!
//! The module provides:
//! - Command routing between threads using the `ConfigCommand` enum
//! - Configuration settings for FFT processing via the `ConfigContainer`
//! - Thread-safe data sharing through the `ThreadCommunication` structure

use crate::data_container::{PlotDataContainer, ScannedImageFilterData};
use crate::gui::application::GuiSettingsContainer;
use crate::gui::matrix_plot::{SelectedPixel, ROI};
use crate::math_tools::FftWindowType;
use bevy::prelude::Resource;
use bevy_voxel_plot::InstanceData;
use crossbeam_channel::{Receiver, Sender, TrySendError};
use dotthz::DotthzMetaData;
use ndarray::{Array1, Array2, Array3};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, RwLock};
use std::time::Duration;

/// Sends a configuration command to the processing thread, ensuring only the latest command is queued.
///
/// This function attempts to send a command through the provided channel. If the channel is full,
/// it removes the oldest command and replaces it with the new one, ensuring that only the most
/// recent command is processed.
///
/// # Arguments
/// * `thread_communication` - The communication structure containing the command channels
/// * `cmd` - The configuration command to be sent
///
/// # Example
/// ```
/// let cmd = ConfigCommand::SetFFTWindowLow(2.0);
/// send_latest_config(&thread_communication, cmd);
/// ```
pub fn send_latest_config(thread_communication: &ThreadCommunication, cmd: ConfigCommand) {
    match thread_communication.config_tx.try_send(cmd.clone()) {
        Ok(_) => {}
        Err(TrySendError::Full(_)) => {
            // Remove the old command and send the new one
            let _ = thread_communication.config_rx.recv();
            let _ = thread_communication.config_tx.try_send(cmd);
        }
        Err(_) => {}
    }
}

/// Enum representing the various commands sent to the configuration thread.
///
/// These commands are used to control various aspects of the application processing,
/// such as opening files, setting FFT parameters, updating filtering windows, etc.
#[derive(Clone, Debug)]
pub enum ConfigCommand {
    /// Command to open a specified file.
    /// The file is identified using a `PathBuf`, and its type is determined based on its extension.
    OpenFile(PathBuf),

    /// Command to open a specified reference file.
    /// The file is identified using a `PathBuf`, and its type is determined based on its extension.
    OpenRef(PathBuf),

    /// Command to save data to a specified file.
    /// The file is identified using a `PathBuf`, and its type is determined based on its extension.
    SaveFile(PathBuf),

    /// Command to load metadata from a specified file.
    /// The file is identified using a `PathBuf`.
    LoadMetaData(PathBuf),

    /// Command to update metadata in a specified file.
    /// The file is identified using a `PathBuf`.
    UpdateMetaData(PathBuf),

    SaveROIs(PathBuf),

    /// Command to set the lower bound of the FFT window.
    SetFFTWindowLow(f32),

    /// Command to set the upper bound of the FFT window.
    SetFFTWindowHigh(f32),

    /// Command to toggle logarithmic plotting of FFT data.
    /// A `true` value enables logarithmic plotting, while `false` disables it.
    SetFFTLogPlot(bool),

    /// Command to enable or disable normalization of FFT data.
    /// A `true` value enables normalization, while `false` disables it.
    SetFFTNormalization(bool),

    /// Command to enable or disable averaging in frequency domain.
    /// A `true` value enables averaging in frequency domain, while `false` enables averaging in time domain.
    SetAvgInFourierSpace(bool),

    /// Command to set the FFT frequency resolution.
    /// The resolution is specified as a `f32` value in Hz.
    SetFFTResolution(f32),

    /// Command to set the type of FFT window to be used.
    /// The window type is represented by the [`FftWindowType`] enum.
    SetFftWindowType(FftWindowType),

    /// Command to adjust the downscaling factor.
    /// This affects the resolution of the processed image and data.
    SetDownScaling(usize),

    /// Command to update the currently selected pixel in the image.
    /// The selected pixel is represented by the [`SelectedPixel`] structure.
    SetSelectedPixel(SelectedPixel),

    /// Command to update all custom filters in the processing pipeline.
    /// This triggers recalculation of all filter results.
    UpdateFilters,

    /// Command to update a specific custom filter identified by its UUID.
    /// This allows for selective recalculation when only one filter changes.
    UpdateFilter(String),

    /// Calls an update of the material calculation process. (refractive index, etc.)
    UpdateMaterialCalculation,

    /// Command to add a new Region of Interest (ROI) to the processing pipeline.
    AddROI(String, ROI),

    /// Command to update an existing Region of Interest (ROI) identified by its name.
    UpdateROI(String, ROI),

    /// Command to delete a specific Region of Interest (ROI) identified by its name.
    DeleteROI(String),

    /// Command to set the reference data for processing.
    SetReference(String),

    /// Command to set the sample data for processing.
    SetSample(String),

    /// Command to set the material thickness for processing.
    SetMaterialThickness(f32),
}

/// A container for storing configuration settings related to FFT and filtering processes.
///
/// This struct stores parameters such as FFT window bounds, filters, time windows,
/// as well as options for plotting, normalization, and resolution.
#[derive(Clone)]
pub struct ConfigContainer {
    /// Lower and upper bounds of the FFT window used for processing.
    pub fft_window: [f32; 2],

    /// Type of FFT window function to be used.
    /// See [`FftWindowType`] for details.
    pub fft_window_type: FftWindowType,

    pub scale_factor: usize,

    /// Flag indicating whether to use logarithmic plotting for FFT results.
    pub fft_log_plot: bool,

    /// Flag indicating whether to normalize the FFT results.
    pub normalize_fft: bool,

    /// Flag indicating whether to average in frequency domain or not.
    pub avg_in_fourier_space: bool,

    /// The frequency resolution (distance between frequency bins) for the FFT.
    pub fft_df: f32,
}

impl Default for ConfigContainer {
    /// Provides default values for the configuration container.
    ///
    /// The defaults are:
    /// - `fft_window`: `[1.0, 7.0]`
    /// - `fft_window_type`: `FftWindowType::AdaptedBlackman`
    /// - `scale_factor`: `1`
    /// - `fft_log_plot`: `false`
    /// - `normalize_fft`: `false`
    /// - `avg_in_fourier_space`: `true`
    /// - `fft_df`: `1.0`
    fn default() -> Self {
        ConfigContainer {
            fft_window: [1.0, 7.0],
            fft_window_type: FftWindowType::AdaptedBlackman,
            scale_factor: 1,
            fft_log_plot: false,
            normalize_fft: false,
            avg_in_fourier_space: true,
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
    /// Atomic flag used to signal threads to abort their current processing.
    pub abort_flag: Arc<AtomicBool>,

    /// Lock for the metadata (`DotthzMetaData`) shared across threads.
    pub md_lock: Arc<RwLock<DotthzMetaData>>,

    /// Lock for the [`PlotDataContainer`] containing signal data.
    pub data_lock: Arc<RwLock<PlotDataContainer>>,

    /// Lock for the filtered 3D matrix data.
    /// Contains processed data after applying filters.
    pub filtered_data_lock: Arc<RwLock<Array3<f32>>>,

    /// Lock for the time array associated with filtered data.
    pub filtered_time_lock: Arc<RwLock<Array1<f32>>>,

    /// Lock for the voxel plot visualization data.
    /// Contains instances of voxels and their dimensions (width, height, depth).
    pub voxel_plot_instances_lock: Arc<RwLock<(Vec<InstanceData>, f32, f32, f32)>>,

    /// Lock for the 2D array representing the intensity image.
    pub img_lock: Arc<RwLock<Array2<f32>>>,

    /// Index for the scaling filter in the filter chain.
    /// Used to identify where scaling processing occurs in the sequence.
    pub scaling_index: usize,

    /// Index for the FFT filter in the filter chain.
    /// Used to identify where FFT processing occurs in the sequence.
    pub fft_index: usize,

    /// Index for the inverse FFT filter in the filter chain.
    /// Used to identify where IFFT processing occurs in the sequence.
    pub ifft_index: usize,

    /// Lock for tracking computation time of each filter.
    /// Maps filter UUIDs to their processing duration.
    pub filter_computation_time_lock: Arc<RwLock<HashMap<String, Duration>>>,

    /// Lock for storing the data processed by each filter.
    /// Contains a vector of filter output data for each step in the chain/pipeline.
    pub filter_data_pipeline_lock: Arc<RwLock<Vec<ScannedImageFilterData>>>,

    /// Lock for the ordered sequence of filter UUIDs to be applied.
    /// Determines the processing pipeline order.
    pub filter_chain_lock: Arc<RwLock<Vec<String>>>,

    /// Lock for mapping filter UUIDs to their index in the filter data vector.
    pub filter_uuid_to_index_lock: Arc<RwLock<HashMap<String, usize>>>,

    /// Lock for tracking which filters are currently active.
    /// Maps filter UUIDs to boolean activation status.
    pub filters_active_lock: Arc<RwLock<HashMap<String, bool>>>,

    /// GUI-specific settings stored in the [`GuiSettingsContainer`].
    pub gui_settings: GuiSettingsContainer,

    /// Channel for sending configuration commands to the processing thread.
    pub config_tx: Sender<ConfigCommand>,

    /// Channel for receiving configuration commands in the processing thread.
    pub config_rx: Receiver<ConfigCommand>,

    /// Channel for sending configuration ROI to the GUI thread.
    pub roi_tx: Sender<(String, ROI)>,

    /// Channel for receiving configuration ROI in the GUI thread.
    pub roi_rx: Receiver<(String, ROI)>,

    /// Lock for tracking filter processing progress.
    /// Maps filter UUIDs to their current progress (0.0 to 1.0, or None if inactive).
    pub progress_lock: HashMap<String, Arc<RwLock<Option<f32>>>>,
}
