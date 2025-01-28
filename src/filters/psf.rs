use ndarray::Array2;
use std::f64;

/// Linear interpolation function for a 1D array.
fn linear_interp(x: &Vec<f64>, y: &Vec<f64>, xi: f64) -> f64 {
    let n = x.len();
    for i in 0..n - 1 {
        if xi >= x[i] && xi <= x[i + 1] {
            let slope = (y[i + 1] - y[i]) / (x[i + 1] - x[i]);
            return y[i] + slope * (xi - x[i]);
        }
    }
    0.0 // Return 0.0 if xi is out of bounds
}

fn create_psf_2d(
    mut psf_x: Vec<f64>,
    mut psf_y: Vec<f64>,
    mut x: Vec<f64>,
    mut y: Vec<f64>,
    dx: f64,
    dy: f64,
) -> (Array2<f64>, Array2<f64>, Array2<f64>) {
    let x_max = x.iter().cloned().fold(f64::MIN, f64::max).floor() as usize;
    let y_max = y.iter().cloned().fold(f64::MIN, f64::max).floor() as usize;

    // Factor for padding
    let factor = 2.0;
    let new_x_max = (factor * x_max as f64).ceil();
    let new_y_max = (factor * y_max as f64).ceil();

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
    let xx: Vec<f64> = (-(x_max as i32)..=x_max as i32)
        .step_by(dx as usize)
        .map(|v| v as f64)
        .collect();

    let yy: Vec<f64> = (-(y_max as i32)..=y_max as i32)
        .step_by(dy as usize)
        .map(|v| v as f64)
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

    let xx_grid = Array2::from_shape_fn((xx.len(), 1), |(i, _)| xx[i]);
    let yy_grid = Array2::from_shape_fn((yy.len(), 1), |(i, _)| yy[i]);

    (xx_grid, yy_grid, psf_2d)
}
