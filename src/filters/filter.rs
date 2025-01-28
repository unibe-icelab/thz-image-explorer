use crate::data_container::ScannedImage;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Mutex;
#[allow(unused_imports)]
use ctor::ctor; // this dependency is required by the `register_filter` macro

pub trait Filter: Send + Sync + Debug {
    fn new() -> Self
    where
        Self: Sized;
    fn filter(&self, t: &mut ScannedImage);

    fn config(&self) -> FilterConfig;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterDomain {
    Time,
    Frequency,
}

/// A structure to hold filter configuration parameters
#[derive(Debug, Clone)]
pub struct FilterConfig {
    pub name: String,
    pub domain: FilterDomain,
    pub parameters: Vec<FilterParameter>,
}

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

#[derive(Debug, Clone)]
pub struct FilterParameter {
    pub name: String,
    pub kind: ParameterKind,
}

#[derive(Debug)]
pub struct FilterRegistry {
    filters: HashMap<String, Box<dyn Filter>>,
}

impl FilterRegistry {
    pub fn register_filter<F: Filter + 'static>() {
        // Create an instance of the filter
        let filter_instance = F::new();

        // Extract the name from the filter's config
        let name = filter_instance.config().name.clone();

        // Register the filter in the registry
        let mut registry = FILTER_REGISTRY.lock().unwrap();
        registry.filters.insert(name, Box::new(F::new()));
    }

    pub fn get_filter(&self, name: &str) -> Option<&Box<dyn Filter>> {
        self.filters.get(name)
    }
}

/// Global, thread-safe registry
/// '''rust
///
/// use crate::filters::filter::FILTER_REGISTRY;
///
///
/// if let Some(filter) = FILTER_REGISTRY.lock().unwrap().get_filter("Deconvolution") {
///     println!("Filter found: {}", filter.name());
/// } else {
///     println!("Filter not found");
/// }
/// '''
pub static FILTER_REGISTRY: Lazy<Mutex<FilterRegistry>> = Lazy::new(|| {
    Mutex::new(FilterRegistry {
        filters: HashMap::new(),
    })
});
