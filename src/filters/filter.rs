//! This module provides the `Filter` trait and related structures for managing filters and their configuration.
//! Filters can be applied to processed data (`ScannedImage`) and customized through settings.
//! It also implements a global, thread-safe registry for managing filters dynamically.

use crate::config::{ConfigCommand, ThreadCommunication};
use crate::data_container::ScannedImageFilterData;
use crate::gui::application::GuiSettingsContainer;
// this dependency is required by the `register_filter` macro
use bevy_egui::egui;
use chrono::Utc;
#[allow(unused_imports)]
use ctor::ctor;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::{Arc, Mutex, RwLock};

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
pub trait Filter: Send + Sync + Debug + CloneBoxedFilter {
    /// Creates a new instance of the filter with default parameters.
    fn new() -> Self
    where
        Self: Sized;
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
    TimeBeforeFFT,
    Frequency,
    TimeAfterFFT,
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

        // Extract the name from the filter's config
        let name = filter_instance.config().name.clone();

        // Register the filter in the registry
        let mut registry = FILTER_REGISTRY.lock().unwrap();
        registry.filters.insert(name, Box::new(F::new()));
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

pub fn draw_filters(
    ui: &mut egui::Ui,
    thread_communication: &mut ThreadCommunication,
    domain: FilterDomain,
    right_panel_width: f32,
) {
    // draw time domain filter after FFT
    if let Ok(mut filters) = FILTER_REGISTRY.lock() {
        let mut update_requested = false;
        for filter in filters.iter_mut() {
            if filter.config().domain != domain {
                continue;
            }
            if Utc::now().timestamp_millis()
                - thread_communication.gui_settings.last_progress_bar_update
                > 100
            {
                if let Some(progress) = thread_communication
                    .progress_lock
                    .get(&filter.config().name)
                {
                    thread_communication.gui_settings.last_progress_bar_update =
                        Utc::now().timestamp_millis();
                    if let Ok(progress) = progress.read() {
                        if let Some(progress_entry) = thread_communication
                            .gui_settings
                            .progress_bars
                            .get_mut(&filter.config().name)
                        {
                            *progress_entry = *progress;
                            thread_communication.gui_settings.filter_ui_active = progress.is_none();
                        }
                    }
                }
            }

            ui.vertical(|ui| {
                if !thread_communication.gui_settings.filter_ui_active {
                    ui.disable();
                }

                ui.separator();
                ui.heading(filter.config().clone().name);
                update_requested |= filter
                    .ui(ui, thread_communication, right_panel_width)
                    .changed();
            });

            if let Some(progress) = thread_communication
                .gui_settings
                .progress_bars
                .get(&filter.config().name)
            {
                if let Some(p) = progress {
                    if *p > 0.0 {
                        ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Wait);
                        ui.horizontal(|ui| {
                            ui.add(
                                // TODO: fix the width!
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
        if update_requested {
            thread_communication
                .config_tx
                .send(ConfigCommand::UpdateFilters)
                .unwrap();
        }
    }
}
