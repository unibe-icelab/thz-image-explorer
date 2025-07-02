use interp1d::Interp1d;
use ndarray::{Array1, Array2};
use serde::{Deserialize, Serialize};

/// Represents a Point Spread Function (PSF) used in spectroscopy and imaging analysis.
///
/// A PSF characterizes the response of an imaging system to a point source signal, providing
/// critical information about the system resolution and frequency characteristics.
/// This structure holds both scalar and multidimensional data derived from an `.npz` file
/// or other data sources.
///
/// # Fields
/// - `low_cut` (*f32*): The low-frequency cutoff value of the PSF filters.
/// - `high_cut` (*f32*): The high-frequency cutoff value of the PSF filters.
/// - `start_freq` (*f32*): The starting frequency of the PSF filters.
/// - `end_freq` (*f32*): The ending frequency of the PSF filters.
/// - `n_filters` (*i32*): The number of filters included in the PSF.
/// - `filters` (*Array2<f32>*): A 2D array containing the filter coefficients for the PSF
///    the first dimension represents the filter coefficients, and the second dimension represents the frequency index.
/// - `filt_freqs` (*Array2<f32>*): A 1D array of frequencies associated with the filters.
/// - `[x_0, w_x]` (*Array2<f32>*): A 2D array representing the PSF in the X-axis, typically used for spatial resolution analysis.
///    the first dimension represents the fit parameters, and the second dimension represents the frequency index.
///    The fit parameters are the center and width of the PSF (in this order).
/// - `[y_0, w_y]` (*Array2<f32>*): A 2D array representing the PSF in the Y-axis, typically used for spatial resolution analysis.
///    the first dimension represents the fit parameters, and the second dimension represents the frequency index.
///    The fit parameters are the center and width of the PSF (in this order).
///
///
/// # Typical Usage
///
/// This struct is often used to:
/// - Load PSFs from data files (e.g., `.npz`) and process their frequency or spatial characteristics.
/// - Perform computations like filtering, interpolation, or fitting operations for spectroscopic data analysis.
///
/// # Example
/// ```
/// use crate::PSF;
/// use ndarray::{Array1, Array2};
///
/// let psf = PSF {
///     low_cut: 0.15,
///     high_cut: 6.0,
///     start_freq: 0.2,
///     end_freq: 4.0,
///     n_filters: 100,
///     filters: Array2::zeros((100, 100)),
///     filt_freqs: Array2::linspace(0.2, 4.0, 100),
///     popt_x: Array2::zeros((2, 100)),
///     popt_y: Array2::zeros((2, 100)),
/// };
///
/// println!("PSF has {} filters and spans the frequency range {:.1} Hz to {:.1} Hz.",
///     psf.n_filters, psf.start_freq, psf.end_freq);
/// ```
#[derive(Serialize, Deserialize, Default, PartialEq, Debug, Clone)]
pub struct PSF {
    pub low_cut: f32,
    pub high_cut: f32,
    pub start_freq: f32,
    pub end_freq: f32,
    pub n_filters: i32,
    pub filters: Array2<f32>,
    pub filt_freqs: Array1<f32>,
    pub popt_x: Array2<f32>,
    pub popt_y: Array2<f32>,
}

/// Creates a 2D Point Spread Function (PSF) grid by interpolating and normalizing input PSF data.
///
/// # Arguments
/// - `psf_x` (*Vec<f32>*): The PSF values along the X-axis.
/// - `psf_y` (*Vec<f32>*): The PSF values along the Y-axis.
/// - `x` (*Vec<f32>*): The X-axis coordinates.
/// - `y` (*Vec<f32>*): The Y-axis coordinates.
/// - `dx` (*f32*): The step size for the X-axis.
/// - `dy` (*f32*): The step size for the Y-axis.
///
/// # Returns
/// - (*Array2<f32>*): A 2D array representing the interpolated PSF grid.
///
/// # Panics
/// - This function panics if the interpolation fails.
///
/// # Notes
/// - The function normalizes the input PSF values and pads them with zeros for interpolation.
/// - The resulting PSF grid is calculated using linear interpolation.
pub fn create_psf_2d(
    psf_x: Vec<f32>,
    psf_y: Vec<f32>,
    x: Vec<f32>,
    y: Vec<f32>,
    dx: f32,
    dy: f32,
) -> Array2<f32> {
    // Clone the input vectors to allow modifications
    let mut psf_x = psf_x;
    let mut psf_y = psf_y;
    let mut x = x;
    let mut y = y;

    // Normalize psf_x and psf_y
    // Normalize psf_x and psf_y by dividing each value by the maximum value in the respective vector
    let psf_x_max = psf_x.iter().cloned().fold(f32::MIN, f32::max);
    let psf_y_max = psf_y.iter().cloned().fold(f32::MIN, f32::max);
    psf_x.iter_mut().for_each(|v| *v /= psf_x_max);
    psf_y.iter_mut().for_each(|v| *v /= psf_y_max);

    // Determine the maximum values for x and y, rounded down to the nearest integer
    let x_max = x.iter().cloned().fold(f32::MIN, f32::max).floor() as usize;
    let y_max = y.iter().cloned().fold(f32::MIN, f32::max).floor() as usize;

    // Factor for padding
    // Define a padding factor and calculate the new maximum dimensions for x and y
    let factor = 2.0;
    let new_x_max = (factor * x_max as f32).ceil();
    let new_y_max = (factor * y_max as f32).ceil();

    // Calculate step sizes
    // Calculate the step sizes for x and y based on the last two elements of the respective vectors
    let x_step = x[x.len() - 1] - x[x.len() - 2];
    let y_step = y[y.len() - 1] - y[y.len() - 2];

    // Calculate the number of additional steps needed to pad x and y to the new maximum dimensions
    let n_new_steps_x = ((new_x_max - x[x.len() - 1]) / x_step).ceil() as usize;
    let n_new_steps_y = ((new_y_max - y[y.len() - 1]) / y_step).ceil() as usize;

    // Padding PSF with zeros for interpolation
    // Pad the x vector and psf_x with zeros on both ends to match the new dimensions
    for _ in 0..n_new_steps_x {
        x.push(x[x.len() - 1] + x_step);
        x.insert(0, x[0] - x_step);
        psf_x.push(0.0);
        psf_x.insert(0, 0.0);
    }

    // Pad the y vector and psf_y with zeros on both ends to match the new dimensions
    for _ in 0..n_new_steps_y {
        y.push(y[y.len() - 1] + y_step);
        y.insert(0, y[0] - y_step);
        psf_y.push(0.0);
        psf_y.insert(0, 0.0);
    }

    // Create the PSF grid
    // Generate the grid of x and y coordinates based on the padded dimensions and step sizes
    let xx: Vec<f32> = (-(x_max as f32) as i32..=x_max as f32 as i32)
        .map(|v| v as f32 * dx)
        .collect();

    let yy: Vec<f32> = (-(y_max as f32) as i32..=y_max as f32 as i32)
        .map(|v| v as f32 * dy)
        .collect();

    // Initialize a 2D array to store the interpolated PSF values
    let mut psf_2d = Array2::zeros((xx.len(), yy.len()));

    // Fill in the PSF 2D array using linear interpolation
    // Create interpolators for the x and y dimensions using the padded data
    let interp_x =
        Interp1d::new_unsorted(x.to_vec(), psf_x.to_vec()).expect("Failed to create interpolator");
    let interp_y =
        Interp1d::new_unsorted(y.to_vec(), psf_y.to_vec()).expect("Failed to create interpolator");
    // Populate the 2D PSF array by interpolating values for each grid point
    for (i, &x_val) in xx.iter().enumerate() {
        for (j, &y_val) in yy.iter().enumerate() {
            let psf_x_interp = interp_x.interpolate(x_val);
            let psf_y_interp = interp_y.interpolate(y_val);
            psf_2d[(i, j)] = psf_x_interp * psf_y_interp;
        }
    }
    psf_2d
}

/// Gaussian function with a different normalization
/// Computes a Gaussian function with a different normalization for the given input data and parameters.
///
/// # Arguments
/// - `x` (*&Array1<f32>*): The input data.
/// - `params` (*&[f32]*): The parameters of the Gaussian function:
///   - `params[0]` (*f32*): The center of the Gaussian.
///   - `params[1]` (*f32*): The width of the Gaussian.
///
/// # Returns
/// - (*Array1<f32>*): The computed Gaussian values for the input data.
pub fn gaussian(x: &Array1<f32>, params: &[f32]) -> Array1<f32> {
    let x0 = params[0];
    let w = params[1];
    x.mapv(|xi| {
        (2.0 / std::f32::consts::PI).sqrt() * (-2.0 * (xi - x0).powf(2.0) / (w * w)).exp() / w
    })
}
