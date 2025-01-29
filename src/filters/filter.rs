//! This module provides the `Filter` trait and related structures for managing filters and their configuration.
//! Filters can be applied to processed data (`ScannedImage`) and customized through settings.
//! It also implements a global, thread-safe registry for managing filters dynamically.

use crate::data_container::ScannedImage;
use crate::gui::application::GuiSettingsContainer;
#[allow(unused_imports)]
use ctor::ctor; // this dependency is required by the `register_filter` macro
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Mutex;

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
pub trait Filter: Send + Sync + Debug {
    /// Creates a new instance of the filter with default parameters.
    fn new() -> Self
    where
        Self: Sized;

    /// Applies the filter to the given `ScannedImage`.
    ///
    /// # Arguments
    ///
    /// - `_scan`: A mutable reference to the image to be processed.
    /// - `gui_settings`: Mutable reference to GUI settings associated with the filter.
    fn filter(&self, _scan: &mut ScannedImage, gui_settings: &mut GuiSettingsContainer);

    /// Returns the filter configuration, including name, domain, and parameters.
    fn config(&self) -> FilterConfig;
}

/// The `FilterDomain` enum specifies whether a filter operates in the time or frequency domain.
///
/// - `Time`: The filter processes data in the time domain.
/// - `Frequency`: The filter processes data in the frequency domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterDomain {
    Time,
    Frequency,
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
/// - `parameters`: A vector of customizable filter parameters.#[derive(Debug, Clone)]
pub struct FilterConfig {
    pub name: String,
    pub domain: FilterDomain,
    pub parameters: Vec<FilterParameter>,
}

/// Represents a specific parameter for a filter configuration.
///
/// It can be of different types:
/// - `Int`: A signed integer.
/// - `UInt`: An unsigned integer.
/// - `Float`: A floating-point number.
/// - `Boolean`: A simple true/false value.
/// - `Slider`: A continuous range represented by a slider.
/// - `DoubleSlider`: Represents two value ranges with additional visual and logical constraints.
///
/// **Example**:
/// ```rust
/// use crate::filters::filter::{ParameterKind, FilterParameter};
///
/// let param = FilterParameter {
///     name: "Threshold".to_string(),
///     kind: ParameterKind::Slider {
///         value: 0.5,
///         show_in_plot: true,
///         min: 0.0,
///         max: 1.0,
///     }
/// };
/// ```
#[derive(Debug, Clone)]
pub enum ParameterKind {
    Int(isize),
    UInt(usize),
    Float(f64),
    Boolean(bool),
    Slider {
        value: f64,
        show_in_plot: bool,
        min: f64,
        max: f64,
    },
    DoubleSlider {
        values: [f64; 2],
        show_in_plot: bool,
        minimum_separation: f64,
        inverted: bool,
        min: f64,
        max: f64,
    },
}

/// Holds metadata and values for an individual filter parameter.
///
/// **Fields**:
/// - `name`: The name of the parameter.
/// - `kind`: The type and value of the parameter, represented as `ParameterKind`.
#[derive(Debug, Clone)]
pub struct FilterParameter {
    pub name: String,
    pub kind: ParameterKind,
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
    filters: HashMap<String, Box<dyn Filter>>,
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
