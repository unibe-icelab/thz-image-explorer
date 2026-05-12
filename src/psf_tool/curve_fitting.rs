use serde::{Deserialize, Serialize};

/// Hybrid fit: physical model (a/f + b) + spline correction for optical defects
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HybridFit {
    pub a: f64,                  // 1/f coefficient
    pub b: f64,                  // constant offset
    pub correction: CubicSpline, // Spline for residuals
}

impl HybridFit {
    /// Fit hybrid model: base (a/f + b) + spline correction
    pub fn fit(frequencies: &[f64], values: &[f64]) -> Result<Self, String> {
        if frequencies.len() != values.len() {
            return Err("frequencies and values must have same length".to_string());
        }
        if frequencies.len() < 3 {
            return Err("Need at least 3 points for hybrid fit".to_string());
        }

        // Step 1: Fit base model a/f + b using least squares
        let n = frequencies.len();
        let mut sum_1_f = 0.0;
        let mut sum_1_f2 = 0.0;
        let mut sum_w = 0.0;
        let mut sum_w_f = 0.0;
        let mut sum_1 = 0.0;

        for i in 0..n {
            let f = frequencies[i];
            let w = values[i];
            let inv_f = 1.0 / f;

            sum_1_f += inv_f;
            sum_1_f2 += inv_f * inv_f;
            sum_w += w;
            sum_w_f += w * inv_f;
            sum_1 += 1.0;
        }

        // Solve 2x2 system for [a, b]
        // [sum(1/f²)  sum(1/f)] [a]   [sum(w/f)]
        // [sum(1/f)   n       ] [b] = [sum(w)  ]
        let det = sum_1_f2 * sum_1 - sum_1_f * sum_1_f;
        if det.abs() < 1e-10 {
            return Err("Singular matrix in base fit".to_string());
        }

        let a = (sum_w_f * sum_1 - sum_w * sum_1_f) / det;
        let b = (sum_1_f2 * sum_w - sum_1_f * sum_w_f) / det;

        // Step 2: Calculate residuals
        let mut residuals = Vec::with_capacity(n);
        for i in 0..n {
            let f = frequencies[i];
            let base = a / f + b;
            residuals.push(values[i] - base);
        }

        // Step 3: Fit spline to residuals
        let correction = CubicSpline::fit(frequencies, &residuals)?;

        Ok(Self { a, b, correction })
    }

    /// Evaluate correction with constrained extrapolation
    fn eval_correction(&self, f: f64) -> f64 {
        let n = self.correction.x.len();
        let f_min = self.correction.x[0];
        let f_max = self.correction.x[n - 1];

        // Inside data range: use spline as is
        if f >= f_min && f <= f_max {
            return self.correction.eval_single(f);
        }

        // Outside data range: extrapolate with slope constraint
        // The total derivative is: dw/df = -a/f² + correction'(f)
        // We need: -a/f² + correction'(f) <= 0
        // So: correction'(f) <= a/f²

        if f < f_min {
            let dx = f - f_min;
            let coeffs = &self.correction.coeffs[0];
            let y0 = coeffs[0];
            let slope = coeffs[1]; // First derivative at f_min
                                   // Maximum allowed slope to keep total derivative <= 0
            let max_slope = self.a / (f * f);
            let safe_slope = slope.min(max_slope);
            return y0 + safe_slope * dx;
        } else {
            // Right extrapolation
            let i = n - 2;
            let dx_end = self.correction.x[n - 1] - self.correction.x[i];
            let coeffs = &self.correction.coeffs[i];
            // Evaluate value and derivative at right endpoint
            let y_end = coeffs[0]
                + coeffs[1] * dx_end
                + coeffs[2] * dx_end * dx_end
                + coeffs[3] * dx_end * dx_end * dx_end;
            let slope_end =
                coeffs[1] + 2.0 * coeffs[2] * dx_end + 3.0 * coeffs[3] * dx_end * dx_end;
            // Maximum allowed slope to keep total derivative <= 0
            let max_slope = self.a / (f * f);
            let safe_slope = slope_end.min(max_slope);
            let dx = f - self.correction.x[n - 1];
            return y_end + safe_slope * dx;
        }
    }

    /// Evaluate hybrid model at given frequencies with monotonicity constraint
    pub fn evaluate(&self, frequencies: &[f64]) -> Vec<f64> {
        let mut results: Vec<f64> = frequencies
            .iter()
            .map(|&f| {
                let base = self.a / f + self.b;
                let corr = self.eval_correction(f);
                base + corr
            })
            .collect();

        // Enforce monotonic decrease: w(f1) >= w(f2) if f1 < f2
        // Clip values to maintain monotonicity from left to right
        for i in 1..results.len() {
            if results[i] > results[i - 1] {
                results[i] = results[i - 1];
            }
        }

        results
    }
}

/// Cubic spline interpolation for smooth curve fitting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CubicSpline {
    pub x: Vec<f64>,
    pub y: Vec<f64>,
    pub coeffs: Vec<[f64; 4]>, // [a, b, c, d] for each segment
}

impl CubicSpline {
    /// Fit a natural cubic spline through the data points
    pub fn fit(x: &[f64], y: &[f64]) -> Result<Self, String> {
        if x.len() != y.len() {
            return Err("x and y must have same length".to_string());
        }
        if x.len() < 2 {
            return Err("Need at least 2 points for spline".to_string());
        }

        let n = x.len();

        // Sort points by x
        let mut indices: Vec<usize> = (0..n).collect();
        indices.sort_by(|&i, &j| x[i].partial_cmp(&x[j]).unwrap());

        let x_sorted: Vec<f64> = indices.iter().map(|&i| x[i]).collect();
        let y_sorted: Vec<f64> = indices.iter().map(|&i| y[i]).collect();

        // Calculate intervals
        let mut h = vec![0.0; n - 1];
        for i in 0..n - 1 {
            h[i] = x_sorted[i + 1] - x_sorted[i];
            if h[i] <= 0.0 {
                return Err("x values must be strictly increasing".to_string());
            }
        }

        // Build tridiagonal system for natural spline (second derivative = 0 at endpoints)
        let mut a = vec![0.0; n];
        let mut b = vec![0.0; n];
        let mut c = vec![0.0; n];
        let mut d = vec![0.0; n];

        // Natural spline boundary conditions
        a[0] = 0.0;
        b[0] = 1.0;
        c[0] = 0.0;
        d[0] = 0.0;

        for i in 1..n - 1 {
            a[i] = h[i - 1];
            b[i] = 2.0 * (h[i - 1] + h[i]);
            c[i] = h[i];
            d[i] = 3.0
                * ((y_sorted[i + 1] - y_sorted[i]) / h[i]
                    - (y_sorted[i] - y_sorted[i - 1]) / h[i - 1]);
        }

        a[n - 1] = 0.0;
        b[n - 1] = 1.0;
        c[n - 1] = 0.0;
        d[n - 1] = 0.0;

        // Solve tridiagonal system for second derivatives
        let m = solve_tridiagonal(&a, &b, &c, &d)?;

        // Calculate spline coefficients for each segment
        let mut coeffs = Vec::new();
        for i in 0..n - 1 {
            let dx = h[i];
            let dy = y_sorted[i + 1] - y_sorted[i];

            // Coefficients for polynomial: S_i(x) = a + b*(x-x_i) + c*(x-x_i)^2 + d*(x-x_i)^3
            let a_coeff = y_sorted[i];
            let b_coeff = dy / dx - dx * (2.0 * m[i] + m[i + 1]) / 3.0;
            let c_coeff = m[i];
            let d_coeff = (m[i + 1] - m[i]) / (3.0 * dx);

            coeffs.push([a_coeff, b_coeff, c_coeff, d_coeff]);
        }

        Ok(Self {
            x: x_sorted,
            y: y_sorted,
            coeffs,
        })
    }

    /// Evaluate spline at given points
    #[allow(dead_code)]
    pub fn evaluate(&self, x_eval: &[f64]) -> Vec<f64> {
        x_eval.iter().map(|&x| self.eval_single(x)).collect()
    }

    /// Evaluate spline at given points with constant extrapolation
    pub fn evaluate_const_extrap(&self, x_eval: &[f64]) -> Vec<f64> {
        x_eval
            .iter()
            .map(|&x| self.eval_single_const_extrap(x))
            .collect()
    }

    /// Evaluate spline at a single point with constrained extrapolation
    fn eval_single(&self, x: f64) -> f64 {
        let n = self.x.len();

        // Handle extrapolation with linear continuation based on endpoint tangent
        if x < self.x[0] {
            // Left extrapolation: use linear extrapolation based on tangent at x[0]
            let dx = x - self.x[0];
            let coeffs = &self.coeffs[0];
            let y0 = coeffs[0];
            let slope = coeffs[1]; // First derivative at x[0]
            let y_extrap = y0 + slope * dx;
            // Ensure positive for beam width (w > 0) only in extrapolation
            return y_extrap.max(1e-6);
        }

        if x > self.x[n - 1] {
            // Right extrapolation: use linear extrapolation based on tangent at x[n-1]
            let i = n - 2;
            let dx_end = self.x[n - 1] - self.x[i];
            let coeffs = &self.coeffs[i];
            // Evaluate value and derivative at right endpoint
            let y_end = coeffs[0]
                + coeffs[1] * dx_end
                + coeffs[2] * dx_end * dx_end
                + coeffs[3] * dx_end * dx_end * dx_end;
            let slope_end =
                coeffs[1] + 2.0 * coeffs[2] * dx_end + 3.0 * coeffs[3] * dx_end * dx_end;
            let dx = x - self.x[n - 1];
            let y_extrap = y_end + slope_end * dx;
            // Ensure positive for beam width (w > 0) only in extrapolation
            return y_extrap.max(1e-6);
        }

        // Interpolation: binary search for the right segment
        let mut left = 0;
        let mut right = n - 1;
        while right - left > 1 {
            let mid = (left + right) / 2;
            if self.x[mid] > x {
                right = mid;
            } else {
                left = mid;
            }
        }

        // Evaluate polynomial (no clamping in interpolation region)
        let dx = x - self.x[left];
        let coeffs = &self.coeffs[left];
        coeffs[0] + coeffs[1] * dx + coeffs[2] * dx * dx + coeffs[3] * dx * dx * dx
    }

    /// Evaluate spline with constant extrapolation (for x0/y0 positions)
    fn eval_single_const_extrap(&self, x: f64) -> f64 {
        let n = self.x.len();

        // Constant extrapolation: hold boundary values
        if x < self.x[0] {
            return self.y[0];
        }

        if x > self.x[n - 1] {
            return self.y[n - 1];
        }

        // Interpolation: binary search for the right segment
        let mut left = 0;
        let mut right = n - 1;
        while right - left > 1 {
            let mid = (left + right) / 2;
            if self.x[mid] > x {
                right = mid;
            } else {
                left = mid;
            }
        }

        // Evaluate polynomial
        let dx = x - self.x[left];
        let coeffs = &self.coeffs[left];
        coeffs[0] + coeffs[1] * dx + coeffs[2] * dx * dx + coeffs[3] * dx * dx * dx
    }
}

/// Solve tridiagonal system Ax = d where A has diagonals a, b, c
fn solve_tridiagonal(a: &[f64], b: &[f64], c: &[f64], d: &[f64]) -> Result<Vec<f64>, String> {
    let n = b.len();
    let mut c_prime = vec![0.0; n];
    let mut d_prime = vec![0.0; n];
    let mut x = vec![0.0; n];

    // Forward sweep
    c_prime[0] = c[0] / b[0];
    d_prime[0] = d[0] / b[0];

    for i in 1..n {
        let denom = b[i] - a[i] * c_prime[i - 1];
        if denom.abs() < 1e-10 {
            return Err("Tridiagonal system is singular".to_string());
        }
        c_prime[i] = c[i] / denom;
        d_prime[i] = (d[i] - a[i] * d_prime[i - 1]) / denom;
    }

    // Back substitution
    x[n - 1] = d_prime[n - 1];
    for i in (0..n - 1).rev() {
        x[i] = d_prime[i] - c_prime[i] * x[i + 1];
    }

    Ok(x)
}

/// Fit results for beam width and center curves
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurveFits {
    pub wx_fit: HybridFit,
    pub wy_fit: HybridFit,
    pub x0_fit: CubicSpline,
    pub y0_fit: CubicSpline,
}

impl CurveFits {
    /// Generate interpolated frequency points for smooth curve display
    pub fn generate_interpolated_frequencies(frequencies: &[f64], num_points: usize) -> Vec<f64> {
        if frequencies.is_empty() {
            return Vec::new();
        }

        let min_freq = frequencies.iter().cloned().fold(f64::INFINITY, f64::min);
        let max_freq = frequencies
            .iter()
            .cloned()
            .fold(f64::NEG_INFINITY, f64::max);

        (0..num_points)
            .map(|i| {
                let t = i as f64 / (num_points - 1) as f64;
                min_freq + t * (max_freq - min_freq)
            })
            .collect()
    }
    /// Fit all curves from beam width data
    pub fn fit_from_data(
        frequencies: &[f64],
        wx: &[f64],
        wy: &[f64],
        x0: &[f64],
        y0: &[f64],
    ) -> Result<Self, String> {
        // Fit beam widths with hybrid model (a/f + b + spline correction)
        // This captures both the physical 1/f behavior and optical defects
        let wx_fit = HybridFit::fit(frequencies, wx)?;
        let wy_fit = HybridFit::fit(frequencies, wy)?;

        // Fit beam centers with cubic splines (no physical model)
        let x0_fit = CubicSpline::fit(frequencies, x0)?;
        let y0_fit = CubicSpline::fit(frequencies, y0)?;

        Ok(Self {
            wx_fit,
            wy_fit,
            x0_fit,
            y0_fit,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cubic_spline() {
        // Test with simple parabola y = x^2
        let x = vec![0.0, 1.0, 2.0, 3.0, 4.0];
        let y: Vec<f64> = x.iter().map(|&xi| xi * xi).collect();

        let spline = CubicSpline::fit(&x, &y).unwrap();

        // Test that spline passes through original points
        for i in 0..x.len() {
            let y_eval = spline.eval_single(x[i]);
            assert!(
                (y_eval - y[i]).abs() < 1e-10,
                "Spline should pass through point ({}, {}), got {}",
                x[i],
                y[i],
                y_eval
            );
        }

        // Test interpolation
        let x_test = 1.5;
        let y_expected = x_test * x_test;
        let y_eval = spline.eval_single(x_test);
        assert!(
            (y_eval - y_expected).abs() < 0.1,
            "Spline interpolation at {} should be close to {}, got {}",
            x_test,
            y_expected,
            y_eval
        );
    }

    #[test]
    fn test_cubic_spline_linear() {
        // Test with linear function y = 2x + 1
        let x = vec![0.0, 1.0, 2.0, 3.0];
        let y: Vec<f64> = x.iter().map(|&xi| 2.0 * xi + 1.0).collect();

        let spline = CubicSpline::fit(&x, &y).unwrap();

        // For linear data, spline should be exact everywhere
        let x_test = vec![0.5, 1.5, 2.5];
        for &xt in &x_test {
            let y_expected = 2.0 * xt + 1.0;
            let y_eval = spline.eval_single(xt);
            assert!(
                (y_eval - y_expected).abs() < 1e-10,
                "Spline for linear data at {} should be {}, got {}",
                xt,
                y_expected,
                y_eval
            );
        }
    }

    #[test]
    fn test_cubic_spline_extrapolation() {
        // Test extrapolation with decreasing function (like beam width)
        let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let y = vec![5.0, 3.5, 2.8, 2.3, 2.0]; // Decreasing

        let spline = CubicSpline::fit(&x, &y).unwrap();

        // Test left extrapolation (x < 1.0)
        let y_left = spline.eval_single(0.5);
        assert!(
            y_left > 0.0,
            "Extrapolated value should be positive, got {}",
            y_left
        );
        assert!(
            y_left > y[0],
            "Left extrapolation should be larger than first point for decreasing function"
        );

        // Test right extrapolation (x > 5.0)
        let y_right = spline.eval_single(6.0);
        assert!(
            y_right > 0.0,
            "Extrapolated value should be positive, got {}",
            y_right
        );
        // Right extrapolation should continue the trend
    }
}
