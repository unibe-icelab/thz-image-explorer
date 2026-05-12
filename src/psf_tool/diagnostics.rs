use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

// Physical constants
const C_LIGHT: f64 = 299_792_458.0; // m/s
const FOCAL_LENGTH_MM: f64 = 152.4; // 6 inches = 152.4 mm (measured at 1 THz)
const FREQ_REF_HZ: f64 = 1.0e12; // Reference frequency for focal length (1 THz)

/// PSF Diagnostic results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticResults {
    // Input data
    pub frequencies_thz: Vec<f64>,
    pub wavelengths_um: Vec<f64>,
    pub w0x_mm: Vec<f64>,
    pub w0y_mm: Vec<f64>,

    // Reference point (at 1 THz)
    pub freq_ref_thz: f64,
    pub lambda_ref_um: f64,
    pub w0x_ref_mm: f64,
    pub w0y_ref_mm: f64,

    // Ratio π·w0/λ analysis
    pub ratio_x: Vec<f64>,
    pub ratio_y: Vec<f64>,
    pub ratio_x_mean: f64,
    pub ratio_x_std: f64,
    pub ratio_y_mean: f64,
    pub ratio_y_std: f64,
    // Filtered means for frequencies < 1 THz
    pub ratio_x_mean_filtered: f64,
    pub ratio_x_std_filtered: f64,
    pub ratio_y_mean_filtered: f64,
    pub ratio_y_std_filtered: f64,

    // Effective aperture D_eff
    pub d_eff_x_mm: Vec<f64>,
    pub d_eff_y_mm: Vec<f64>,
    pub d_eff_x_mean_mm: f64,
    pub d_eff_x_std_mm: f64,
    pub d_eff_y_mean_mm: f64,
    pub d_eff_y_std_mm: f64,
    pub d_eff_x_theory_mm: f64,
    pub d_eff_y_theory_mm: f64,
    // Filtered means for frequencies < 1 THz
    pub d_eff_x_mean_filtered_mm: f64,
    pub d_eff_x_std_filtered_mm: f64,
    pub d_eff_y_mean_filtered_mm: f64,
    pub d_eff_y_std_filtered_mm: f64,

    // Linear fit w0 = A·λ
    pub a_x: f64, // proportionality constant X
    pub a_y: f64, // proportionality constant Y
    pub w0_fit_x_mm: Vec<f64>,
    pub w0_fit_y_mm: Vec<f64>,
    pub rmse_x_mm: f64,
    pub rmse_y_mm: f64,

    // Theoretical model (constant D_eff)
    pub w0_theory_x_mm: Vec<f64>,
    pub w0_theory_y_mm: Vec<f64>,
    pub rmse_theory_x_mm: f64,
    pub rmse_theory_y_mm: f64,

    // Rayleigh range z_R = π·w0²/λ
    pub z_r_x_mm: Vec<f64>,
    pub z_r_y_mm: Vec<f64>,
    pub z_r_fit_x_mm: Vec<f64>, // From linear fit
    pub z_r_fit_y_mm: Vec<f64>,
    pub z_r_theory_x_mm: Vec<f64>, // From constant D_eff
    pub z_r_theory_y_mm: Vec<f64>,

    // Diagnostic flags
    pub is_diffraction_limited: bool,
    pub cv_x_percent: f64, // Coefficient of variation for D_eff_x
    pub cv_y_percent: f64, // Coefficient of variation for D_eff_y
}

impl DiagnosticResults {
    /// Compute diagnostics from PSF fit results
    pub fn compute(
        frequencies_thz: &[f64],
        w0x_mm: &[f64],
        w0y_mm: &[f64],
    ) -> anyhow::Result<Self> {
        if frequencies_thz.len() != w0x_mm.len() || frequencies_thz.len() != w0y_mm.len() {
            return Err(anyhow::anyhow!("Input arrays must have the same length"));
        }

        if frequencies_thz.is_empty() {
            return Err(anyhow::anyhow!("Input arrays cannot be empty"));
        }

        let n = frequencies_thz.len();

        // Convert to SI units
        let freq_hz: Vec<f64> = frequencies_thz.iter().map(|f| f * 1e12).collect();
        let wavelength_m: Vec<f64> = freq_hz.iter().map(|f| C_LIGHT / f).collect();
        let wavelength_um: Vec<f64> = wavelength_m.iter().map(|w| w * 1e6).collect();
        let w0x_m: Vec<f64> = w0x_mm.iter().map(|w| w * 1e-3).collect();
        let w0y_m: Vec<f64> = w0y_mm.iter().map(|w| w * 1e-3).collect();
        let f_m = FOCAL_LENGTH_MM * 1e-3;

        // Find reference frequency closest to 1 THz
        let idx_ref = freq_hz
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                ((**a - FREQ_REF_HZ).abs())
                    .partial_cmp(&((**b - FREQ_REF_HZ).abs()))
                    .unwrap()
            })
            .map(|(i, _)| i)
            .unwrap();

        let freq_ref_actual = freq_hz[idx_ref];
        let lambda_ref_m = wavelength_m[idx_ref];
        let w0x_ref_m = w0x_m[idx_ref];
        let w0y_ref_m = w0y_m[idx_ref];

        // === 1. Calculate ratio π·w0/λ ===
        let ratio_x: Vec<f64> = w0x_m
            .iter()
            .zip(wavelength_m.iter())
            .map(|(w0, lambda)| PI * w0 / lambda)
            .collect();
        let ratio_y: Vec<f64> = w0y_m
            .iter()
            .zip(wavelength_m.iter())
            .map(|(w0, lambda)| PI * w0 / lambda)
            .collect();

        let ratio_x_mean = ratio_x.iter().sum::<f64>() / n as f64;
        let ratio_y_mean = ratio_y.iter().sum::<f64>() / n as f64;
        let ratio_x_std = (ratio_x
            .iter()
            .map(|r| (r - ratio_x_mean).powi(2))
            .sum::<f64>()
            / n as f64)
            .sqrt();
        let ratio_y_std = (ratio_y
            .iter()
            .map(|r| (r - ratio_y_mean).powi(2))
            .sum::<f64>()
            / n as f64)
            .sqrt();

        // === Filtered calculations for frequencies < 1 THz ===
        let filtered_indices: Vec<usize> = frequencies_thz
            .iter()
            .enumerate()
            .filter_map(|(i, &f)| if f < 1.0 { Some(i) } else { None })
            .collect();

        let (ratio_x_mean_filtered, ratio_x_std_filtered) = if !filtered_indices.is_empty() {
            let filtered_ratio_x: Vec<f64> = filtered_indices.iter().map(|&i| ratio_x[i]).collect();
            let mean = filtered_ratio_x.iter().sum::<f64>() / filtered_ratio_x.len() as f64;
            let std = (filtered_ratio_x
                .iter()
                .map(|r| (r - mean).powi(2))
                .sum::<f64>()
                / filtered_ratio_x.len() as f64)
                .sqrt();
            (mean, std)
        } else {
            (ratio_x_mean, ratio_x_std)
        };

        let (ratio_y_mean_filtered, ratio_y_std_filtered) = if !filtered_indices.is_empty() {
            let filtered_ratio_y: Vec<f64> = filtered_indices.iter().map(|&i| ratio_y[i]).collect();
            let mean = filtered_ratio_y.iter().sum::<f64>() / filtered_ratio_y.len() as f64;
            let std = (filtered_ratio_y
                .iter()
                .map(|r| (r - mean).powi(2))
                .sum::<f64>()
                / filtered_ratio_y.len() as f64)
                .sqrt();
            (mean, std)
        } else {
            (ratio_y_mean, ratio_y_std)
        };
        let d_eff_x_m: Vec<f64> = ratio_x.iter().map(|r| f_m / r).collect();
        let d_eff_y_m: Vec<f64> = ratio_y.iter().map(|r| f_m / r).collect();

        let d_eff_x_mm: Vec<f64> = d_eff_x_m.iter().map(|d| d * 1e3).collect();
        let d_eff_y_mm: Vec<f64> = d_eff_y_m.iter().map(|d| d * 1e3).collect();

        let d_eff_x_mean_m = d_eff_x_m.iter().sum::<f64>() / n as f64;
        let d_eff_y_mean_m = d_eff_y_m.iter().sum::<f64>() / n as f64;
        let d_eff_x_std_m = (d_eff_x_m
            .iter()
            .map(|d| (d - d_eff_x_mean_m).powi(2))
            .sum::<f64>()
            / n as f64)
            .sqrt();
        let d_eff_y_std_m = (d_eff_y_m
            .iter()
            .map(|d| (d - d_eff_y_mean_m).powi(2))
            .sum::<f64>()
            / n as f64)
            .sqrt();

        let d_eff_x_mean_mm = d_eff_x_mean_m * 1e3;
        let d_eff_x_std_mm = d_eff_x_std_m * 1e3;
        let d_eff_y_mean_mm = d_eff_y_mean_m * 1e3;
        let d_eff_y_std_mm = d_eff_y_std_m * 1e3;

        // Calculate filtered D_eff means for frequencies < 1 THz
        let (d_eff_x_mean_filtered_mm, d_eff_x_std_filtered_mm) = if !filtered_indices.is_empty() {
            let filtered_d_eff_x: Vec<f64> =
                filtered_indices.iter().map(|&i| d_eff_x_mm[i]).collect();
            let mean = filtered_d_eff_x.iter().sum::<f64>() / filtered_d_eff_x.len() as f64;
            let std = (filtered_d_eff_x
                .iter()
                .map(|d| (d - mean).powi(2))
                .sum::<f64>()
                / filtered_d_eff_x.len() as f64)
                .sqrt();
            (mean, std)
        } else {
            (d_eff_x_mean_mm, d_eff_x_std_mm)
        };

        let (d_eff_y_mean_filtered_mm, d_eff_y_std_filtered_mm) = if !filtered_indices.is_empty() {
            let filtered_d_eff_y: Vec<f64> =
                filtered_indices.iter().map(|&i| d_eff_y_mm[i]).collect();
            let mean = filtered_d_eff_y.iter().sum::<f64>() / filtered_d_eff_y.len() as f64;
            let std = (filtered_d_eff_y
                .iter()
                .map(|d| (d - mean).powi(2))
                .sum::<f64>()
                / filtered_d_eff_y.len() as f64)
                .sqrt();
            (mean, std)
        } else {
            (d_eff_y_mean_mm, d_eff_y_std_mm)
        };

        // Estimate D_eff from reference point
        let d_eff_x_theory_m = (lambda_ref_m * f_m) / (PI * w0x_ref_m);
        let d_eff_y_theory_m = (lambda_ref_m * f_m) / (PI * w0y_ref_m);
        let d_eff_x_theory_mm = d_eff_x_theory_m * 1e3;
        let d_eff_y_theory_mm = d_eff_y_theory_m * 1e3;

        // === 3. Linear fit w0 = A·λ ===
        let (a_x, _b_x) = linear_fit(&wavelength_m, &w0x_m);
        let (a_y, _b_y) = linear_fit(&wavelength_m, &w0y_m);

        let w0_fit_x_m: Vec<f64> = wavelength_m.iter().map(|lambda| a_x * lambda).collect();
        let w0_fit_y_m: Vec<f64> = wavelength_m.iter().map(|lambda| a_y * lambda).collect();
        let w0_fit_x_mm: Vec<f64> = w0_fit_x_m.iter().map(|w| w * 1e3).collect();
        let w0_fit_y_mm: Vec<f64> = w0_fit_y_m.iter().map(|w| w * 1e3).collect();

        let residuals_x_m: Vec<f64> = w0x_m
            .iter()
            .zip(w0_fit_x_m.iter())
            .map(|(a, b)| a - b)
            .collect();
        let residuals_y_m: Vec<f64> = w0y_m
            .iter()
            .zip(w0_fit_y_m.iter())
            .map(|(a, b)| a - b)
            .collect();
        let rmse_x_mm =
            (residuals_x_m.iter().map(|r| r.powi(2)).sum::<f64>() / n as f64).sqrt() * 1e3;
        let rmse_y_mm =
            (residuals_y_m.iter().map(|r| r.powi(2)).sum::<f64>() / n as f64).sqrt() * 1e3;

        // === 4. Theoretical model (constant D_eff) ===
        let w0_theory_x_m: Vec<f64> = wavelength_m
            .iter()
            .map(|lambda| (lambda * f_m) / (PI * d_eff_x_theory_m))
            .collect();
        let w0_theory_y_m: Vec<f64> = wavelength_m
            .iter()
            .map(|lambda| (lambda * f_m) / (PI * d_eff_y_theory_m))
            .collect();
        let w0_theory_x_mm: Vec<f64> = w0_theory_x_m.iter().map(|w| w * 1e3).collect();
        let w0_theory_y_mm: Vec<f64> = w0_theory_y_m.iter().map(|w| w * 1e3).collect();

        let residuals_theory_x_m: Vec<f64> = w0x_m
            .iter()
            .zip(w0_theory_x_m.iter())
            .map(|(a, b)| a - b)
            .collect();
        let residuals_theory_y_m: Vec<f64> = w0y_m
            .iter()
            .zip(w0_theory_y_m.iter())
            .map(|(a, b)| a - b)
            .collect();
        let rmse_theory_x_mm =
            (residuals_theory_x_m.iter().map(|r| r.powi(2)).sum::<f64>() / n as f64).sqrt() * 1e3;
        let rmse_theory_y_mm =
            (residuals_theory_y_m.iter().map(|r| r.powi(2)).sum::<f64>() / n as f64).sqrt() * 1e3;

        // === 5. Rayleigh range z_R = π·w0²/λ ===
        let z_r_x_m: Vec<f64> = w0x_m
            .iter()
            .zip(wavelength_m.iter())
            .map(|(w0, lambda)| PI * w0.powi(2) / lambda)
            .collect();
        let z_r_y_m: Vec<f64> = w0y_m
            .iter()
            .zip(wavelength_m.iter())
            .map(|(w0, lambda)| PI * w0.powi(2) / lambda)
            .collect();
        let z_r_x_mm: Vec<f64> = z_r_x_m.iter().map(|z| z * 1e3).collect();
        let z_r_y_mm: Vec<f64> = z_r_y_m.iter().map(|z| z * 1e3).collect();

        // z_R from linear fit: z_R = π·A²·λ
        let z_r_fit_x_m: Vec<f64> = wavelength_m
            .iter()
            .map(|lambda| PI * a_x.powi(2) * lambda)
            .collect();
        let z_r_fit_y_m: Vec<f64> = wavelength_m
            .iter()
            .map(|lambda| PI * a_y.powi(2) * lambda)
            .collect();
        let z_r_fit_x_mm: Vec<f64> = z_r_fit_x_m.iter().map(|z| z * 1e3).collect();
        let z_r_fit_y_mm: Vec<f64> = z_r_fit_y_m.iter().map(|z| z * 1e3).collect();

        // z_R from constant D_eff theory
        let z_r_theory_x_m: Vec<f64> = w0_theory_x_m
            .iter()
            .zip(wavelength_m.iter())
            .map(|(w0, lambda)| PI * w0.powi(2) / lambda)
            .collect();
        let z_r_theory_y_m: Vec<f64> = w0_theory_y_m
            .iter()
            .zip(wavelength_m.iter())
            .map(|(w0, lambda)| PI * w0.powi(2) / lambda)
            .collect();
        let z_r_theory_x_mm: Vec<f64> = z_r_theory_x_m.iter().map(|z| z * 1e3).collect();
        let z_r_theory_y_mm: Vec<f64> = z_r_theory_y_m.iter().map(|z| z * 1e3).collect();

        // === 6. Diagnostic assessment ===
        let cv_x_percent = (d_eff_x_std_m / d_eff_x_mean_m) * 100.0;
        let cv_y_percent = (d_eff_y_std_m / d_eff_y_mean_m) * 100.0;
        let is_diffraction_limited = cv_x_percent < 5.0 && cv_y_percent < 5.0;

        Ok(DiagnosticResults {
            frequencies_thz: frequencies_thz.to_vec(),
            wavelengths_um: wavelength_um,
            w0x_mm: w0x_mm.to_vec(),
            w0y_mm: w0y_mm.to_vec(),
            freq_ref_thz: freq_ref_actual / 1e12,
            lambda_ref_um: lambda_ref_m * 1e6,
            w0x_ref_mm: w0x_ref_m * 1e3,
            w0y_ref_mm: w0y_ref_m * 1e3,
            ratio_x,
            ratio_y,
            ratio_x_mean,
            ratio_x_std,
            ratio_y_mean,
            ratio_y_std,
            ratio_x_mean_filtered,
            ratio_x_std_filtered,
            ratio_y_mean_filtered,
            ratio_y_std_filtered,
            d_eff_x_mm,
            d_eff_y_mm,
            d_eff_x_mean_mm,
            d_eff_x_std_mm,
            d_eff_y_mean_mm,
            d_eff_y_std_mm,
            d_eff_x_mean_filtered_mm,
            d_eff_x_std_filtered_mm,
            d_eff_y_mean_filtered_mm,
            d_eff_y_std_filtered_mm,
            d_eff_x_theory_mm,
            d_eff_y_theory_mm,
            a_x,
            a_y,
            w0_fit_x_mm,
            w0_fit_y_mm,
            rmse_x_mm,
            rmse_y_mm,
            w0_theory_x_mm,
            w0_theory_y_mm,
            rmse_theory_x_mm,
            rmse_theory_y_mm,
            z_r_x_mm,
            z_r_y_mm,
            z_r_fit_x_mm,
            z_r_fit_y_mm,
            z_r_theory_x_mm,
            z_r_theory_y_mm,
            is_diffraction_limited,
            cv_x_percent,
            cv_y_percent,
        })
    }

    /// Compute diagnostics with custom parameters
    pub fn compute_with_params(
        frequencies_thz: &[f64],
        w0x_mm: &[f64],
        w0y_mm: &[f64],
        focal_length_mm: f64,
        freq_ref_thz: f64,
        aperture_d_mm: Option<f64>,
    ) -> anyhow::Result<Self> {
        if frequencies_thz.len() != w0x_mm.len() || frequencies_thz.len() != w0y_mm.len() {
            return Err(anyhow::anyhow!("Input arrays must have the same length"));
        }

        if frequencies_thz.is_empty() {
            return Err(anyhow::anyhow!("Input arrays cannot be empty"));
        }

        let n = frequencies_thz.len();

        // Convert to SI units
        let freq_hz: Vec<f64> = frequencies_thz.iter().map(|f| f * 1e12).collect();
        let wavelength_m: Vec<f64> = freq_hz.iter().map(|f| C_LIGHT / f).collect();
        let wavelength_um: Vec<f64> = wavelength_m.iter().map(|w| w * 1e6).collect();
        let w0x_m: Vec<f64> = w0x_mm.iter().map(|w| w * 1e-3).collect();
        let w0y_m: Vec<f64> = w0y_mm.iter().map(|w| w * 1e-3).collect();
        let f_m = focal_length_mm * 1e-3; // Use custom focal length
        let freq_ref_hz = freq_ref_thz * 1e12; // Use custom reference frequency

        // Find reference frequency closest to specified frequency
        let idx_ref = freq_hz
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                ((**a - freq_ref_hz).abs())
                    .partial_cmp(&((**b - freq_ref_hz).abs()))
                    .unwrap()
            })
            .map(|(i, _)| i)
            .unwrap();

        let freq_ref_actual = freq_hz[idx_ref];
        let lambda_ref_m = wavelength_m[idx_ref];
        let w0x_ref_m = w0x_m[idx_ref];
        let w0y_ref_m = w0y_m[idx_ref];

        // === 1. Calculate ratio π·w0/λ ===
        let ratio_x: Vec<f64> = w0x_m
            .iter()
            .zip(wavelength_m.iter())
            .map(|(w0, lambda)| PI * w0 / lambda)
            .collect();
        let ratio_y: Vec<f64> = w0y_m
            .iter()
            .zip(wavelength_m.iter())
            .map(|(w0, lambda)| PI * w0 / lambda)
            .collect();

        let ratio_x_mean = ratio_x.iter().sum::<f64>() / n as f64;
        let ratio_y_mean = ratio_y.iter().sum::<f64>() / n as f64;
        let ratio_x_std = (ratio_x
            .iter()
            .map(|r| (r - ratio_x_mean).powi(2))
            .sum::<f64>()
            / n as f64)
            .sqrt();
        let ratio_y_std = (ratio_y
            .iter()
            .map(|r| (r - ratio_y_mean).powi(2))
            .sum::<f64>()
            / n as f64)
            .sqrt();

        // === Filtered calculations for frequencies < 1 THz ===
        let filtered_indices: Vec<usize> = frequencies_thz
            .iter()
            .enumerate()
            .filter_map(|(i, &f)| if f < 1.0 { Some(i) } else { None })
            .collect();

        let (ratio_x_mean_filtered, ratio_x_std_filtered) = if !filtered_indices.is_empty() {
            let filtered_ratio_x: Vec<f64> = filtered_indices.iter().map(|&i| ratio_x[i]).collect();
            let mean = filtered_ratio_x.iter().sum::<f64>() / filtered_ratio_x.len() as f64;
            let std = (filtered_ratio_x
                .iter()
                .map(|r| (r - mean).powi(2))
                .sum::<f64>()
                / filtered_ratio_x.len() as f64)
                .sqrt();
            (mean, std)
        } else {
            (ratio_x_mean, ratio_x_std)
        };

        let (ratio_y_mean_filtered, ratio_y_std_filtered) = if !filtered_indices.is_empty() {
            let filtered_ratio_y: Vec<f64> = filtered_indices.iter().map(|&i| ratio_y[i]).collect();
            let mean = filtered_ratio_y.iter().sum::<f64>() / filtered_ratio_y.len() as f64;
            let std = (filtered_ratio_y
                .iter()
                .map(|r| (r - mean).powi(2))
                .sum::<f64>()
                / filtered_ratio_y.len() as f64)
                .sqrt();
            (mean, std)
        } else {
            (ratio_y_mean, ratio_y_std)
        };

        // === 2. Calculate D_eff ===
        let d_eff_x_m: Vec<f64> = ratio_x.iter().map(|r| f_m / r).collect();
        let d_eff_y_m: Vec<f64> = ratio_y.iter().map(|r| f_m / r).collect();

        let d_eff_x_mm: Vec<f64> = d_eff_x_m.iter().map(|d| d * 1e3).collect();
        let d_eff_y_mm: Vec<f64> = d_eff_y_m.iter().map(|d| d * 1e3).collect();

        let d_eff_x_mean_m = d_eff_x_m.iter().sum::<f64>() / n as f64;
        let d_eff_y_mean_m = d_eff_y_m.iter().sum::<f64>() / n as f64;
        let d_eff_x_std_m = (d_eff_x_m
            .iter()
            .map(|d| (d - d_eff_x_mean_m).powi(2))
            .sum::<f64>()
            / n as f64)
            .sqrt();
        let d_eff_y_std_m = (d_eff_y_m
            .iter()
            .map(|d| (d - d_eff_y_mean_m).powi(2))
            .sum::<f64>()
            / n as f64)
            .sqrt();

        let d_eff_x_mean_mm = d_eff_x_mean_m * 1e3;
        let d_eff_x_std_mm = d_eff_x_std_m * 1e3;
        let d_eff_y_mean_mm = d_eff_y_mean_m * 1e3;
        let d_eff_y_std_mm = d_eff_y_std_m * 1e3;

        // Calculate filtered D_eff means for frequencies < 1 THz
        let (d_eff_x_mean_filtered_mm, d_eff_x_std_filtered_mm) = if !filtered_indices.is_empty() {
            let filtered_d_eff_x: Vec<f64> =
                filtered_indices.iter().map(|&i| d_eff_x_mm[i]).collect();
            let mean = filtered_d_eff_x.iter().sum::<f64>() / filtered_d_eff_x.len() as f64;
            let std = (filtered_d_eff_x
                .iter()
                .map(|d| (d - mean).powi(2))
                .sum::<f64>()
                / filtered_d_eff_x.len() as f64)
                .sqrt();
            (mean, std)
        } else {
            (d_eff_x_mean_mm, d_eff_x_std_mm)
        };

        let (d_eff_y_mean_filtered_mm, d_eff_y_std_filtered_mm) = if !filtered_indices.is_empty() {
            let filtered_d_eff_y: Vec<f64> =
                filtered_indices.iter().map(|&i| d_eff_y_mm[i]).collect();
            let mean = filtered_d_eff_y.iter().sum::<f64>() / filtered_d_eff_y.len() as f64;
            let std = (filtered_d_eff_y
                .iter()
                .map(|d| (d - mean).powi(2))
                .sum::<f64>()
                / filtered_d_eff_y.len() as f64)
                .sqrt();
            (mean, std)
        } else {
            (d_eff_y_mean_mm, d_eff_y_std_mm)
        };

        // Use provided aperture or estimate from measured D_eff at reference frequency
        let d_eff_x_theory_m = if let Some(d_mm) = aperture_d_mm {
            d_mm * 1e-3 // Use provided aperture value
        } else {
            (lambda_ref_m * f_m) / (PI * w0x_ref_m) // Estimate from measurement
        };
        let d_eff_y_theory_m = if let Some(d_mm) = aperture_d_mm {
            d_mm * 1e-3 // Use provided aperture value
        } else {
            (lambda_ref_m * f_m) / (PI * w0y_ref_m) // Estimate from measurement
        };
        let d_eff_x_theory_mm = d_eff_x_theory_m * 1e3;
        let d_eff_y_theory_mm = d_eff_y_theory_m * 1e3;

        // === 3. Linear fit w0 = A·λ ===
        let (a_x, _b_x) = linear_fit(&wavelength_m, &w0x_m);
        let (a_y, _b_y) = linear_fit(&wavelength_m, &w0y_m);

        let w0_fit_x_m: Vec<f64> = wavelength_m.iter().map(|lambda| a_x * lambda).collect();
        let w0_fit_y_m: Vec<f64> = wavelength_m.iter().map(|lambda| a_y * lambda).collect();
        let w0_fit_x_mm: Vec<f64> = w0_fit_x_m.iter().map(|w| w * 1e3).collect();
        let w0_fit_y_mm: Vec<f64> = w0_fit_y_m.iter().map(|w| w * 1e3).collect();

        let residuals_x_m: Vec<f64> = w0x_m
            .iter()
            .zip(w0_fit_x_m.iter())
            .map(|(a, b)| a - b)
            .collect();
        let residuals_y_m: Vec<f64> = w0y_m
            .iter()
            .zip(w0_fit_y_m.iter())
            .map(|(a, b)| a - b)
            .collect();
        let rmse_x_mm =
            (residuals_x_m.iter().map(|r| r.powi(2)).sum::<f64>() / n as f64).sqrt() * 1e3;
        let rmse_y_mm =
            (residuals_y_m.iter().map(|r| r.powi(2)).sum::<f64>() / n as f64).sqrt() * 1e3;

        // === 4. Theoretical model (constant D_eff) ===
        let w0_theory_x_m: Vec<f64> = wavelength_m
            .iter()
            .map(|lambda| (lambda * f_m) / (PI * d_eff_x_theory_m))
            .collect();
        let w0_theory_y_m: Vec<f64> = wavelength_m
            .iter()
            .map(|lambda| (lambda * f_m) / (PI * d_eff_y_theory_m))
            .collect();
        let w0_theory_x_mm: Vec<f64> = w0_theory_x_m.iter().map(|w| w * 1e3).collect();
        let w0_theory_y_mm: Vec<f64> = w0_theory_y_m.iter().map(|w| w * 1e3).collect();

        let residuals_theory_x_m: Vec<f64> = w0x_m
            .iter()
            .zip(w0_theory_x_m.iter())
            .map(|(a, b)| a - b)
            .collect();
        let residuals_theory_y_m: Vec<f64> = w0y_m
            .iter()
            .zip(w0_theory_y_m.iter())
            .map(|(a, b)| a - b)
            .collect();
        let rmse_theory_x_mm =
            (residuals_theory_x_m.iter().map(|r| r.powi(2)).sum::<f64>() / n as f64).sqrt() * 1e3;
        let rmse_theory_y_mm =
            (residuals_theory_y_m.iter().map(|r| r.powi(2)).sum::<f64>() / n as f64).sqrt() * 1e3;

        // === 5. Rayleigh range z_R = π·w0²/λ ===
        let z_r_x_m: Vec<f64> = w0x_m
            .iter()
            .zip(wavelength_m.iter())
            .map(|(w0, lambda)| PI * w0.powi(2) / lambda)
            .collect();
        let z_r_y_m: Vec<f64> = w0y_m
            .iter()
            .zip(wavelength_m.iter())
            .map(|(w0, lambda)| PI * w0.powi(2) / lambda)
            .collect();
        let z_r_x_mm: Vec<f64> = z_r_x_m.iter().map(|z| z * 1e3).collect();
        let z_r_y_mm: Vec<f64> = z_r_y_m.iter().map(|z| z * 1e3).collect();

        // z_R from linear fit: z_R = π·A²·λ
        let z_r_fit_x_m: Vec<f64> = wavelength_m
            .iter()
            .map(|lambda| PI * a_x.powi(2) * lambda)
            .collect();
        let z_r_fit_y_m: Vec<f64> = wavelength_m
            .iter()
            .map(|lambda| PI * a_y.powi(2) * lambda)
            .collect();
        let z_r_fit_x_mm: Vec<f64> = z_r_fit_x_m.iter().map(|z| z * 1e3).collect();
        let z_r_fit_y_mm: Vec<f64> = z_r_fit_y_m.iter().map(|z| z * 1e3).collect();

        // z_R from constant D_eff theory
        let z_r_theory_x_m: Vec<f64> = w0_theory_x_m
            .iter()
            .zip(wavelength_m.iter())
            .map(|(w0, lambda)| PI * w0.powi(2) / lambda)
            .collect();
        let z_r_theory_y_m: Vec<f64> = w0_theory_y_m
            .iter()
            .zip(wavelength_m.iter())
            .map(|(w0, lambda)| PI * w0.powi(2) / lambda)
            .collect();
        let z_r_theory_x_mm: Vec<f64> = z_r_theory_x_m.iter().map(|z| z * 1e3).collect();
        let z_r_theory_y_mm: Vec<f64> = z_r_theory_y_m.iter().map(|z| z * 1e3).collect();

        // === 6. Diagnostic assessment ===
        let cv_x_percent = (d_eff_x_std_m / d_eff_x_mean_m) * 100.0;
        let cv_y_percent = (d_eff_y_std_m / d_eff_y_mean_m) * 100.0;
        let is_diffraction_limited = cv_x_percent < 5.0 && cv_y_percent < 5.0;

        Ok(DiagnosticResults {
            frequencies_thz: frequencies_thz.to_vec(),
            wavelengths_um: wavelength_um,
            w0x_mm: w0x_mm.to_vec(),
            w0y_mm: w0y_mm.to_vec(),
            freq_ref_thz: freq_ref_actual / 1e12,
            lambda_ref_um: lambda_ref_m * 1e6,
            w0x_ref_mm: w0x_ref_m * 1e3,
            w0y_ref_mm: w0y_ref_m * 1e3,
            ratio_x,
            ratio_y,
            ratio_x_mean,
            ratio_x_std,
            ratio_y_mean,
            ratio_y_std,
            ratio_x_mean_filtered,
            ratio_x_std_filtered,
            ratio_y_mean_filtered,
            ratio_y_std_filtered,
            d_eff_x_mm,
            d_eff_y_mm,
            d_eff_x_mean_mm,
            d_eff_x_std_mm,
            d_eff_y_mean_mm,
            d_eff_y_std_mm,
            d_eff_x_mean_filtered_mm,
            d_eff_x_std_filtered_mm,
            d_eff_y_mean_filtered_mm,
            d_eff_y_std_filtered_mm,
            d_eff_x_theory_mm,
            d_eff_y_theory_mm,
            a_x,
            a_y,
            w0_fit_x_mm,
            w0_fit_y_mm,
            rmse_x_mm,
            rmse_y_mm,
            w0_theory_x_mm,
            w0_theory_y_mm,
            rmse_theory_x_mm,
            rmse_theory_y_mm,
            z_r_x_mm,
            z_r_y_mm,
            z_r_fit_x_mm,
            z_r_fit_y_mm,
            z_r_theory_x_mm,
            z_r_theory_y_mm,
            is_diffraction_limited,
            cv_x_percent,
            cv_y_percent,
        })
    }

    /// Get a text summary of diagnostic results
    pub fn summary(&self) -> String {
        let mut s = String::new();
        s.push_str(&format!("=== PSF DIAGNOSTICS ===\n\n"));
        s.push_str(&format!(
            "Frequencies: {} bands\n",
            self.frequencies_thz.len()
        ));
        s.push_str(&format!(
            "Range: {:.3} - {:.3} THz\n",
            self.frequencies_thz.first().unwrap_or(&0.0),
            self.frequencies_thz.last().unwrap_or(&0.0)
        ));
        s.push_str(&format!(
            "       {:.1} - {:.1} µm\n\n",
            self.wavelengths_um.last().unwrap_or(&0.0),
            self.wavelengths_um.first().unwrap_or(&0.0)
        ));

        s.push_str(&format!(
            "Reference (nearest to target): {:.3} THz (λ={:.1} µm):\n",
            self.freq_ref_thz, self.lambda_ref_um
        ));
        s.push_str(&format!(
            "  w0x={:.3} mm, w0y={:.3} mm\n",
            self.w0x_ref_mm, self.w0y_ref_mm
        ));
        s.push_str(&format!(
            "  D_eff: X={:.2} mm, Y={:.2} mm\n\n",
            self.d_eff_x_theory_mm, self.d_eff_y_theory_mm
        ));

        s.push_str("1. RATIO π*w0/λ (should be constant if w0 ~ λ)\n");
        s.push_str(&format!(
            "  X (all): mean={:.4}, std={:.4}, CV={:.2}%\n",
            self.ratio_x_mean,
            self.ratio_x_std,
            (self.ratio_x_std / self.ratio_x_mean) * 100.0
        ));
        s.push_str(&format!(
            "  X (<1THz): mean={:.4}, std={:.4}, CV={:.2}%\n",
            self.ratio_x_mean_filtered,
            self.ratio_x_std_filtered,
            (self.ratio_x_std_filtered / self.ratio_x_mean_filtered) * 100.0
        ));
        s.push_str(&format!(
            "  Y (all): mean={:.4}, std={:.4}, CV={:.2}%\n",
            self.ratio_y_mean,
            self.ratio_y_std,
            (self.ratio_y_std / self.ratio_y_mean) * 100.0
        ));
        s.push_str(&format!(
            "  Y (<1THz): mean={:.4}, std={:.4}, CV={:.2}%\n\n",
            self.ratio_y_mean_filtered,
            self.ratio_y_std_filtered,
            (self.ratio_y_std_filtered / self.ratio_y_mean_filtered) * 100.0
        ));

        s.push_str("2. EFFECTIVE APERTURE D_eff (should be constant)\n");
        s.push_str(&format!(
            "  X (all): mean={:.2} mm, std={:.2} mm, CV={:.2}%\n",
            self.d_eff_x_mean_mm, self.d_eff_x_std_mm, self.cv_x_percent
        ));
        s.push_str(&format!(
            "  X (<1THz): mean={:.2} mm, std={:.2} mm, CV={:.2}%\n",
            self.d_eff_x_mean_filtered_mm,
            self.d_eff_x_std_filtered_mm,
            (self.d_eff_x_std_filtered_mm / self.d_eff_x_mean_filtered_mm) * 100.0
        ));
        s.push_str(&format!(
            "  Y (all): mean={:.2} mm, std={:.2} mm, CV={:.2}%\n",
            self.d_eff_y_mean_mm, self.d_eff_y_std_mm, self.cv_y_percent
        ));
        s.push_str(&format!(
            "  Y (<1THz): mean={:.2} mm, std={:.2} mm, CV={:.2}%\n",
            self.d_eff_y_mean_filtered_mm,
            self.d_eff_y_std_filtered_mm,
            (self.d_eff_y_std_filtered_mm / self.d_eff_y_mean_filtered_mm) * 100.0
        ));

        if self.is_diffraction_limited {
            s.push_str("  [OK] D_eff roughly constant -> w0 ~ λ likely VALID\n");
        } else {
            s.push_str("  [X]  D_eff varies -> aperture/coupling effects present\n");
        }

        // Filtered evaluation for < 1 THz
        let cv_x_filtered = (self.d_eff_x_std_filtered_mm / self.d_eff_x_mean_filtered_mm) * 100.0;
        let cv_y_filtered = (self.d_eff_y_std_filtered_mm / self.d_eff_y_mean_filtered_mm) * 100.0;
        let is_diffraction_limited_filtered = cv_x_filtered < 5.0 && cv_y_filtered < 5.0;

        if is_diffraction_limited_filtered {
            s.push_str("  [OK] D_eff (<1THz) roughly constant -> improved model validity\n\n");
        } else {
            s.push_str(
                "  [!]  D_eff (<1THz) still varies -> consider frequency-dependent effects\n\n",
            );
        }

        s.push_str("3. LINEAR FIT w0 = A*λ\n");
        s.push_str(&format!(
            "  X: A={:.6}, RMSE={:.3} mm\n",
            self.a_x, self.rmse_x_mm
        ));
        s.push_str(&format!(
            "  Y: A={:.6}, RMSE={:.3} mm\n\n",
            self.a_y, self.rmse_y_mm
        ));

        s.push_str("4. MODEL COMPARISON\n");
        s.push_str(&format!(
            "  Linear fit: X RMSE={:.3} mm, Y RMSE={:.3} mm\n",
            self.rmse_x_mm, self.rmse_y_mm
        ));
        s.push_str(&format!(
            "  Theory (D_eff const): X RMSE={:.3} mm, Y RMSE={:.3} mm\n\n",
            self.rmse_theory_x_mm, self.rmse_theory_y_mm
        ));

        if self.is_diffraction_limited {
            s.push_str("[OK] DIFFRACTION-LIMITED MODEL VALID\n");
            s.push_str(&format!(
                "  Use: w0x = {:.3}*λ(mm), w0y = {:.3}*λ(mm)\n",
                self.a_x * 1e3,
                self.a_y * 1e3
            ));
            s.push_str("  Then: z_R = π*A^2*λ (linear in λ)\n");
        } else {
            s.push_str("[X] APERTURE/CHROMATIC EFFECTS PRESENT\n");
            s.push_str("  Use measured w0(λ) directly (not w0~λ)\n");
            s.push_str("  Consider frequency-dependent D_eff(λ) model\n");
        }

        s
    }
}

/// Simple linear regression: y = a*x + b
fn linear_fit(x: &[f64], y: &[f64]) -> (f64, f64) {
    let n = x.len() as f64;
    let sum_x: f64 = x.iter().sum();
    let sum_y: f64 = y.iter().sum();
    let sum_xy: f64 = x.iter().zip(y.iter()).map(|(xi, yi)| xi * yi).sum();
    let sum_x2: f64 = x.iter().map(|xi| xi * xi).sum();

    let a = (n * sum_xy - sum_x * sum_y) / (n * sum_x2 - sum_x * sum_x);
    let b = (sum_y - a * sum_x) / n;

    (a, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diagnostic_computation() {
        // Test data: 5 frequencies from 0.5 to 2.5 THz
        let frequencies_thz = vec![0.5, 1.0, 1.5, 2.0, 2.5];

        // Simulated w0 values that follow w0 ∝ λ (diffraction-limited)
        // λ = c/f, so w0 should be inversely proportional to f
        let c = 299.792458; // Speed of light in mm/THz
        let a = 0.1; // Proportionality constant

        let w0x_mm: Vec<f64> = frequencies_thz.iter().map(|&f| a * (c / f)).collect();

        let w0y_mm = w0x_mm.clone();

        // Compute diagnostics
        let result = DiagnosticResults::compute(&frequencies_thz, &w0x_mm, &w0y_mm);

        assert!(result.is_ok(), "Diagnostic computation should succeed");

        let diag = result.unwrap();

        // Check that we have the correct number of data points
        assert_eq!(diag.frequencies_thz.len(), 5);
        assert_eq!(diag.wavelengths_um.len(), 5);
        assert_eq!(diag.w0x_mm.len(), 5);
        assert_eq!(diag.w0y_mm.len(), 5);

        // For perfect diffraction-limited system, CV should be very small
        println!("CV X: {:.2}%", diag.cv_x_percent);
        println!("CV Y: {:.2}%", diag.cv_y_percent);

        // Since our test data is perfectly linear, CV should be near zero
        assert!(
            diag.cv_x_percent < 0.1,
            "CV should be very small for perfect data"
        );
        assert!(
            diag.cv_y_percent < 0.1,
            "CV should be very small for perfect data"
        );

        // Should be classified as diffraction-limited
        assert!(diag.is_diffraction_limited, "Should be diffraction-limited");

        // Check that linear fit coefficient is close to expected value
        // Note: a is in meters, w0 in mm, λ in m → w0_mm = a * λ_m * 1000
        // So a_measured ≈ 0.1 / 1000 = 0.0001 m/m = 100 when measuring w0 in mm
        let a_expected = 100.0; // Because w0 is in mm and λ in m
        let tolerance = 0.1;
        assert!(
            (diag.a_x - a_expected).abs() < tolerance,
            "Linear fit coefficient should match: got {}, expected {}",
            diag.a_x,
            a_expected
        );
    }

    #[test]
    fn test_diagnostic_with_noise() {
        // Test data with some noise
        let frequencies_thz = vec![0.5, 1.0, 1.5, 2.0, 2.5];

        // w0 with some noise
        let w0x_mm = vec![60.0, 30.5, 20.2, 15.1, 12.0];
        let w0y_mm = vec![59.8, 30.3, 20.0, 15.0, 11.9];

        let result = DiagnosticResults::compute(&frequencies_thz, &w0x_mm, &w0y_mm);

        assert!(result.is_ok());

        let diag = result.unwrap();

        // Should still compute without errors
        assert!(diag.cv_x_percent > 0.0);
        assert!(diag.d_eff_x_mean_mm > 0.0);
        assert!(diag.d_eff_y_mean_mm > 0.0);

        // Check that summary doesn't panic
        let summary = diag.summary();
        assert!(summary.contains("PSF DIAGNOSTICS"));
        assert!(summary.contains("RATIO"));
        assert!(summary.contains("EFFECTIVE APERTURE"));
    }

    #[test]
    fn test_diagnostic_empty_input() {
        let frequencies_thz: Vec<f64> = vec![];
        let w0x_mm: Vec<f64> = vec![];
        let w0y_mm: Vec<f64> = vec![];

        let result = DiagnosticResults::compute(&frequencies_thz, &w0x_mm, &w0y_mm);

        // Should return an error for empty input
        assert!(result.is_err());
    }

    #[test]
    fn test_diagnostic_mismatched_lengths() {
        let frequencies_thz = vec![1.0, 2.0, 3.0];
        let w0x_mm = vec![30.0, 15.0]; // Wrong length
        let w0y_mm = vec![30.0, 15.0, 10.0];

        let result = DiagnosticResults::compute(&frequencies_thz, &w0x_mm, &w0y_mm);

        // Should return an error for mismatched lengths
        assert!(result.is_err());
    }
}
