use ndarray::{s, Array1, Array2, Axis, Ix1, Zip};
use ndarray_stats::QuantileExt;
use serde::{Deserialize, Serialize};
use std::error::Error;

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
/// TODO: improve the names of the fields for the fit parameters.
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

/// Linear interpolation function for a 1D array.
fn linear_interp(x: &Vec<f32>, y: &Vec<f32>, xi: f32) -> f32 {
    let n = x.len();
    for i in 0..n - 1 {
        if xi >= x[i] && xi <= x[i + 1] {
            let slope = (y[i + 1] - y[i]) / (x[i + 1] - x[i]);
            return y[i] + slope * (xi - x[i]);
        }
    }
    0.0 // Return 0.0 if xi is out of bounds
}

/// TODO: this does not yet work!
pub fn create_psf_2d(
    psf_x: Vec<f32>,
    psf_y: Vec<f32>,
    x: Vec<f32>,
    y: Vec<f32>,
    dx: f32,
    dy: f32,
) -> Array2<f32> {
    let mut psf_x = psf_x;
    let mut psf_y = psf_y;
    let mut x = x;
    let mut y = y;

    let x_max = x.iter().cloned().fold(f32::MIN, f32::max).floor() as usize;
    let y_max = y.iter().cloned().fold(f32::MIN, f32::max).floor() as usize;

    // Factor for padding
    let factor = 2.0;
    let new_x_max = (factor * x_max as f32).ceil();
    let new_y_max = (factor * y_max as f32).ceil();

    // Calculate step sizes
    let x_step = x[x.len() - 1] - x[x.len() - 2];
    let y_step = y[y.len() - 1] - y[y.len() - 2];

    let n_new_steps_x = ((new_x_max - x[x.len() - 1]) / x_step).ceil() as usize;
    let n_new_steps_y = ((new_y_max - y[y.len() - 1]) / y_step).ceil() as usize;

    // Padding PSF with zeros for interpolation
    for _ in 0..n_new_steps_x {
        x.push(x[x.len() - 1] + x_step);
        x.insert(0, x[0] - x_step);
        psf_x.push(0.0);
        psf_x.insert(0, 0.0);
    }

    for _ in 0..n_new_steps_y {
        y.push(y[y.len() - 1] + y_step);
        y.insert(0, y[0] - y_step);
        psf_y.push(0.0);
        psf_y.insert(0, 0.0);
    }

    // Create the PSF grid
    let xx: Vec<f32> = (-(x_max as f32) as i32..=x_max as f32 as i32)
        .map(|v| v as f32 * dx)
        .collect();

    let yy: Vec<f32> = (-(y_max as f32) as i32..=y_max as f32 as i32)
        .map(|v| v as f32 * dy)
        .collect();

    let mut psf_2d = Array2::zeros((xx.len(), yy.len()));

    // Fill in the PSF 2D array using linear interpolation
    for (i, &x_val) in xx.iter().enumerate() {
        for (j, &y_val) in yy.iter().enumerate() {
            let psf_x_interp = linear_interp(&x, &psf_x, x_val);
            let psf_y_interp = linear_interp(&y, &psf_y, y_val);
            psf_2d[(i, j)] = psf_x_interp * psf_y_interp;
        }
    }
    psf_2d
}

/// Computes the sum of squared residuals
fn residual_sum_squares(
    x: &Array1<f64>,
    y: &Array1<f64>,
    params: &[f64],
    model: fn(&Array1<f64>, &[f64]) -> Array1<f64>,
) -> f64 {
    let y_model = model(x, params);
    Zip::from(y)
        .and(&y_model)
        .fold(0.0, |acc, &yi, &ymi| acc + (yi - ymi).powi(2))
}

/// Curve fitting implementation
fn curve_fit(
    func: fn(&Array1<f64>, &[f64]) -> Array1<f64>,
    x_data: &Array1<f64>,
    y_data: &Array1<f64>,
    initial_params: &[f64],
) -> Result<Vec<f64>, Box<dyn Error>> {
    let mut params = initial_params.to_vec();
    let learning_rate = 0.01;
    let max_iters = 1000;

    for _ in 0..max_iters {
        let gradient = compute_gradient(x_data, y_data, &params, func);
        for (p, g) in params.iter_mut().zip(gradient) {
            *p -= learning_rate * g;
        }
    }

    Ok(params)
}

/// Computes the gradient of the cost function
fn compute_gradient(
    x: &Array1<f64>,
    y: &Array1<f64>,
    params: &[f64],
    model: fn(&Array1<f64>, &[f64]) -> Array1<f64>,
) -> Vec<f64> {
    let delta = 1e-6;
    let mut gradient = vec![0.0; params.len()];
    for i in 0..params.len() {
        let mut params_plus = params.to_vec();
        let mut params_minus = params.to_vec();
        params_plus[i] += delta;
        params_minus[i] -= delta;

        let rss_plus = residual_sum_squares(x, y, &params_plus, model);
        let rss_minus = residual_sum_squares(x, y, &params_minus, model);
        gradient[i] = (rss_plus - rss_minus) / (2.0 * delta);
    }
    gradient
}

/// Gaussian function
fn gaussian(x: &Array1<f64>, params: &[f64]) -> Array1<f64> {
    let x0 = params[0];
    let w = params[1];
    x.mapv(|xi| {
        (2.0 * (-2.0 * (xi - x0).powf(2.0) / (w * w)) / (2.0 * std::f64::consts::PI).sqrt() * w)
            .exp()
    })
}
/// Gaussian function with a different normalization
pub fn gaussian2(x: &Array1<f32>, params: &[f32]) -> Array1<f32> {
    let x0 = params[0];
    let w = params[1];
    x.mapv(|xi| {
        (2.0 / std::f32::consts::PI).sqrt() * (-2.0 * (xi - x0).powf(2.0) / (w * w)).exp() / w
    })
}

/// Error function
fn error_f(x: &Array1<f64>, params: &[f64]) -> Array1<f64> {
    let x0 = params[0];
    let w = params[1];
    x.mapv(|xi| {
        (1.0 + statrs::function::erf::erf(2.0_f64.sqrt() * (xi - x0) / w)) / 2.0
    })
}

/// TODO: this does not yet work
pub fn get_center(
    x_axis_psf: &Array1<f64>,
    y_axis_psf: &Array1<f64>,
    np_psf_t_x: &Array2<f64>,
    np_psf_t_y: &Array2<f64>,
    n_min: usize,
    n_max: usize,
) -> Result<
    (
        f64,
        f64,
        ndarray::Array<f64, Ix1>,
        ndarray::Array<f64, Ix1>,
        ndarray::Array<f64, Ix1>,
        ndarray::Array<f64, Ix1>,
    ),
    Box<dyn Error>,
> {
    println!("Extracting the center of the PSF...");
    todo!();

    // Cropping the PSF to improve the fit
    let x_axis_psf_2 = x_axis_psf.slice(s![n_min..n_max]).to_owned();
    let y_axis_psf_2 = y_axis_psf.slice(s![n_min..n_max]).to_owned();
    let np_psf_t_x_2 = np_psf_t_x.slice(s![n_min..n_max, ..]).to_owned();
    let np_psf_t_y_2 = np_psf_t_y.slice(s![n_min..n_max, ..]).to_owned();

    // Calculate intensities
    let mut intensity_x = np_psf_t_x.map_axis(Axis(1), |row| row.mapv(|v| v.powi(2)).sum());
    let mut intensity_y = np_psf_t_y.map_axis(Axis(1), |row| row.mapv(|v| v.powi(2)).sum());

    intensity_x -= *intensity_x.min()?;
    intensity_x /= *intensity_x.max()?;
    intensity_y -= *intensity_y.min()?;
    intensity_y /= *intensity_y.max()?;

    let mut intensity_x_2 = np_psf_t_x_2.map_axis(Axis(1), |row| row.mapv(|v| v.powi(2)).sum());
    let mut intensity_y_2 = np_psf_t_y_2.map_axis(Axis(1), |row| row.mapv(|v| v.powi(2)).sum());

    intensity_x_2 -= *intensity_x_2.min()?;
    intensity_x_2 /= *intensity_x_2.max()?;
    intensity_y_2 -= *intensity_y_2.min()?;
    intensity_y_2 /= *intensity_y_2.max()?;

    // Initial parameters for the curve fit
    let initial_params = vec![0.0, 10.0];

    // Perform curve fitting
    let popt_x = curve_fit(error_f, &x_axis_psf_2, &intensity_x_2, &initial_params)?;
    let popt_y = curve_fit(error_f, &y_axis_psf_2, &intensity_y_2, &initial_params)?;

    dbg!(&popt_x, &popt_y);

    // Find the peak of the Gaussian
    let gaussian_values_x = gaussian(x_axis_psf, &popt_x);
    let max_x = gaussian_values_x
        .iter()
        .enumerate()
        .max_by(|(_, v1), (_, v2)| v1.partial_cmp(v2).unwrap())
        .map(|(index, _)| index)
        .unwrap();

    let gaussian_values_y = gaussian(y_axis_psf, &popt_y);
    let max_y = gaussian_values_y
        .iter()
        .enumerate()
        .max_by(|(_, v1), (_, v2)| v1.partial_cmp(v2).unwrap())
        .map(|(index, _)| index)
        .unwrap();

    let x0 = x_axis_psf[max_x];
    let y0 = y_axis_psf[max_y];
    println!("Center of the PSF: ({}, {})", x0, y0);

    Ok((
        x0,
        y0,
        x_axis_psf_2,
        intensity_x_2,
        y_axis_psf_2,
        intensity_y_2,
    ))
}
