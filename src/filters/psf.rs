use interp1d::Interp1d;
use ndarray::{Array1, Array2};
use serde::{Deserialize, Serialize};

/// Cubic spline interpolation coefficients for a single curve
#[derive(Serialize, Deserialize, Default, PartialEq, Debug, Clone)]
pub struct CubicSplineCoeffs {
    pub knots: Array1<f32>,   // x values (knots)
    pub values: Array1<f32>,  // y values at knots
    pub coeff_a: Array1<f32>, // a coefficient for each segment
    pub coeff_b: Array1<f32>, // b coefficient for each segment
    pub coeff_c: Array1<f32>, // c coefficient for each segment
    pub coeff_d: Array1<f32>, // d coefficient for each segment
}

impl CubicSplineCoeffs {
    /// Evaluate spline at a single point with constrained extrapolation
    pub fn eval_single(&self, x: f32) -> f32 {
        let n = self.knots.len();

        if n == 0 {
            return 0.0;
        }

        // Handle extrapolation with linear continuation based on endpoint tangent
        if x < self.knots[0] {
            // Left extrapolation: use linear extrapolation based on tangent at knots[0]
            let dx = x - self.knots[0];
            let y0 = self.coeff_a[0];
            let slope = self.coeff_b[0]; // First derivative at knots[0]
            let y_extrap = y0 + slope * dx;
            // Ensure positive for beam width (w > 0) only in extrapolation
            return y_extrap.max(1e-6);
        }

        if x > self.knots[n - 1] {
            // Right extrapolation: use linear extrapolation based on tangent at knots[n-1]
            let i = n - 2;
            let dx_end = self.knots[n - 1] - self.knots[i];
            // Evaluate value and derivative at right endpoint
            let y_end = self.coeff_a[i]
                + self.coeff_b[i] * dx_end
                + self.coeff_c[i] * dx_end * dx_end
                + self.coeff_d[i] * dx_end * dx_end * dx_end;
            let slope_end = self.coeff_b[i]
                + 2.0 * self.coeff_c[i] * dx_end
                + 3.0 * self.coeff_d[i] * dx_end * dx_end;
            let dx = x - self.knots[n - 1];
            let y_extrap = y_end + slope_end * dx;
            // Ensure positive for beam width (w > 0) only in extrapolation
            return y_extrap.max(1e-6);
        }

        // Interpolation: binary search for the right segment
        let mut left = 0;
        let mut right = n - 1;
        while right - left > 1 {
            let mid = (left + right) / 2;
            if self.knots[mid] > x {
                right = mid;
            } else {
                left = mid;
            }
        }

        // Evaluate polynomial (no clamping in interpolation region)
        let dx = x - self.knots[left];
        self.coeff_a[left]
            + self.coeff_b[left] * dx
            + self.coeff_c[left] * dx * dx
            + self.coeff_d[left] * dx * dx * dx
    }
}

/// Represents a Point Spread Function (PSF) used in spectroscopy and imaging analysis.
///
/// A PSF characterizes the response of an imaging system to a point source signal, providing
/// critical information about the system resolution and frequency characteristics.
/// This structure holds cubic spline fit coefficients for beam widths and centers as functions
/// of frequency, replacing the previous filter bank approach.
///
/// # Fields
/// - `wx_spline` (*CubicSplineCoeffs*): Cubic spline coefficients for beam width in X direction
/// - `wy_spline` (*CubicSplineCoeffs*): Cubic spline coefficients for beam width in Y direction
/// - `x0_spline` (*CubicSplineCoeffs*): Cubic spline coefficients for beam center in X direction
/// - `y0_spline` (*CubicSplineCoeffs*): Cubic spline coefficients for beam center in Y direction
///
/// # Typical Usage
///
/// This struct is used to:
/// - Load PSF spline coefficients from data files (e.g., `.npz`)
/// - Evaluate beam widths and centers at arbitrary frequencies
/// - Generate PSFs for frequency-dependent deconvolution
#[derive(Serialize, Deserialize, Default, PartialEq, Debug, Clone)]
pub struct PSF {
    pub wx_spline: CubicSplineCoeffs,
    pub wy_spline: CubicSplineCoeffs,
    pub x0_spline: CubicSplineCoeffs,
    pub y0_spline: CubicSplineCoeffs,
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
