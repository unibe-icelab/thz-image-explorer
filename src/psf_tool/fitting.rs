use ndarray::{Array1, Array2};
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

/// Error function approximation
pub fn erf(x: f64) -> f64 {
    // Abramowitz and Stegun approximation
    let a1 = 0.254829592;
    let a2 = -0.284496736;
    let a3 = 1.421413741;
    let a4 = -1.453152027;
    let a5 = 1.061405429;
    let p = 0.3275911;

    let sign = x.signum();
    let x = x.abs();

    let t = 1.0 / (1.0 + p * x);
    let y = 1.0 - (((((a5 * t + a4) * t) + a3) * t + a2) * t + a1) * t * (-x * x).exp();

    sign * y
}

/// Error function for fitting (cumulative Gaussian)
pub fn error_function(x: f64, x0: f64, w: f64) -> f64 {
    (1.0 + erf(std::f64::consts::SQRT_2 * (x - x0) / w)) / 2.0
}

/// Gaussian function
#[allow(dead_code)]
pub fn gaussian(x: f64, x0: f64, w: f64) -> f64 {
    (2.0 / PI).sqrt() * (-2.0 * (x - x0).powi(2) / w.powi(2)).exp() / w
}

/// Beam width as a function of frequency: w(f) = a/f
#[allow(dead_code)]
pub fn beam_width_function(freq: f64, a: f64) -> f64 {
    a / freq
}

/// Parameters for beam fitting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeamFitParams {
    pub w_max: f64,
    #[serde(default = "default_use_monotonicity")]
    pub use_monotonicity_constraint: bool,
}

fn default_use_monotonicity() -> bool {
    true
}

impl Default for BeamFitParams {
    fn default() -> Self {
        Self {
            w_max: 30.0,
            use_monotonicity_constraint: true,
        }
    }
}

/// Result of fitting mean beam
#[derive(Debug, Clone)]
pub struct MeanBeamFit {
    pub x0: f64,
    #[allow(dead_code)]
    pub y0: f64,
    pub popt_x: [f64; 2], // [x0, w_x]
    pub popt_y: [f64; 2], // [y0, w_y]
}

/// Result of fitting beam widths for multiple frequencies
#[derive(Debug, Clone)]
pub struct BeamWidthFits {
    pub popt_xs: Array2<f64>, // [n_filters, 2] - [x0, w_x] for each frequency
    pub popt_ys: Array2<f64>, // [n_filters, 2] - [y0, w_y] for each frequency
    pub filtered_traces_x: Vec<Array2<f64>>, // Filtered traces for each filter (X)
    pub filtered_traces_y: Vec<Array2<f64>>, // Filtered traces for each filter (Y)
    pub x_positions: Vec<f64>, // X positions
    pub y_positions: Vec<f64>, // Y positions
    // Optional: store left/right fits separately for visualization
    pub popt_xs_left: Option<Array2<f64>>,
    pub popt_xs_right: Option<Array2<f64>>,
    pub popt_ys_left: Option<Array2<f64>>,
    pub popt_ys_right: Option<Array2<f64>>,
    pub filtered_traces_x_left: Option<Vec<Array2<f64>>>,
    pub filtered_traces_x_right: Option<Vec<Array2<f64>>>,
    pub filtered_traces_y_left: Option<Vec<Array2<f64>>>,
    pub filtered_traces_y_right: Option<Vec<Array2<f64>>>,
    pub x_positions_left: Option<Vec<f64>>,
    pub x_positions_right: Option<Vec<f64>>,
    pub y_positions_left: Option<Vec<f64>>,
    pub y_positions_right: Option<Vec<f64>>,
}

/// Fit error function to data using Levenberg-Marquardt
fn fit_error_function(
    x_data: &[f64],
    y_data: &[f64],
    initial_guess: [f64; 2],
    bounds: Option<([f64; 2], [f64; 2])>,
) -> Result<[f64; 2], String> {
    use argmin::core::{CostFunction, Executor};
    use argmin::solver::neldermead::NelderMead;

    struct ErrorFunctionFit {
        x_data: Vec<f64>,
        y_data: Vec<f64>,
        bounds: Option<([f64; 2], [f64; 2])>,
    }

    impl CostFunction for ErrorFunctionFit {
        type Param = Vec<f64>;
        type Output = f64;

        fn cost(&self, p: &Self::Param) -> Result<Self::Output, argmin::core::Error> {
            let x0 = p[0];
            let w = p[1];

            // Apply bounds as penalty
            if let Some((lower, upper)) = &self.bounds {
                if x0 < lower[0] || x0 > upper[0] || w < lower[1] || w > upper[1] {
                    return Ok(1e10); // Large penalty for out-of-bounds
                }
            }

            let mut sum = 0.0;
            for (x, y) in self.x_data.iter().zip(self.y_data.iter()) {
                let pred = error_function(*x, x0, w);
                sum += (y - pred).powi(2);
            }

            Ok(sum)
        }
    }

    let cost = ErrorFunctionFit {
        x_data: x_data.to_vec(),
        y_data: y_data.to_vec(),
        bounds,
    };

    let solver = NelderMead::new(vec![
        vec![initial_guess[0], initial_guess[1]],
        vec![initial_guess[0] + 0.1, initial_guess[1]],
        vec![initial_guess[0], initial_guess[1] + 0.1],
    ]);

    let res = Executor::new(cost, solver)
        .configure(|state| state.max_iters(8000))
        .run()
        .map_err(|e| format!("Optimization failed: {}", e))?;

    let best_param = res.state().best_param.as_ref().unwrap();
    Ok([best_param[0], best_param[1]])
}

/// Compute intensity from time traces (sum of squares)
pub fn compute_intensity(traces: &Array2<f64>) -> Array1<f64> {
    let n_positions = traces.nrows();
    let mut intensity = Array1::zeros(n_positions);

    for i in 0..n_positions {
        let row = traces.row(i);
        intensity[i] = row.iter().map(|&x| x.powi(2)).sum();
    }

    // Normalize
    let min_val = intensity.iter().cloned().fold(f64::INFINITY, f64::min);
    let max_val = intensity.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

    if (max_val - min_val).abs() > 1e-10 {
        intensity = (intensity - min_val) / (max_val - min_val);
    }

    intensity
}

/// Fit the mean beam to determine center position and initial width
pub fn fit_mean_beam(
    x_positions: &[f64],
    y_positions: &[f64],
    x_traces: &Array2<f64>,
    y_traces: &Array2<f64>,
) -> Result<MeanBeamFit, String> {
    println!(
        "[DEBUG] fit_mean_beam: Starting with {} x positions, {} y positions",
        x_positions.len(),
        y_positions.len()
    );
    println!(
        "[DEBUG] fit_mean_beam: x_traces shape: {:?}, y_traces shape: {:?}",
        x_traces.dim(),
        y_traces.dim()
    );

    // Compute intensities
    let intensity_x = compute_intensity(x_traces);
    let intensity_y = compute_intensity(y_traces);

    println!(
        "[DEBUG] fit_mean_beam: intensity_x range: [{:.6}, {:.6}]",
        intensity_x.iter().cloned().fold(f64::INFINITY, f64::min),
        intensity_x
            .iter()
            .cloned()
            .fold(f64::NEG_INFINITY, f64::max)
    );
    println!(
        "[DEBUG] fit_mean_beam: intensity_y range: [{:.6}, {:.6}]",
        intensity_y.iter().cloned().fold(f64::INFINITY, f64::min),
        intensity_y
            .iter()
            .cloned()
            .fold(f64::NEG_INFINITY, f64::max)
    );

    // Initial guess
    let initial_guess = [0.0, 10.0];

    // Fit X
    println!(
        "[DEBUG] fit_mean_beam: Fitting X with initial guess: {:?}",
        initial_guess
    );
    let popt_x = fit_error_function(
        x_positions,
        intensity_x.as_slice().unwrap(),
        initial_guess,
        None,
    )?;
    println!(
        "[DEBUG] fit_mean_beam: X fit result: x0={:.3}, w={:.3}",
        popt_x[0], popt_x[1]
    );

    // Fit Y
    println!(
        "[DEBUG] fit_mean_beam: Fitting Y with initial guess: {:?}",
        initial_guess
    );
    let popt_y = fit_error_function(
        y_positions,
        intensity_y.as_slice().unwrap(),
        initial_guess,
        None,
    )?;
    println!(
        "[DEBUG] fit_mean_beam: Y fit result: y0={:.3}, w={:.3}",
        popt_y[0], popt_y[1]
    );

    // Find centers (peak of Gaussian derivative)
    let x0 = popt_x[0];
    let y0 = popt_y[0];

    Ok(MeanBeamFit {
        x0,
        y0,
        popt_x,
        popt_y,
    })
}

/// Convolve signal with filter (simple implementation matching scipy mode='same')
pub fn convolve(signal: &[f64], filter: &[f64]) -> Vec<f64> {
    let n_signal = signal.len();
    let n_filter = filter.len();
    let mut result = vec![0.0; n_signal];
    let mid = n_filter / 2;

    for i in 0..n_signal {
        let mut sum = 0.0;
        for j in 0..n_filter {
            let signal_idx = i as i32 + j as i32 - mid as i32;
            if signal_idx >= 0 && signal_idx < n_signal as i32 {
                sum += signal[signal_idx as usize] * filter[j];
            }
        }
        result[i] = sum;
    }

    result
}

/// Fit beam widths for each filter frequency
pub fn fit_beam_widths<F>(
    mean_fit: &MeanBeamFit,
    x_positions: &[f64],
    y_positions: &[f64],
    x_traces: &Array2<f64>,
    y_traces: &Array2<f64>,
    filters: &Array2<f64>,
    fit_params: &BeamFitParams,
    mut progress_callback: F,
) -> Result<BeamWidthFits, String>
where
    F: FnMut(usize, usize) -> bool, // ← bool au lieu de ()
{
    let n_filters = filters.nrows();
    println!(
        "[DEBUG] fit_beam_widths: Starting with {} filters",
        n_filters
    );
    println!(
        "[DEBUG] fit_beam_widths: mean_fit popt_x={:?}, popt_y={:?}",
        mean_fit.popt_x, mean_fit.popt_y
    );

    let mut popt_xs = Array2::zeros((n_filters, 2));
    let mut popt_ys = Array2::zeros((n_filters, 2));

    // Store filtered traces for SNR analysis
    let mut filtered_traces_x_vec = Vec::with_capacity(n_filters);
    let mut filtered_traces_y_vec = Vec::with_capacity(n_filters);

    // Initial guesses from mean fit
    let mut popt_x = [mean_fit.popt_x[0], fit_params.w_max];
    let mut popt_y = [mean_fit.popt_y[0], fit_params.w_max];

    // Initial bounds
    let w_max = fit_params.w_max;
    let range_max = w_max * 1.5;
    let mut bounds_x = ([-range_max / 2.0, 0.01], [range_max / 2.0, w_max]);
    let mut bounds_y = ([-range_max / 2.0, 0.01], [range_max / 2.0, w_max]);

    for nf in 0..n_filters {
        println!(
            "[DEBUG] fit_beam_widths: Processing filter {}/{}",
            nf + 1,
            n_filters
        );
        let filter_coeffs = filters.row(nf).to_vec();

        // Filter X traces
        let mut filtered_x_traces = Array2::zeros(x_traces.dim());
        for i in 0..x_traces.nrows() {
            let signal = x_traces.row(i).to_vec();
            let filtered = convolve(&signal, &filter_coeffs);
            for (j, &val) in filtered.iter().enumerate() {
                filtered_x_traces[[i, j]] = val;
            }
        }

        // Filter Y traces
        let mut filtered_y_traces = Array2::zeros(y_traces.dim());
        for i in 0..y_traces.nrows() {
            let signal = y_traces.row(i).to_vec();
            let filtered = convolve(&signal, &filter_coeffs);
            for (j, &val) in filtered.iter().enumerate() {
                filtered_y_traces[[i, j]] = val;
            }
        }

        // Compute intensities
        let intensity_x = compute_intensity(&filtered_x_traces);
        let intensity_y = compute_intensity(&filtered_y_traces);

        // Store filtered traces for SNR analysis
        filtered_traces_x_vec.push(filtered_x_traces.clone());
        filtered_traces_y_vec.push(filtered_y_traces.clone());

        // Fit X with bounds
        popt_x = fit_error_function(
            x_positions,
            intensity_x.as_slice().unwrap(),
            popt_x,
            Some(bounds_x),
        )?;

        // Update bounds based on fit result (like Python)
        let x_offset = popt_x[0];
        let w_x = popt_x[1];
        if fit_params.use_monotonicity_constraint {
            bounds_x = ([-w_x / 2.0 + x_offset, 0.0], [w_x / 2.0 + x_offset, w_x]);
        } else {
            // Without constraint, use fixed bounds based on w_max
            bounds_x = ([-range_max / 2.0, 0.01], [range_max / 2.0, w_max]);
        }

        // Fit Y with bounds
        popt_y = fit_error_function(
            y_positions,
            intensity_y.as_slice().unwrap(),
            popt_y,
            Some(bounds_y),
        )?;

        // Update bounds based on fit result (like Python)
        let y_offset = popt_y[0];
        let w_y = popt_y[1];
        if fit_params.use_monotonicity_constraint {
            bounds_y = ([-w_y / 2.0 + y_offset, 0.0], [w_y / 2.0 + y_offset, w_y]);
        } else {
            // Without constraint, use fixed bounds based on w_max
            bounds_y = ([-range_max / 2.0, 0.01], [range_max / 2.0, w_max]);
        }

        // Store results
        popt_xs[[nf, 0]] = popt_x[0];
        popt_xs[[nf, 1]] = popt_x[1].abs(); // Take absolute value like Python
        popt_ys[[nf, 0]] = popt_y[0];
        popt_ys[[nf, 1]] = popt_y[1].abs(); // Take absolute value like Python
        println!(
            "[DEBUG] fit_beam_widths: Filter {} results: x0={:.3}, wx={:.3}, y0={:.3}, wy={:.3}",
            nf,
            popt_x[0],
            popt_x[1].abs(),
            popt_y[0],
            popt_y[1].abs()
        );

        if !progress_callback(nf + 1, n_filters) {
            return Err("Cancelled".to_string());
        }
    }

    println!(
        "[DEBUG] fit_beam_widths: Completed all {} filters",
        n_filters
    );
    Ok(BeamWidthFits {
        popt_xs,
        popt_ys,
        filtered_traces_x: filtered_traces_x_vec,
        filtered_traces_y: filtered_traces_y_vec,
        x_positions: x_positions.to_vec(),
        y_positions: y_positions.to_vec(),
        popt_xs_left: None,
        popt_xs_right: None,
        popt_ys_left: None,
        popt_ys_right: None,
        filtered_traces_x_left: None,
        filtered_traces_x_right: None,
        filtered_traces_y_left: None,
        filtered_traces_y_right: None,
        x_positions_left: None,
        x_positions_right: None,
        y_positions_left: None,
        y_positions_right: None,
    })
}
