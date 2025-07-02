//! This module implements a custom filter named `Deconvolution`, which operates on scanned images
//! and performs a deconvolution operation in the frequency domain.
//!
//! The implementation includes a Richardson-Lucy deconvolution algorithm placeholder,
//! allowing for further customization and parameterization.

use crate::config::ThreadCommunication;
use crate::data_container::ScannedImageFilterData;
use crate::filters::filter::{CopyStaticFieldsTrait, Filter, FilterConfig, FilterDomain};
use crate::filters::psf::{create_psf_2d, gaussian};
use crate::gui::application::GuiSettingsContainer;
use bevy_egui::egui::{self, Ui};
use filter_macros::{register_filter, CopyStaticFields};
use ndarray::{arr1, s, Array1, Array2, Array3, Axis, Zip};
use num_complex::Complex32;
use rayon::prelude::*;
use rustfft::{num_complex::Complex, FftPlanner};
use std::error::Error;

use cancellable_loops::par_for_each_cancellable_reduce;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, RwLock};

/// Represents a `Deconvolution` filter.
///
/// This filter is designed to perform deconvolution using a configurable number of iterations
/// and a defined frequency range. It is implemented to work in the frequency domain.
///
/// Fields:
/// - `n_iterations`: The number of iterations for performing the deconvolution.
#[register_filter]
#[derive(Clone, Debug, CopyStaticFields)]
/// Represents the Deconvolution filter configuration.
///
/// # Fields
/// - `n_iterations` (*usize*): The number of iterations for the deconvolution algorithm.
pub struct Deconvolution {
    // Number of iterations for the deconvolution algorithm
    pub n_iterations: usize,
}

/// Performs 1D convolution using FFT.
///
/// # Arguments
/// - `a` (*&Array1<f32>*): The first input array.
/// - `b` (*&Array1<f32>*): The second input array (kernel).
/// - `fft` (*&dyn rustfft::Fft<f64>*): The FFT implementation.
/// - `ifft` (*&dyn rustfft::Fft<f64>*): The inverse FFT implementation.
/// - `fft_size` (*usize*): The size of the FFT to be performed.
///
/// # Returns
/// - (*Array1<f32>*): The result of the convolution.
pub fn convolve1d(
    a: &Array1<f32>,
    b: &Array1<f32>,
    fft: &dyn rustfft::Fft<f64>,
    ifft: &dyn rustfft::Fft<f64>,
    fft_size: usize,
) -> Array1<f32> {
    // Pad input signals to the FFT size
    // Initialize padded arrays for the input signals with zero values
    let mut a_padded: Vec<Complex<f64>> = vec![Complex { re: 0.0, im: 0.0 }; fft_size];
    let mut b_padded: Vec<Complex<f64>> = vec![Complex { re: 0.0, im: 0.0 }; fft_size];

    // Calculate the shift length for aligning the convolution result
    let shift_len = (b.len() - 1) / 2;

    // Copy input data into the padded arrays
    a.iter().enumerate().for_each(|(i, &val)| {
        a_padded[i] = Complex {
            re: val as f64,
            im: 0.0,
        };
    });

    b.iter().enumerate().for_each(|(i, &val)| {
        b_padded[i] = Complex {
            re: val as f64,
            im: 0.0,
        };
    });

    // Perform FFT on both input signals
    fft.process(&mut a_padded);
    fft.process(&mut b_padded);

    // Perform pointwise multiplication in the frequency domain
    let mut result_freq: Vec<Complex<f64>> = a_padded
        .iter()
        .zip(b_padded.iter())
        .map(|(x, y)| x * y)
        .collect();

    // Perform inverse FFT to transform the result back to the time domain
    ifft.process(&mut result_freq);

    // Normalize the result by dividing by the FFT size and extract the real part
    Array1::from(
        result_freq[shift_len..a.len() + shift_len]
            .iter()
            .map(|c| (c.re / fft_size as f64) as f32) // Normalize by FFT size and cast to f32
            .collect::<Vec<f32>>(),
    )
}

/// Perform element-wise multiplication of two complex matrices
/// Performs element-wise multiplication of two complex matrices in the frequency domain.
///
/// # Arguments
/// - `a` (*&Array2<Complex<f32>>*): The first complex matrix.
/// - `b` (*&Array2<Complex<f32>>*): The second complex matrix.
///
/// # Returns
/// - (*Array2<Complex<f32>>*): The result of the element-wise multiplication.
fn multiply_freq_domain(
    a: &Array2<Complex<f32>>,
    b: &Array2<Complex<f32>>,
) -> Array2<Complex<f32>> {
    // Clone the first matrix to store the result
    let mut result = a.clone();

    // Perform element-wise multiplication of the two matrices
    Zip::from(&mut result)
        .and(b)
        .for_each(|r, &bval| *r *= bval);

    result
}

/// Pads a 2D array with zeros to a specified shape.
///
/// # Arguments
/// - `input` (*&Array2<f32>*): The input array to be padded.
/// - `padded_shape` (*tuple(usize, usize)*): The desired shape of the padded array.
///
/// # Returns
/// - (*Array2<Complex32>*): The padded array with complex values.
pub fn pad_array(input: &Array2<f32>, padded_shape: (usize, usize)) -> Array2<Complex32> {
    let (input_rows, input_cols) = input.dim();
    let (padded_rows, padded_cols) = padded_shape;

    // Ensure the padded dimensions are larger than or equal to the input dimensions
    assert!(padded_rows >= input_rows && padded_cols >= input_cols);

    // Initialize the output array with zeros
    let mut output = Array2::<Complex32>::zeros((padded_rows, padded_cols));

    // Copy the input array into the top-left corner of the output array
    // Convert each value from f32 to Complex32
    for y in 0..input_rows {
        for x in 0..input_cols {
            output[[y, x]] = Complex32::new(input[[y, x]], 0.0);
        }
    }

    // Return the padded array
    output
}

/// Perform 2D FFT (in-place) on a matrix
/// Performs 2D FFT or inverse FFT on a matrix (in-place).
///
/// # Arguments
/// - `matrix` (*&mut Array2<Complex<f32>>*): The matrix to be transformed.
/// - `fft_cols` (*&dyn rustfft::Fft<f32>*): The FFT implementation for columns.
/// - `fft_rows` (*&dyn rustfft::Fft<f32>*): The FFT implementation for rows.
/// - `inverse` (*bool*): Whether to perform an inverse FFT.
///
/// # Notes
/// - Normalizes the result if `inverse` is true.
fn fft2d(
    matrix: &mut Array2<Complex<f32>>,
    fft_cols: &dyn rustfft::Fft<f32>,
    fft_rows: &dyn rustfft::Fft<f32>,
    inverse: bool,
) -> Result<(), Box<dyn Error>> {
    let (rows, cols) = matrix.dim();

    // FFT on rows
    // Perform FFT on each row of the matrix
    for mut row in matrix.outer_iter_mut() {
        match row.as_slice_mut() {
            Some(slice) => {
                fft_cols.process(slice);
            }
            None => {
                return Err("Row is not contiguous, cannot process FFT".into());
            }
        }
    }

    // Perform FFT on each column of the matrix
    for x in 0..cols {
        // Extract the column as a vector
        let mut column: Vec<_> = (0..rows).map(|y| matrix[[y, x]]).collect();

        // Apply FFT to the column
        fft_rows.process(&mut column);

        // Write the transformed column back into the matrix
        for (y, val) in column.iter().enumerate() {
            matrix[[y, x]] = *val;
        }
    }

    // If performing an inverse FFT, normalize the matrix by dividing by the total number of elements
    if inverse {
        let scale = (rows * cols) as f32;
        matrix.mapv_inplace(|v| v / scale);
    }
    Ok(())
}

/// Direct 2D Convolution for small kernels
/// Performs direct 2D convolution for small kernels.
///
/// # Arguments
/// - `a` (*&Array2<f32>*): The input array.
/// - `b` (*&Array2<f32>*): The kernel array.
///
/// # Returns
/// - (*Array2<f32>*): The result of the convolution.
fn direct_convolve2d(a: &Array2<f32>, b: &Array2<f32>) -> Array2<f32> {
    let (a_rows, a_cols) = a.dim();
    let (b_rows, b_cols) = b.dim();

    let mut result = Array2::<f32>::zeros((a_rows, a_cols));

    let half_b_rows = b_rows / 2;
    let half_b_cols = b_cols / 2;

    for i in 0..a_rows {
        for j in 0..a_cols {
            let mut sum = 0.0;
            for m in 0..b_rows {
                for n in 0..b_cols {
                    let x = i as isize + m as isize - half_b_rows as isize;
                    let y = j as isize + n as isize - half_b_cols as isize;

                    if x >= 0 && y >= 0 && (x as usize) < a_rows && (y as usize) < a_cols {
                        sum += a[[x as usize, y as usize]] * b[[m, n]];
                    }
                }
            }
            result[[i, j]] = sum;
        }
    }
    result
}

/// FFT-based convolution (output same size as `a`)
/// Performs 2D convolution using FFT (output same size as `a`).
///
/// # Arguments
/// - `a` (*&Array2<f32>*): The input array.
/// - `b` (*&Array2<f32>*): The kernel array.
/// - `fft_cols` (*&dyn rustfft::Fft<f32>*): The FFT implementation for columns.
/// - `ifft_cols` (*&dyn rustfft::Fft<f32>*): The inverse FFT implementation for columns.
/// - `fft_rows` (*&dyn rustfft::Fft<f32>*): The FFT implementation for rows.
/// - `ifft_rows` (*&dyn rustfft::Fft<f32>*): The inverse FFT implementation for rows.
///
/// # Returns
/// - (*Array2<f32>*): The result of the convolution.
pub fn convolve2d(
    a: &Array2<f32>,
    b: &Array2<f32>,
    fft_cols: &dyn rustfft::Fft<f32>,
    ifft_cols: &dyn rustfft::Fft<f32>,
    fft_rows: &dyn rustfft::Fft<f32>,
    ifft_rows: &dyn rustfft::Fft<f32>,
) -> Result<Array2<f32>, Box<dyn std::error::Error>> {
    let (a_rows, a_cols) = a.dim();
    let (b_rows, b_cols) = b.dim();

    // If the kernel is small, we use direct convolution for efficiency
    const THRESHOLD: usize = 256;
    if b_rows * b_cols <= THRESHOLD {
        return Ok(direct_convolve2d(a, b));
    }

    // Calculate padding to ensure output size matches input
    let padded_rows = (a_rows + b_rows - 1).next_power_of_two();
    let padded_cols = (a_cols + b_cols - 1).next_power_of_two();

    // Pad both input arrays to the calculated dimensions
    let mut a_padded = pad_array(a, (padded_rows, padded_cols));
    let mut b_padded = pad_array(b, (padded_rows, padded_cols));

    // Perform FFT on both padded arrays
    if let Err(e) = fft2d(&mut a_padded, &*fft_cols, &*fft_rows, false) {
        log::error!("fft2d on a_padded failed: {}", e);
        return Err(e);
    }
    if let Err(e) = fft2d(&mut b_padded, &*fft_cols, &*fft_rows, false) {
        log::error!("fft2d on b_padded failed: {}", e);
        return Err(e);
    }

    // Multiply the two arrays in the frequency domain
    let mut result_freq = multiply_freq_domain(&a_padded, &b_padded);

    // Perform inverse FFT to transform back to the spatial domain
    if let Err(e) = fft2d(&mut result_freq, &*ifft_cols, &*ifft_rows, true) {
        log::error!("fft2d on result_freq failed: {}", e);
        return Err(e);
    }

    // For "same" size convolution, we need to extract the center part
    let start_row = (b_rows - 1) / 2;
    let start_col = (b_cols - 1) / 2;

    // Ensure we have enough data to extract the required output size
    let available_rows = result_freq.nrows();
    let available_cols = result_freq.ncols();

    if start_row + a_rows > available_rows || start_col + a_cols > available_cols {
        log::error!("Insufficient data after convolution: need {}x{} starting at ({}, {}), but only have {}x{}",
                   a_rows, a_cols, start_row, start_col, available_rows, available_cols);

        // Fallback: use direct convolution
        return Ok(direct_convolve2d(a, b));
    }

    // Extract the result with the same size as input 'a'
    let result_view = result_freq.slice(s![
        start_row..start_row + a_rows,
        start_col..start_col + a_cols
    ]);

    // Extract the real part of the complex result
    let mut result = Array2::<f32>::zeros((a_rows, a_cols));
    Zip::from(&mut result)
        .and(result_view)
        .for_each(|r, &c| *r = c.re);

    Ok(result)
}

impl Deconvolution {
    /// Computes the minimum maximum range for the deconvolution algorithm.
    /// Ensures that the range_max value is not smaller than the minimum allowable range (wmin).
    ///
    /// # Arguments
    /// - `range_max` (*f32*): The maximum range value.
    /// - `wmin` (*f32*): The minimum allowable range.
    ///
    /// # Returns
    /// - (*f32*): The adjusted maximum range.
    fn range_max_min(&self, range_max: f32, wmin: f32) -> f32 {
        // If range_max is smaller than wmin, return wmin; otherwise, return range_max
        if range_max < wmin {
            wmin
        } else {
            range_max
        }
    }

    /// Computes the filtered scan with the FIR filter by convolving each time trace with the filter.
    /// Applies a filter to a scanned image using convolution.
    ///
    /// # Arguments
    /// - `_scan` (*&ScannedImageFilterData*): The scanned image data to be filtered.
    /// - `filter` (*&Array1<f32>*): The filter to apply.
    ///
    /// # Returns
    /// - (*Array3<f32>*): The filtered image data.
    fn filter_scan(&self, _scan: &ScannedImageFilterData, filter: &Array1<f32>) -> Array3<f32> {
        let (rows, cols, depth) = _scan.data.dim();
        let mut filtered_data = Array3::<f32>::zeros((rows, cols, depth));

        // Calculate the convolution size and round it up to the next power of two for FFT efficiency
        let conv_size = _scan.data.slice(s![0, 0, ..]).len() + filter.len() - 1;
        let fft_size = conv_size.next_power_of_two();

        // Create FFT and inverse FFT plans
        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(fft_size);
        let ifft = planner.plan_fft_inverse(fft_size);

        // Filter each time trace in the scan
        filtered_data
            .axis_iter_mut(Axis(0)) // Iterate over the first axis (rows)
            .into_iter()
            .enumerate()
            .for_each(|(i, mut row)| {
                row.axis_iter_mut(Axis(0)) // Iterate over the second axis (columns)
                    .enumerate()
                    .for_each(|(j, mut slice)| {
                        // Extract the time trace for the current row and column
                        let input_slice = _scan.data.slice(s![i, j, ..]);
                        // Perform 1D convolution and assign the result to the slice
                        slice.assign(&convolve1d(
                            &input_slice.to_owned(),
                            &filter,
                            &*fft,
                            &*ifft,
                            fft_size,
                        ));
                    });
            });
        filtered_data
    }

    /// Performs Richardson-Lucy deconvolution on an image.
    ///
    /// # Arguments
    /// - `image` (*&Array2<f32>*): The input image to be deconvolved.
    /// - `psf` (*&Array2<f32>*): The point spread function (PSF).
    /// - `n_iterations` (*usize*): The number of iterations to perform.
    ///
    /// # Returns
    /// - (*Array2<f32>*): The deconvolved image.
    fn richardson_lucy(
        &self,
        image: &Array2<f32>,
        psf: &Array2<f32>,
        n_iterations: usize,
    ) -> Result<Array2<f32>, Box<dyn std::error::Error>> {
        // Flip the PSF kernel to create its mirrored version
        let psf_mirror = psf.slice(s![..;-1, ..;-1]).to_owned();

        // Calculate padding sizes for rows and columns
        let pad_y = psf.nrows() / 2;
        let pad_x = psf.ncols() / 2;

        let (h, w) = (image.nrows(), image.ncols());
        let padded_h = h + 2 * pad_y;
        let padded_w = w + 2 * pad_x;

        // Initialize the padded image with zeros
        let mut padded_image = Array2::<f32>::zeros((padded_h, padded_w));

        // Copy the input image into the center of the padded image
        padded_image
            .slice_mut(s![pad_y..pad_y + h, pad_x..pad_x + w])
            .assign(image);

        // Reflect the top and bottom edges of the image into the padding
        for i in 0..pad_y {
            let src_top = image.slice(s![pad_y - i, ..]);
            let src_bottom = image.slice(s![h - 2 - i, ..]);

            padded_image
                .slice_mut(s![i, pad_x..pad_x + w])
                .assign(&src_top);
            padded_image
                .slice_mut(s![pad_y + h + i, pad_x..pad_x + w])
                .assign(&src_bottom);
        }

        // Reflect the left and right edges of the image into the padding
        for j in 0..pad_x {
            let src_left = padded_image.slice(s![.., pad_x + (pad_x - j)]).to_owned();
            let src_right = padded_image.slice(s![.., pad_x + w - 2 - j]).to_owned();

            padded_image.slice_mut(s![.., j]).assign(&src_left);
            padded_image
                .slice_mut(s![.., pad_x + w + j])
                .assign(&src_right);
        }

        // Initialize the deconvolved image with the padded image as the initial guess
        let mut u = padded_image.clone();

        let eps: f32 = 1e-12; // Small constant to avoid division by zero

        // Calculate the next power of two for FFT dimensions
        let (n_rows, n_cols) = padded_image.dim();
        let n_rows = n_rows.next_power_of_two();
        let n_cols = n_cols.next_power_of_two();

        // Create FFT and inverse FFT plans
        let mut planner = FftPlanner::new();
        let fft_cols: Arc<dyn rustfft::Fft<f32>> = planner.plan_fft_forward(n_cols);
        let ifft_cols: Arc<dyn rustfft::Fft<f32>> = planner.plan_fft_inverse(n_cols);
        let fft_rows: Arc<dyn rustfft::Fft<f32>> = planner.plan_fft_forward(n_rows);
        let ifft_rows: Arc<dyn rustfft::Fft<f32>> = planner.plan_fft_inverse(n_rows);

        // Perform Richardson-Lucy iterations
        for _ in 0..n_iterations {
            // Convolve the current estimate with the PSF
            let ustarp = convolve2d(&u, psf, &*fft_cols, &*ifft_cols, &*fft_rows, &*ifft_rows)?;

            // Compute the relative blur
            let relative_blur = Zip::from(&padded_image)
                .and(&ustarp)
                .map_collect(|&d, &c| d / (c + eps));

            // Convolve the relative blur with the mirrored PSF
            let correction = convolve2d(
                &relative_blur,
                &psf_mirror,
                &*fft_cols,
                &*ifft_cols,
                &*fft_rows,
                &*ifft_rows,
            )?;

            // Update the estimate by multiplying with the correction
            Zip::from(&mut u).and(&correction).for_each(|e, &c| *e *= c);
        }

        // Crop the deconvolved image to the original size
        Ok(u.slice(s![pad_y..pad_y + h, pad_x..pad_x + w]).to_owned())
    }
}

impl Filter for Deconvolution {
    /// Creates a new `Deconvolution` filter with default settings.
    ///
    /// Default values:
    /// - `n_iterations`: 500
    fn new() -> Self {
        Deconvolution { n_iterations: 500 }
    }

    /// Resets the filter state. Not used.
    fn reset(&mut self, _time: &Array1<f32>, _shape: &[usize]) {
        // NOOP
    }

    /// Displays the filter settings in the GUI. Not used, no data to show.
    fn show_data(&mut self, _data: &ScannedImageFilterData) {
        // NOOP
    }

    fn config(&self) -> FilterConfig {
        FilterConfig {
            name: "Deconvolution".to_string(),
            description: "Frequency-dependent deconvolution for enhanced THz-TDS scans, accounting for beam width variations in time traces.\n\nCitation: A. Demion, L. L. Stöckli, N. Thomas and S. Zahno, \"Frequency-Dependent Deconvolution for Enhanced THz-TDS Scans: Accounting for Beam Width Variations in Time Traces,\" in IEEE Transactions on Terahertz Science and Technology, vol. 15, no. 3, pp. 505-513, May 2025".to_string(),
            hyperlink: Some((Some("TTHZ.2025.3546756".to_string()),"https://doi.org/10.1109/TTHZ.2025.3546756".to_string())),
            domain: FilterDomain::TimeAfterFFTPrioLast,
        }
    }

    /// Applies the deconvolution algorithm to a scanned image.
    ///
    /// # Arguments:
    /// - `_scan`: Mutable reference to the scanned image to be processed.
    /// - `_gui_settings`: Mutable reference to the GUI settings associated with the filter.
    ///
    /// # Notes:
    /// This method currently contains a placeholder for the Richardson-Lucy algorithm.
    fn filter(
        &mut self,
        input_data: &ScannedImageFilterData,
        gui_settings: &mut GuiSettingsContainer,
        progress_lock: &mut Arc<RwLock<Option<f32>>>,
        abort_flag: &Arc<AtomicBool>,
    ) -> ScannedImageFilterData {
        // Initialize the progress lock to indicate the start of the filtering process
        if let Ok(mut p) = progress_lock.write() {
            *p = Some(0.0);
        }

        let mut output_data = input_data.clone();

        // Check if the input data contains valid spatial resolution (dx, dy)
        if input_data.dx.is_none() || input_data.dy.is_none() {
            log::error!("No data loaded, skipping deconvolution.");
            if let Ok(mut p) = progress_lock.write() {
                *p = None;
            }
            return input_data.clone();
        }

        if gui_settings.psf.filters.is_empty() {
            log::error!("PSF filters appear empty — a PSF may not have been loaded. Skipping deconvolution.");
            if let Ok(mut p) = progress_lock.write() {
                *p = None;
            }
            return input_data.clone();
        }
        if gui_settings.psf.popt_x.is_empty() || gui_settings.psf.popt_y.is_empty() {
            log::error!("PSF popt_x or popt_y appear empty — a PSF may not have been loaded correctly. Skipping deconvolution.");
            if let Ok(mut p) = progress_lock.write() {
                *p = None;
            }
            return input_data.clone();
        }

        // Get image dimensions for bounds checking
        let (img_rows, img_cols, _) = input_data.data.dim();

        // Check minimum image size for meaningful deconvolution
        const MIN_IMAGE_SIZE: usize = 16;
        if img_rows < MIN_IMAGE_SIZE || img_cols < MIN_IMAGE_SIZE {
            log::warn!(
            "Image dimensions ({}x{}) are too small for deconvolution (minimum {}x{}). Skipping.",
            img_rows, img_cols, MIN_IMAGE_SIZE, MIN_IMAGE_SIZE
        );
            if let Ok(mut p) = progress_lock.write() {
                *p = None;
            }
            return input_data.clone();
        }

        // Log the start of the deconvolution process and initialize the output data array
        log::info!("Starting deconvolution filter...");
        output_data.data = Array3::zeros((
            output_data.data.dim().0,
            output_data.data.dim().1,
            output_data.data.dim().2,
        ));

        // Pre-calculation of min and max values to avoid recalculating them in each iteration
        // Calculate the minimum and maximum widths for the PSF in the X direction
        let (wx_min, wx_max) = gui_settings
            .psf
            .popt_x
            .rows()
            .into_iter()
            .filter_map(|row| row[1].is_finite().then_some(row[1]))
            .fold((f32::INFINITY, f32::NEG_INFINITY), |(min, max), val| {
                (min.min(val), max.max(val))
            });

        let (wy_min, wy_max) = gui_settings
            .psf
            .popt_y
            .rows()
            .into_iter()
            .filter_map(|row| row[1].is_finite().then_some(row[1]))
            .fold((f32::INFINITY, f32::NEG_INFINITY), |(min, max), val| {
                (min.min(val), max.max(val))
            });

        let w_min = wx_min.min(wy_min);
        let w_max = wx_max.max(wy_max);

        let (dx, dy) = match (input_data.dx, input_data.dy) {
            (Some(dx_val), Some(dy_val)) => (dx_val as f32, dy_val as f32),
            _ => {
                log::error!("dx or dy is missing in input_data");
                if let Ok(mut p) = progress_lock.write() {
                    *p = None;
                }
                return input_data.clone();
            }
        };

        // Check if maximum PSF dimensions would be too large for the image
        let max_psf_width_x = ((wx_max / dx).ceil() as usize * 2 + 1).max(3);
        let max_psf_width_y = ((wy_max / dy).ceil() as usize * 2 + 1).max(3);

        if max_psf_width_x >= img_cols || max_psf_width_y >= img_rows {
            log::warn!(
            "Maximum PSF dimensions ({}x{}) are too large for image dimensions ({}x{}). Skipping deconvolution.",
            max_psf_width_x, max_psf_width_y, img_rows, img_cols
        );
            if let Ok(mut p) = progress_lock.write() {
                *p = None;
            }
            return input_data.clone();
        }

        // Error flag to track if any non-contiguous rows are encountered
        let error_flag = Arc::new(AtomicBool::new(false));
        let error_flag_clone = Arc::clone(&error_flag);

        let processed_data = par_for_each_cancellable_reduce(
            gui_settings
                .psf
                .filters
                .outer_iter()
                .enumerate()
                .par_bridge(),
            &abort_flag,
            |(i, _)| {
                let error_flag = Arc::clone(&error_flag_clone);
                if let Ok(mut p) = progress_lock.write() {
                    if if let Some(p_old) = *p {
                        p_old < (i as f32) / (gui_settings.psf.n_filters as f32)
                    } else {
                        false
                    } {
                        *p = Some((i as f32) / (gui_settings.psf.n_filters as f32));
                    }
                }

                // Calculating the range for the PSF
                let range_max_x = self.range_max_min(
                    (gui_settings.psf.popt_x.row(i)[1] as f32
                        + gui_settings.psf.popt_x.row(i)[0].abs() as f32)
                        * 3.0,
                    2.5,
                );
                let range_max_y = self.range_max_min(
                    (gui_settings.psf.popt_y.row(i)[1] as f32
                        + gui_settings.psf.popt_y.row(i)[0].abs() as f32)
                        * 3.0,
                    2.5,
                );

                let range_max_x = (range_max_x / dx).floor() * dx + dx;
                let range_max_y = (range_max_y / dy).floor() * dy + dy;

                // Constrain PSF size to be smaller than image dimensions
                let max_allowed_x = (img_cols as f32 - 2.0) * dx / 2.0;
                let max_allowed_y = (img_rows as f32 - 2.0) * dy / 2.0;
                let constrained_range_x = range_max_x.min(max_allowed_x);
                let constrained_range_y = range_max_y.min(max_allowed_y);

                if constrained_range_x != range_max_x || constrained_range_y != range_max_y {
                    log::warn!(
                        "PSF range constrained from ({:.2}, {:.2}) to ({:.2}, {:.2}) for filter {}",
                        range_max_x,
                        range_max_y,
                        constrained_range_x,
                        constrained_range_y,
                        i
                    );
                }

                let x: Vec<f32> = (-((constrained_range_x / dx).floor() as isize)
                    ..=((constrained_range_x / dx).floor() as isize))
                    .map(|i| i as f32 * dx)
                    .collect();
                let y: Vec<f32> = (-((constrained_range_y / dy).floor() as isize)
                    ..=((constrained_range_y / dy).floor() as isize))
                    .map(|i| i as f32 * dy)
                    .collect();

                let popt_x_view = gui_settings.psf.popt_x.row(i);
                let popt_x_row = match popt_x_view.as_slice() {
                    Some(slice) => slice,
                    None => {
                        log::error!("popt_x row {i} is not contiguous");
                        error_flag.store(true, std::sync::atomic::Ordering::Relaxed);
                        return None;
                    }
                };

                let popt_y_view = gui_settings.psf.popt_y.row(i);
                let popt_y_row = match popt_y_view.as_slice() {
                    Some(slice) => slice,
                    None => {
                        log::error!("popt_y row {i} is not contiguous");
                        error_flag.store(true, std::sync::atomic::Ordering::Relaxed);
                        return None;
                    }
                };

                let gaussian_x = gaussian(&arr1(&x), popt_x_row).to_vec();
                let gaussian_y = gaussian(&arr1(&y), popt_y_row).to_vec();

                let psf_2d = create_psf_2d(gaussian_x, gaussian_y, x, y, dx, dy);

                // Filter scan data in-place
                let mut filtered_data =
                    self.filter_scan(&input_data, &gui_settings.psf.filters.row(i).to_owned());

                // Compute filtered image
                let filtered_image = filtered_data.mapv(|x| x * x).sum_axis(Axis(2));

                let n_iter = (((gui_settings.psf.popt_x.row(i)[1] - w_min) / (w_max - w_min)
                    * (self.n_iterations as f32 - 1.0)
                    + 1.0)
                    .floor()) as usize;

                let deconvolved_image = match self.richardson_lucy(&filtered_image, &psf_2d, n_iter)
                {
                    Ok(image) => image.mapv(|x| x.max(0.0)),
                    Err(e) => {
                        log::error!("Richardson-Lucy deconvolution failed for iteration {i}: {e}");
                        error_flag.store(true, std::sync::atomic::Ordering::Relaxed);
                        return None;
                    }
                };

                let mut deconvolution_gains =
                    Array2::zeros((output_data.data.dim().0, output_data.data.dim().1));

                // Compute deconvolution gains
                Zip::from(&deconvolved_image)
                    .and(&filtered_image)
                    .and(&mut deconvolution_gains)
                    .for_each(|&u, &d, g| *g = (u / d).sqrt());

                // Apply deconvolution gains in-place
                let shape = filtered_data.dim();
                for i in 0..shape.0 {
                    for j in 0..shape.1 {
                        let gain = deconvolution_gains[[i, j]];
                        filtered_data
                            .slice_mut(s![i, j, ..])
                            .mapv_inplace(|x| x * gain);
                    }
                }

                Some(filtered_data)
            },
            |mut acc, data| {
                acc += &data;
                acc
            },
            Array3::zeros(input_data.data.dim()), // initial accumulator
        );

        // Check if any errors occurred during processing
        if error_flag.load(std::sync::atomic::Ordering::Relaxed) {
            log::error!(
                "Deconvolution failed due to non-contiguous data rows. Returning original data."
            );
            if let Ok(mut p) = progress_lock.write() {
                *p = None;
            }
            return input_data.clone();
        }
        // Update the progress lock to indicate the completion of the filtering process
        if let Ok(mut p) = progress_lock.write() {
            *p = Some(1.0);
        }

        output_data.data = processed_data;
        output_data.img = output_data.data.mapv(|x| x * x).sum_axis(Axis(2));

        log::info!("\nDeconvolution filter completed.");

        // Reset the progress lock to indicate that the process has finished
        if let Ok(mut p) = progress_lock.write() {
            *p = None;
        }

        output_data
    }

    fn ui(
        &mut self,
        ui: &mut Ui,
        _thread_communication: &mut ThreadCommunication,
        _panel_width: f32,
    ) -> egui::Response {
        let mut clicked = false;
        let mut response = ui
            .horizontal(|ui| {
                let button_response = ui.add(egui::Button::new("Apply"));
                if button_response.clicked() {
                    clicked = true;
                }
                button_response
            })
            .inner;
        if clicked {
            response.mark_changed();
        }
        response
    }
}
