use crate::psf_tool::curve_fitting::{CubicSpline, CurveFits};
use anyhow::Result;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

/// Export PSF curve fit coefficients to NPZ format
pub fn export_to_npz(path: &Path, curve_fits: &CurveFits) -> Result<()> {
    // Helper function to extract coefficients from a spline
    let extract_coeffs = |spline: &CubicSpline| -> (Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>) {
        let n = spline.coeffs.len();
        let mut a = Vec::with_capacity(n);
        let mut b = Vec::with_capacity(n);
        let mut c = Vec::with_capacity(n);
        let mut d = Vec::with_capacity(n);

        for coeff in &spline.coeffs {
            a.push(coeff[0]);
            b.push(coeff[1]);
            c.push(coeff[2]);
            d.push(coeff[3]);
        }

        (a, b, c, d)
    };

    // Extract coefficients for hybrid fits (wx, wy)
    let (wx_corr_a, wx_corr_b, wx_corr_c, wx_corr_d) =
        extract_coeffs(&curve_fits.wx_fit.correction);
    let (wy_corr_a, wy_corr_b, wy_corr_c, wy_corr_d) =
        extract_coeffs(&curve_fits.wy_fit.correction);

    // Extract coefficients for spline fits (x0, y0)
    let (x0_a, x0_b, x0_c, x0_d) = extract_coeffs(&curve_fits.x0_fit);
    let (y0_a, y0_b, y0_c, y0_d) = extract_coeffs(&curve_fits.y0_fit);

    // Create NPZ file using zip
    let file = File::create(path)?;
    let writer = BufWriter::new(file);
    let mut zip = zip::ZipWriter::new(writer);

    // Helper function to write array to NPZ
    let write_array =
        |zip: &mut zip::ZipWriter<BufWriter<File>>, name: &str, data: &[f64]| -> Result<()> {
            let options: zip::write::FileOptions<()> = zip::write::FileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
            zip.start_file(format!("{}.npy", name), options)?;

            // Write numpy array header manually
            let shape_str = format!("({},)", data.len());
            let header = format!(
                "{{'descr': '<f8', 'fortran_order': False, 'shape': {}, }}",
                shape_str
            );

            // NPY format: magic (6 bytes) + version (2 bytes) + header length (2 bytes) + header + data
            let magic = b"\x93NUMPY";
            let version = [0x01u8, 0x00u8];
            let header_len = ((header.len() + 1) as u16).to_le_bytes();

            zip.write_all(magic)?;
            zip.write_all(&version)?;
            zip.write_all(&header_len)?;
            zip.write_all(header.as_bytes())?;
            zip.write_all(b"\n")?;

            for &value in data {
                zip.write_all(&value.to_le_bytes())?;
            }

            Ok(())
        };

    // Write wx hybrid fit (base model + correction spline)
    write_array(&mut zip, "wx_base_a", &[curve_fits.wx_fit.a])?;
    write_array(&mut zip, "wx_base_b", &[curve_fits.wx_fit.b])?;
    write_array(
        &mut zip,
        "wx_corr_knots_thz",
        &curve_fits.wx_fit.correction.x,
    )?;
    write_array(
        &mut zip,
        "wx_corr_values_mm",
        &curve_fits.wx_fit.correction.y,
    )?;
    write_array(&mut zip, "wx_corr_coeff_a", &wx_corr_a)?;
    write_array(&mut zip, "wx_corr_coeff_b", &wx_corr_b)?;
    write_array(&mut zip, "wx_corr_coeff_c", &wx_corr_c)?;
    write_array(&mut zip, "wx_corr_coeff_d", &wx_corr_d)?;

    // Write wy hybrid fit (base model + correction spline)
    write_array(&mut zip, "wy_base_a", &[curve_fits.wy_fit.a])?;
    write_array(&mut zip, "wy_base_b", &[curve_fits.wy_fit.b])?;
    write_array(
        &mut zip,
        "wy_corr_knots_thz",
        &curve_fits.wy_fit.correction.x,
    )?;
    write_array(
        &mut zip,
        "wy_corr_values_mm",
        &curve_fits.wy_fit.correction.y,
    )?;
    write_array(&mut zip, "wy_corr_coeff_a", &wy_corr_a)?;
    write_array(&mut zip, "wy_corr_coeff_b", &wy_corr_b)?;
    write_array(&mut zip, "wy_corr_coeff_c", &wy_corr_c)?;
    write_array(&mut zip, "wy_corr_coeff_d", &wy_corr_d)?;

    // Write x0 fit coefficients
    write_array(&mut zip, "x0_knots_thz", &curve_fits.x0_fit.x)?;
    write_array(&mut zip, "x0_values_mm", &curve_fits.x0_fit.y)?;
    write_array(&mut zip, "x0_coeff_a", &x0_a)?;
    write_array(&mut zip, "x0_coeff_b", &x0_b)?;
    write_array(&mut zip, "x0_coeff_c", &x0_c)?;
    write_array(&mut zip, "x0_coeff_d", &x0_d)?;

    // Write y0 fit coefficients
    write_array(&mut zip, "y0_knots_thz", &curve_fits.y0_fit.x)?;
    write_array(&mut zip, "y0_values_mm", &curve_fits.y0_fit.y)?;
    write_array(&mut zip, "y0_coeff_a", &y0_a)?;
    write_array(&mut zip, "y0_coeff_b", &y0_b)?;
    write_array(&mut zip, "y0_coeff_c", &y0_c)?;
    write_array(&mut zip, "y0_coeff_d", &y0_d)?;

    zip.finish()?;
    Ok(())
}
