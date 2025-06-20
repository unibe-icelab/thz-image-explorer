//! This module provides the `Filter` trait and related structures for managing filters and their configuration.
//! Filters can be applied to processed data (`ScannedImage`) and customized through settings.
//! It also implements a global, thread-safe registry for managing filters dynamically.

use crate::config::{ConfigCommand, ThreadCommunication};
use crate::data_container::ScannedImageFilterData;
use crate::gui::application::GuiSettingsContainer;
use crate::gui::toggle_widget::toggle;
use bevy_egui::egui;
use chrono::Utc;
#[allow(unused_imports)] // this dependency is required by the `register_filter` macro
use ctor::ctor;
use downcast_rs::Downcast;
use ndarray::Array1;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::{Arc, Mutex, RwLock};
use uuid::Uuid;

pub trait CopyStaticFieldsTrait: Downcast {
    fn copy_static_fields_from(&mut self, other: &dyn CopyStaticFieldsTrait);
}
downcast_rs::impl_downcast!(CopyStaticFieldsTrait);

/// The `Filter` trait defines the structure and behavior of an image filter.
///
/// Filters must implement:
/// - A `new` function to initialize a filter with default parameters.
/// - A `filter` function to apply the filter to a `ScannedImage`.
/// - A `config` function to provide metadata and parameters for the filter.
///
/// **Example**:
/// ```rust
/// use crate::filters::filter::{Filter, ScannedImage, GuiSettingsContainer};
///
/// struct ExampleFilter;
///
/// impl Filter for ExampleFilter {
///     fn new() -> Self { ExampleFilter }
///
///     fn filter(&self, scan: &mut ScannedImage, gui_settings: &mut GuiSettingsContainer) {
///         // Apply filter logic here
///     }
///
///     fn config(&self) -> FilterConfig {
///         FilterConfig {
///             name: "Example Filter".to_string(),
///             domain: FilterDomain::Time,
///             parameters: vec![]
///         }
///     }
/// }
/// ```
pub trait Filter: Send + Sync + Debug + CloneBoxedFilter + CopyStaticFieldsTrait {
    /// Creates a new instance of the filter with default parameters.
    fn new() -> Self
    where
        Self: Sized;

    /// Resets the filter to its initial state, allowing it to be reused.
    fn reset(&mut self, time: &Array1<f32>, shape: &[usize]);

    /// Returns the filter configuration, including name and domain.
    fn config(&self) -> FilterConfig;

    /// Applies the filter to the given `ScannedImage`.
    ///
    /// # Arguments
    ///
    /// - `filter_data`: A mutable reference to the image to be processed.
    /// - `gui_settings`: Mutable reference to GUI settings associated with the filter.
    fn filter(
        &mut self,
        input_data: &ScannedImageFilterData,
        gui_settings: &mut GuiSettingsContainer,
        progress_lock: &mut Arc<RwLock<Option<f32>>>,
        abort_flag: &Arc<AtomicBool>,
    ) -> ScannedImageFilterData;

    /// Renders the filter configuration in the GUI.
    /// make sure to return the `egui::Reponse` of the GUI elements. This way, the application
    /// can detect if any GUI element has been changed and will request a calculation update.
    ///
    /// # Example:
    ///
    /// ```rust
    /// fn ui(&mut self, ui: &mut Ui, _thread_communication: &mut ThreadCommunication) -> egui::Response {
    ///     let mut final_response = ui.allocate_response(egui::Vec2::ZERO, egui::Sense::hover());
    ///
    ///     let response_x = ui.horizontal(|ui| {
    ///         ui.label("Tilt X: ");
    ///         ui.add(egui::Slider::new(&mut self.tilt_x, -15.0..=15.0).suffix(" deg"))
    ///     }).inner; // Get the slider's response
    ///
    ///     let response_y = ui.horizontal(|ui| {
    ///         ui.label("Tilt Y: ");
    ///         ui.add(egui::Slider::new(&mut self.tilt_y, -15.0..=15.0).suffix(" deg"))
    ///     }).inner; // Get the slider's response
    ///
    ///     // Merge responses to track interactivity
    ///     final_response |= response_x.clone();
    ///     final_response |= response_y.clone();
    ///
    ///     // Only mark changed if any slider was changed (not just hovered)
    ///     if response_x.changed() || response_y.changed() {
    ///         final_response.mark_changed();
    ///     }
    ///
    ///     final_response
    /// }
    /// ```
    ///
    fn ui(
        &mut self,
        ui: &mut egui::Ui,
        thread_communication: &mut ThreadCommunication,
        panel_width: f32,
    ) -> egui::Response;
}

/// The `FilterDomain` enum specifies whether a filter operates in the time or frequency domain.
///
/// - `Time`: The filter processes data in the time domain.
/// - `Frequency`: The filter processes data in the frequency domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterDomain {
    TimeBeforeFFTPrioFirst,
    TimeBeforeFFT,
    Frequency,
    TimeAfterFFT,
    TimeAfterFFTPrioLast,
}

/// A structure representing the configuration and metadata of a filter.
///
/// This includes:
/// - The filter's name.
/// - The domain (time or frequency).
/// - A list of configurable parameters.
///
/// **Fields**:
/// - `name`: A human-readable name for the filter.
/// - `domain`: The working domain, represented as a `FilterDomain`.
#[derive(Debug, Clone)]
pub struct FilterConfig {
    pub name: String,
    pub domain: FilterDomain,
}

pub trait CloneBoxedFilter {
    fn clone_box(&self) -> Box<dyn Filter>;
}

impl<T> CloneBoxedFilter for T
where
    T: 'static + Filter + Clone,
{
    fn clone_box(&self) -> Box<dyn Filter> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn Filter> {
    fn clone(&self) -> Box<dyn Filter> {
        self.as_ref().clone_box()
    }
}

/// A registry to manage and retrieve registered filters.
///
/// The `FilterRegistry` provides functionality to:
/// - Register new filters.
/// - Retrieve filters by name.
///
/// **Example**:
/// ```rust
/// use crate::filters::filter::{FilterRegistry, FILTER_REGISTRY};
///
/// // Register a filter
/// FilterRegistry::register_filter::<YourFilter>();
///
/// // Retrieve a filter
/// if let Some(filter) = FILTER_REGISTRY.lock().unwrap().get_filter("YourFilterName") {
///     println!("Filter found: {:?}", filter);
/// } else {
///     println!("Filter not found");
/// }
/// ```
#[derive(Debug)]
pub struct FilterRegistry {
    pub filters: HashMap<String, Box<dyn Filter>>,
}

impl FilterRegistry {
    /// Registers a new filter of type `F` into the global registry.
    ///
    /// The filter instance is created and added to the global `FILTER_REGISTRY`.
    ///
    /// # Type Parameters
    ///
    /// - `F`: A type that implements the `Filter` trait.
    pub fn register_filter<F: Filter + 'static>() {
        // Create an instance of the filter
        let filter_instance = F::new();

        // Get the UUID for this filter type
        let uuid = Uuid::new_v4().to_string();

        // Store mapping from filter name to UUID
        let name = filter_instance.config().name.clone();
        {
            let mut map = FILTER_INSTANCE_UUIDS.lock().unwrap();
            map.insert(name, uuid.clone());
        }

        // Register the filter in the registry
        let mut registry = FILTER_REGISTRY.lock().unwrap();
        registry
            .filters
            .insert(uuid.clone(), Box::new(filter_instance));
    }

    /// Retrieves a registered filter by its name.
    ///
    /// # Arguments
    ///
    /// - `name`: The name of the filter to retrieve.
    ///
    /// # Returns
    ///
    /// An optional reference to the filter if found in the registry.
    pub fn get_filter(&self, name: &str) -> Option<&Box<dyn Filter>> {
        self.filters.get(name)
    }

    /// Returns a mutable iterator over all the registered filters in the registry.
    ///
    /// This method allows for iterating through filters while also enabling modification of
    /// each registered filter.
    ///
    /// # Returns
    /// A mutable iterator (`impl Iterator<Item = &mut Box<dyn Filter>>`) over all filters
    /// in the registry.
    ///
    /// # Behavior
    /// - Provides mutable access to each registered filter, allowing for modifications.
    /// - Iterates only through the values of the `HashMap`, not the keys.
    ///
    /// # Example
    /// ```rust
    /// use crate::filters::filter::{FilterRegistry, FILTER_REGISTRY};
    ///
    /// {
    ///     // Acquire a lock on the global registry and iterate mutably.
    ///     let mut registry = FILTER_REGISTRY.lock().unwrap();
    ///     for filter in registry.iter_mut() {
    ///         // Example modification: Clear the parameters for each filter.
    ///         filter.config().parameters.clear(); // Hypothetical use case
    ///     }
    /// }
    /// ```
    ///
    /// # Notes
    /// - This method is strictly for mutable access to the filters.
    /// - For immutable access during iteration, use the [`IntoIterator`] implementation for `&FilterRegistry`.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Box<dyn Filter>> {
        self.filters.values_mut()
    }
}

/// Implements `IntoIterator` for `&FilterRegistry`.
///
/// This allows iterating over all the registered filters in the registry by borrowing it (non-mutably).
///
/// # Associated Types
/// - `Item`: A reference (`&Box<dyn Filter>`) to each filter in the registry during iteration.
/// - `IntoIter`: The iterator type used for traversing the filters.
///
/// # Behavior
/// This implementation provides a view over the filter registry's internal data and returns each
/// registered filter as a boxed trait object during iteration. This is useful for actions like
/// reading filter configurations or information without modifying the registry.
///
/// # Example
/// ```rust
/// use crate::filters::filter::{FilterRegistry, FILTER_REGISTRY};
///
/// // Parallel iteration
/// {
///     let registry = FILTER_REGISTRY.lock().unwrap();
///     for filter in &*registry {
///         println!("Filter name: {}", filter.config().name);
///     }
/// }
/// ```
///
/// # Notes
/// - The method is designed for cases where only references to the filters are required.
/// - To modify the filters during iteration, use the `iter_mut` method provided by `FilterRegistry`.
impl<'a> IntoIterator for &'a FilterRegistry {
    /// The type of items returned during iteration — a reference to the boxed filter.

    type Item = &'a Box<dyn Filter>;
    /// The iterator type used for traversing values of the `HashMap`.

    type IntoIter = std::collections::hash_map::Values<'a, String, Box<dyn Filter>>;

    /// Initializes an iterator over the values (i.e., filters) in the registry.
    ///
    /// # Returns
    /// A `HashMap::Values` iterator over the registered filters.
    fn into_iter(self) -> Self::IntoIter {
        self.filters.values()
    }
}

/// A global, thread-safe filter registry.
///
/// This allows filters to be registered and accessed globally, across threads.
///
/// **Usage Example**:
/// ```rust
/// use crate::filters::filter::FILTER_REGISTRY;
///
/// if let Some(filter) = FILTER_REGISTRY.lock().unwrap().get_filter("Deconvolution") {
///     println!("Filter found: {}", filter.config().name);
/// } else {
///     println!("Filter not found");
/// }
/// ```
pub static FILTER_REGISTRY: Lazy<Mutex<FilterRegistry>> = Lazy::new(|| {
    Mutex::new(FilterRegistry {
        filters: HashMap::new(),
    })
});

static FILTER_INSTANCE_UUIDS: Lazy<Mutex<HashMap<String, String>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

pub fn draw_filters(
    ui: &mut egui::Ui,
    thread_communication: &mut ThreadCommunication,
    domain: FilterDomain,
    right_panel_width: f32,
) {
    let now = Utc::now().timestamp_millis();

    if let Ok(mut filters) = FILTER_REGISTRY.lock() {
        let filter_entries: Vec<(String, usize)> =
            filters.filters.keys().cloned().zip(0..).collect();

        // 1. Check if any filter is busy for more than 0.5s
        let mut busy_long_enough = false;
        for (uuid, _) in &filter_entries {
            if let Some(Some(_)) = thread_communication.gui_settings.progress_bars.get(uuid) {
                if let Some(start) = thread_communication
                    .gui_settings
                    .progress_start_time
                    .get(uuid)
                {
                    if now - *start > 500 {
                        busy_long_enough = true;

                        // TODO check if we can improve this!!!

                        log::info!("Clearing all config commands from the queue since this one takes so long and enter the latest one back again");
                        let mut r = None;
                        while !thread_communication.config_rx.is_empty() {
                            r = thread_communication.config_rx.recv().ok();
                        }

                        if let Some(r) = r {
                            thread_communication
                                .config_tx
                                .send(r)
                                .expect("Failed to send config task");
                        }

                        break;
                    }
                }
            }
        }

        // 2. Update progress and draw UI
        for (idx, (uuid, _)) in filter_entries.iter().enumerate() {
            let filter = filters.filters.get_mut(uuid).unwrap();
            if filter.config().domain != domain {
                continue;
            }

            // Update progress bar value and start time for this filter
            if let Some(mut update) = thread_communication
                .gui_settings
                .last_progress_bar_update
                .get_mut(uuid)
            {
                if now - *update > 100 {
                    if let Some(progress) = thread_communication.progress_lock.get(uuid) {
                        *update = now;
                        if let Ok(progress) = progress.read() {
                            if let Some(progress_entry) = thread_communication
                                .gui_settings
                                .progress_bars
                                .get_mut(uuid)
                            {
                                let was_none = progress_entry.is_none();
                                *progress_entry = *progress;
                                if progress.is_some() && was_none {
                                    thread_communication
                                        .gui_settings
                                        .progress_start_time
                                        .insert(uuid.clone(), now);
                                }
                                if progress.is_none() {
                                    thread_communication
                                        .gui_settings
                                        .progress_start_time
                                        .remove(uuid);
                                }
                            }
                        }
                    }
                }
            }

            let mut filter_is_active = true;

            ui.vertical(|ui| {
                ui.separator();
                ui.horizontal(|ui| {
                    ui.heading(filter.config().clone().name);

                    // Progress bar, abort button, and toggle are always enabled
                    if let Some(progress) =
                        thread_communication.gui_settings.progress_bars.get(uuid)
                    {
                        if let Some(p) = progress {
                            if *p >= 0.0 {
                                ui.add_space(
                                    ui.available_width()
                                        - ui.spacing().interact_size.y * 2.0
                                        - 15.0
                                        - 50.0
                                        - 55.0,
                                );
                                ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Wait);

                                if ui
                                    .button(format!("{}", egui_phosphor::regular::X_SQUARE))
                                    .on_hover_text("Abort the current calculation")
                                    .clicked()
                                {
                                    thread_communication.abort_flag.store(true, Relaxed);
                                }
                                ui.label(format!("{} %", (p * 100.0) as u8));
                            }
                        } else {
                            ui.add_space(
                                ui.available_width()
                                    - ui.spacing().interact_size.y * 2.0
                                    - 15.0
                                    - 50.0
                                    - 55.0,
                            );
                            ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Default);
                        }
                    } else {
                        ui.add_space(
                            ui.available_width() - ui.spacing().interact_size.y * 2.0 - 15.0 - 50.0,
                        );
                        ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Default);
                    }

                    // Enable only the toggle and computation time label
                    if let Ok(mut filters_active) = thread_communication.filters_active_lock.write()
                    {
                        if let Ok(filter_computation_time) =
                            thread_communication.filter_computation_time_lock.read()
                        {
                            if let Some(t) = filter_computation_time.get(uuid) {
                                if idx < filter_computation_time.len() {
                                    ui.label(format!("{:.2} ms", t.as_millis()));
                                } else {
                                    ui.label("N/A ms");
                                }
                            } else {
                                ui.label("N/A ms");
                            }
                        } else {
                            ui.label("N/A ms");
                        }
                        if let Some(active) = filters_active.get_mut(uuid) {
                            ui.add(toggle(active));
                            filter_is_active = *active;
                        }
                    }
                });

                // Only enable the filter config UI if not busy for >0.5s and filter is active
                ui.add_enabled_ui(!busy_long_enough && filter_is_active, |ui| {
                    if filter
                        .ui(ui, thread_communication, right_panel_width)
                        .changed()
                    {
                        thread_communication
                            .config_tx
                            .send(ConfigCommand::UpdateFilter(uuid.clone()))
                            .unwrap();
                    }
                });
            });

            // Draw progress bar below (optional)
            if let Some(progress) = thread_communication.gui_settings.progress_bars.get(uuid) {
                if let Some(p) = progress {
                    if *p > 0.0 {
                        ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Wait);
                        ui.horizontal(|ui| {
                            ui.add(
                                egui::ProgressBar::new(*p)
                                    .text(format!("{} %", (p * 100.0) as u8))
                                    .desired_width(right_panel_width - 50.0),
                            );
                            if ui
                                .button(format!("{}", egui_phosphor::regular::X_SQUARE))
                                .on_hover_text("Abort the current calculation")
                                .clicked()
                            {
                                thread_communication.abort_flag.store(true, Relaxed);
                            }
                        });
                        ui.ctx().request_repaint();
                    } else {
                        ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Default);
                    }
                } else {
                    ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Default);
                }
            }
        }
    }
}
