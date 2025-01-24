use crate::data::ScannedImage;

pub trait Filter {
    const DOMAIN: FilterDomain;

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
