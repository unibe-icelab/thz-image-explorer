//! This module provides utilities for working with spectroscopic data, covering file I/O operations
//! and data processing tasks for various file formats such as `.npy`, `.npz`, `.csv`, and `.thz` (HDF5).
//!
//! # Features
//! - **File Loading**: Supports `.npz` files for loading filter data, `.json` for metadata, and `.csv` for raw data.
//! - **Signal Processing**: Includes FFT setup and intensity calculations for spectroscopic data.
//! - **Pattern Search**: Finds files with specific extensions in directories.
//!
//! These functionalities are essential for managing and analyzing large-scale spectroscopic or
//! imaging datasets efficiently.

use crate::data_container::{HouseKeeping, Meta, ScannedImage};
use crate::filters::psf::PSF;
use csv::ReaderBuilder;
use dotthz::{DotthzFile, DotthzMetaData};
use glob::glob;
use ndarray::{arr2, Array0, Array1, Array2, Array3, Axis, Ix0, Ix1, Ix2, OwnedRepr};
use ndarray_npy::NpzReader;
use realfft::RealFftPlanner;
use std::error::Error;
use std::fs;
use std::fs::File;
use std::path::{Path, PathBuf};

/// Loads a Point Spread Function (PSF) from a `.npz` file.
///
/// This function reads scalar and array values from the `.npz` file and constructs a `PSF` object
/// with the loaded values.
///
/// # Input File Format
/// The `.npz` file must contain the following datasets:
/// - Scalars:
///   - `"low_cut"`: Low-frequency cutoff.
///   - `"high_cut"`: High-frequency cutoff.
///   - `"start_freq"`: Starting frequency of the filter.
///   - `"end_freq"`: Ending frequency of the filter.
///   - `"n_filters"`: Number of filters in the dataset.
/// - Arrays:
///   - `"filters"`: A 2D array representing the filters.
///   - `"filt_freqs"`: A list of frequencies over which the filters are defined.
///   - `"[x_0, w_x]"`: A 2D array of X-coordinates for the PSF.
///   - `"[y_0, w_y]"`: A 2D array of Y-coordinates for the PSF.
///
/// # Arguments
/// * `file_path` - A reference to the file path of the `.npz` file to be loaded.
///
/// # Returns
/// * `Ok(PSF)` - A `PSF` object containing the loaded filter and spatial frequency data.
/// * `Err(Box<dyn Error>)` - An error if loading or parsing the `.npz` file fails.
///
/// # Errors
/// - The function will return an error if the `.npz` file cannot be opened or properly parsed.
/// - Missing or malformed datasets within the file will also trigger errors.
///
/// # Example
/// ```rust
/// use crate::filters::psf::PSF;
/// use std::path::PathBuf;
///
/// let file_path = PathBuf::from("example.npz");
/// match load_psf(&file_path) {
///     Ok(psf) => println!("Loaded PSF with {} filters.", psf.n_filters),
///     Err(err) => eprintln!("Error loading PSF: {}", err),
/// }
/// ```
pub fn load_psf(file_path: &PathBuf) -> Result<PSF, Box<dyn Error>> {
    let mut npz = NpzReader::new(File::open(file_path)?)?;

    // Load scalar values with explicit type annotations
    let low_cut_arr: Array0<f64> = npz
        .by_name::<OwnedRepr<f64>, Ix0>("low_cut")?
        .into_dimensionality()?;
    let low_cut = low_cut_arr.into_scalar();

    let high_cut_arr: Array0<f64> = npz
        .by_name::<OwnedRepr<f64>, Ix0>("high_cut")?
        .into_dimensionality()?;
    let high_cut = high_cut_arr.into_scalar();

    let start_freq_arr: Array0<f64> = npz
        .by_name::<OwnedRepr<f64>, Ix0>("start_freq")?
        .into_dimensionality()?;
    let start_freq = start_freq_arr.into_scalar();

    let end_freq_arr: Array0<f64> = npz
        .by_name::<OwnedRepr<f64>, Ix0>("end_freq")?
        .into_dimensionality()?;
    let end_freq = end_freq_arr.into_scalar();

    let n_filters_arr: Array0<i64> = npz
        .by_name::<OwnedRepr<i64>, Ix0>("n_filters")?
        .into_dimensionality()?;
    let n_filters = n_filters_arr.into_scalar();

    // Load arrays
    let filters = npz
        .by_name::<OwnedRepr<f64>, Ix2>("filters")?
        .into_dimensionality::<ndarray::Ix2>()?;
    let filt_freqs = npz
        .by_name::<OwnedRepr<f64>, Ix1>("filt_freqs")?
        .into_dimensionality::<ndarray::Ix1>()?;
    let x = npz
        .by_name::<OwnedRepr<f64>, Ix2>("[x_0, w_x]")?
        .into_dimensionality::<ndarray::Ix2>()?;
    let y = npz
        .by_name::<OwnedRepr<f64>, Ix2>("[y_0, w_y]")?
        .into_dimensionality::<ndarray::Ix2>()?;

    Ok(PSF {
        low_cut,
        high_cut,
        start_freq,
        end_freq,
        n_filters,
        filters,
        filt_freqs,
        popt_x: x,
        popt_y: y,
    })
}

/// Finds all files in the same directory as the input file that share the same extension.
///
/// If the provided file path does not have a valid directory or extension, it will return an
/// empty list of results.
///
/// # Arguments
/// * `file_path` - A reference to the path of the file to be checked.
///
/// # Returns
/// * A `Result` containing a vector of file paths that match the extension, or an error.
///
/// # Examples
/// ```
/// let matches = find_files_with_same_extension(Path::new("example.txt")).unwrap();
/// assert_eq!(matches.len(), 1); // if "example.txt" shares the directory
/// ```
pub fn find_files_with_same_extension(file_path: &Path) -> std::io::Result<Vec<PathBuf>> {
    // Convert the input path to a Path
    let path = Path::new(file_path);

    // Get the directory and extension of the file
    if let (Some(dir), Some(extension)) = (path.parent(), path.extension()) {
        // List all files in the directory
        let mut matching_files = Vec::new();
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let entry_path = entry.path();

            // Check if the entry is a file and has the same extension
            if entry_path.is_file() && entry_path.extension() == Some(extension) {
                matching_files.push(entry_path);
            }
        }
        matching_files.sort();
        Ok(matching_files)
    } else {
        // Return an empty list if the file has no directory or extension
        Ok(Vec::new())
    }
}

/// Opens a `.json` file and loads housekeeping metadata.
///
/// The extracted metadata is stored in a `HouseKeeping` struct, and the function returns
/// the width and height dimensions of the data.
///
/// # Arguments
/// * `hk` - A mutable reference to the `HouseKeeping` struct where metadata will be stored.
/// * `file_path` - The file path of the `.json` file to be opened.
///
/// # Returns
/// * A `Result` with the width and height of the dataset or an error if parsing fails.
///
/// # Errors
/// If the file cannot be opened or the JSON parsing fails, the function will return a descriptive error.
pub fn open_json(
    hk: &mut HouseKeeping,
    file_path: &PathBuf,
) -> Result<(usize, usize), Box<dyn Error>> {
    let text = std::fs::read_to_string(file_path).unwrap();

    // Parse the string into a dynamically-typed JSON structure.
    let meta: Meta = serde_json::from_str::<Meta>(&text).unwrap();

    hk.dx = meta.dx;
    hk.x_range[0] = meta.x_min;
    hk.x_range[1] = meta.x_max;
    hk.dy = meta.dy;
    hk.y_range[0] = meta.y_min;
    hk.y_range[1] = meta.y_max;

    Ok((meta.width, meta.height))
}

// pub fn open_from_csv(
//     data: &mut DataPoint,
//     file_path: &String,
//     file_path_fft: &String,
// ) -> Result<(), Box<dyn Error>> {
//     data.time = vec![];
//     data.signal_1 = vec![];
//     data.ref_1 = vec![];
//
//     data.frequencies_fft = vec![];
//     data.signal_1_fft = vec![];
//     data.phase_1_fft = vec![];
//     data.ref_1_fft = vec![];
//     data.ref_phase_1_fft = vec![];
//
//     let mut rdr = ReaderBuilder::new()
//         .has_headers(true)
//         .from_path(file_path)?;
//
//     for result in rdr.records() {
//         let row = result?;
//         data.time.push(row[0].parse::<f32>().unwrap());
//         data.signal_1.push(row[1].parse::<f32>().unwrap());
//         data.ref_1.push(row[2].parse::<f32>().unwrap());
//     }
//
//     let mut rdr = ReaderBuilder::new()
//         .has_headers(true)
//         .from_path(file_path_fft)?;
//
//     for result in rdr.records() {
//         let row = result?;
//         data.frequencies_fft
//             .push(row[0].parse::<f32>().unwrap() / 1000.0);
//         data.signal_1_fft.push(row[1].parse::<f32>().unwrap());
//         data.phase_1_fft.push(row[2].parse::<f32>().unwrap());
//         data.ref_1_fft.push(row[3].parse::<f32>().unwrap());
//         data.ref_phase_1_fft.push(row[4].parse::<f32>().unwrap());
//     }
//     Ok(())
// }

// pub fn save_to_csv(
//     data: &DataPoint,
//     file_path: &String,
//     file_path_fft: &String,
// ) -> Result<(), Box<dyn Error>> {
//     let mut wtr = WriterBuilder::new()
//         .has_headers(false)
//         .from_path(file_path)?;
//     // serialize does not work, so we do it with a loop..
//     wtr.write_record(&["Time_abs/ps", " Signal 1/nA", " Reference 1/nA"])?;
//     for i in 0..data.time.len() {
//         wtr.write_record(&[
//             data.time[i].to_string(),
//             data.signal_1[i].to_string(),
//             data.ref_1[i].to_string(),
//         ])?;
//     }
//     wtr.flush()?;
//
//     let mut wtr = WriterBuilder::new()
//         .has_headers(false)
//         .from_path(file_path_fft)?;
//     // serialize does not work, so we do it with a loop..
//     wtr.write_record(&[
//         "Frequency/GHz",
//         " Amplitude rel. 1",
//         " Phase 1",
//         " Ref.Amplitude rel. 1",
//         " Ref.Phase 1",
//     ])?;
//     for i in 0..data.frequencies_fft.len() {
//         wtr.write_record(&[
//             (data.frequencies_fft[i] * 1_000.0).round().to_string(),
//             data.signal_1_fft[i].to_string(),
//             data.phase_1_fft[i].to_string(),
//             data.ref_1_fft[i].to_string(),
//             data.ref_phase_1_fft[i].to_string(),
//         ])?;
//     }
//     wtr.flush()?;
//
//     Ok(())
// }

/// Opens a `.npz` file and loads it into a `ScannedImage` struct.
///
/// The function loads the time array and dataset into the given `ScannedImage`, computes the image matrix,
/// and sets FFT-related information for further signal processing.
///
/// # Arguments
/// * `scan` - A mutable reference to the `ScannedImage` where data will be stored.
/// * `file_path` - The file path of the `.npz` file to be opened.
///
/// # Returns
/// * A `Result` indicating success or error during the process.
///
/// # Errors
/// The function will return an error if:
/// * The file cannot be opened.
/// * The expected datasets (`time.npy`, `dataset.npy`) are missing or misformatted.
pub fn open_from_npz(scan: &mut ScannedImage, file_path: &PathBuf) -> Result<(), Box<dyn Error>> {
    let file = File::open(file_path)?;
    let mut npz = NpzReader::new(file)?;
    let time: Array1<f32> = npz.by_name("time.npy")?;
    let n = time.len();
    let rng = time[n - 1] - time[0];
    scan.time = time;
    let data: Array3<f32> = npz.by_name("dataset.npy")?;
    scan.raw_data = data;
    scan.raw_img = Array2::zeros((scan.width, scan.height));
    for x in 0..scan.width {
        for y in 0..scan.height {
            // subtract bias
            let offset = scan.raw_data[[x, y, 0]];
            scan.raw_data
                .index_axis_mut(Axis(0), x)
                .index_axis_mut(Axis(0), y)
                .mapv_inplace(|p| p - offset);

            // calculate the intensity by summing the squares
            let sig_squared_sum = scan
                .raw_data
                .index_axis(Axis(0), x)
                .index_axis(Axis(0), y)
                .mapv(|xi| xi * xi)
                .sum();
            scan.raw_img[[x, y]] = sig_squared_sum;
        }
    }

    scan.scaled_data = scan.raw_data.clone();

    scan.filtered_data = scan.scaled_data.clone();
    scan.filtered_img = scan.raw_img.clone();

    let mut real_planner = RealFftPlanner::<f32>::new();
    let r2c = real_planner.plan_fft_forward(n);
    let c2r = real_planner.plan_fft_inverse(n);
    let spectrum = r2c.make_output_vec();
    let freq = (0..spectrum.len()).map(|i| i as f32 / rng).collect();
    scan.frequencies = freq;
    scan.r2c = Some(r2c);
    scan.c2r = Some(c2r);
    Ok(())
}

// Function to write the metadata to an HDF5 file / dotTHz file
pub fn load_meta_data_of_thz_file(
    file_path: &PathBuf,
    metadata: &mut DotthzMetaData,
) -> Result<(), Box<dyn Error>> {
    // Create a new DotthzFile for reading
    let mut file = DotthzFile::open(file_path)?;

    // Define a group name
    let group_name = "Image";

    // Create datasets and write data
    *metadata = file.get_meta_data(group_name)?;
    Ok(())
}

// Function to write the metadata to an HDF5 file / dotTHz file
pub fn update_meta_data_of_thz_file(
    file_path: &PathBuf,
    metadata: &DotthzMetaData,
) -> Result<(), Box<dyn Error>> {
    // Create a new DotthzFile for writing
    let mut file = DotthzFile::open_rw(file_path)?;

    // Define a group name
    let group_name = "Image";

    // Create datasets and write data
    let mut group = file.get_group(group_name)?;
    file.set_meta_data(&mut group, metadata)?;
    Ok(())
}

// Function to save the data and metadata to an HDF5 file / dotTHz file
pub fn save_to_thz(
    file_path: &PathBuf,
    scan: &ScannedImage,
    metadata: &DotthzMetaData,
) -> Result<(), Box<dyn Error>> {
    // Create a new DotthzFile for writing
    let mut file = DotthzFile::create(file_path)?;

    // Define a group name
    let group_name = "Image";

    // Create datasets and write data
    file.add_group(group_name, metadata)?;

    // Save raw data
    file.add_dataset(group_name, "dataset", scan.raw_data.view())?;

    // Save time data
    file.add_dataset(group_name, "time", scan.time.view())?;

    Ok(())
}

/// Opens and reads data from an HDF5 `.thz` file, populating the scan data and metadata.
///
/// This function loads time and raw data arrays from HDF5 files containing spectroscopic information.
/// The function assumes the first dataset contains time points and the second one contains a multi-dimensional
/// dataset (e.g., an image cube).
///
/// # Arguments
/// * `file_path` - The path to the `.thz` file.
/// * `scan` - A mutable reference to the `ScannedImage` where the data will be stored.
/// * `metadata` - A mutable reference to `DotthzMetaData` for storing metadata.
///
/// # Returns
/// * A `Result` indicating either success or an error.
///
/// # Errors
/// Will return an error if:
/// - The `.thz` file cannot be found or opened.
/// - The time or data datasets are missing or misformatted.
pub fn open_from_thz(
    file_path: &PathBuf,
    scan: &mut ScannedImage,
    metadata: &mut DotthzMetaData,
) -> Result<(), Box<dyn Error>> {
    // Open the HDF5 file for reading
    let file = DotthzFile::open(file_path)?;

    if let Some(group_name) = file.get_group_names()?.first() {
        if file.get_groups()?.len() > 1 {
            log::info!("found more than one group, opening only the first");
        }

        // For TeraFlash measurements we always just get the first entry
        let group = file.get_group(group_name)?;

        // get the metadata
        *metadata = file.get_meta_data(group_name)?;

        // Read datasets and populate DataContainer fields, skipping any that are missing
        if let Some(ds1) = metadata.ds_description.iter().position(|d| d == "time") {
            if let Some(ds) = group.datasets().unwrap().get(ds1) {
                if let Ok(arr) = ds.read_1d() {
                    scan.time = arr;
                }
            }
        }
        if let Some(ds2) = metadata.ds_description.iter().position(|d| d == "dataset") {
            if let Some(ds) = group.datasets().unwrap().get(ds2) {
                if let Ok(arr) = ds.read_dyn::<f32>() {
                    if let Ok(arr3) = arr.into_dimensionality::<ndarray::Ix3>() {
                        // check dimensions to make sure
                        if arr3.shape().len() == 3 {
                            scan.raw_data = arr3;
                        }
                    }
                }
            }
        }
    }
    if let Some(w) = metadata.md.get("width") {
        if let Ok(width) = w.parse::<usize>() {
            scan.width = width;
        }
    }

    if let Some(h) = metadata.md.get("height") {
        if let Ok(height) = h.parse::<usize>() {
            scan.height = height;
        }
    }

    scan.raw_img = Array2::zeros((scan.width, scan.height));

    for x in 0..scan.width {
        for y in 0..scan.height {
            // subtract bias
            let offset = scan.raw_data[[x, y, 0]];
            scan.raw_data
                .index_axis_mut(Axis(0), x)
                .index_axis_mut(Axis(0), y)
                .mapv_inplace(|p| p - offset);

            // calculate the intensity by summing the squares
            let sig_squared_sum = scan
                .raw_data
                .index_axis(Axis(0), x)
                .index_axis(Axis(0), y)
                .mapv(|xi| xi * xi)
                .sum();
            scan.raw_img[[x, y]] = sig_squared_sum;
        }
    }

    scan.scaled_data = scan.raw_data.clone();

    scan.filtered_data = scan.scaled_data.clone();
    scan.filtered_img = scan.raw_img.clone();

    scan.dx = metadata.md.get("dx [mm]").unwrap().parse::<f32>().ok();
    scan.dy = metadata.md.get("dy [mm]").unwrap().parse::<f32>().ok();

    let n = scan.time.len();
    let rng = scan.time[n - 1] - scan.time[0];
    let mut real_planner = RealFftPlanner::<f32>::new();
    let r2c = real_planner.plan_fft_forward(n);
    let c2r = real_planner.plan_fft_inverse(n);
    let spectrum = r2c.make_output_vec();
    let freq = (0..spectrum.len()).map(|i| i as f32 / rng).collect();
    scan.frequencies = freq;
    scan.r2c = Some(r2c);
    scan.c2r = Some(c2r);

    Ok(())
}

/// Helper function to extract a substring between two delimiters.
fn extract_substring(text: &str, start: &str, end: &str) -> Option<String> {
    let start_idx = text.find(start)? + start.len();
    let end_idx = text[start_idx..].find(end)? + start_idx;
    Some(text[start_idx..end_idx].to_string())
}

/// Helper function for trimming signals (placeholder; implement your logic here).
pub fn get_windowed_signal(
    signal: &Vec<f64>,
    ratio: f64,
    lr: &str,
    window: &str,
    alpha: f64,
) -> (Vec<f64>, Vec<f64>) {
    // Implement signal processing logic here
    (signal.clone(), vec![1.0; signal.len()]) // Placeholder
}

pub fn load_psfs(
    raw_psf_path: &PathBuf,
    trim: bool,
) -> Result<(Array1<f64>, Array1<f64>, Array2<f64>, Array2<f64>, Vec<f64>), Box<dyn Error>> {
    let dirs = fs::read_dir(raw_psf_path)?
        .map(|entry| entry.unwrap().path())
        .collect::<Vec<_>>();
    let mut psf_t_x = Vec::new();
    let mut psf_t_y = Vec::new();
    let mut pos_x = Vec::new();
    let mut pos_y = Vec::new();
    let mut xx = Vec::new();
    let mut yy = Vec::new();
    let mut times = None;
    let mut len_traces = None;
    let mut nx = 0;
    let mut ny = 0;

    for dir in dirs.iter() {
        let path = dir.to_str().unwrap();
        if glob(&format!("{}/x*.csv", path))?.count() > 0 {
            nx += 1;
            for entry in glob(&format!("{}/x*.csv", path))? {
                let file = entry.unwrap();
                let x = extract_substring(file.to_str().unwrap(), "x=", ".csv")
                    .unwrap()
                    .parse::<f64>()?;
                pos_x.push(x);

                let mut reader = ReaderBuilder::new().has_headers(true).from_path(file)?;
                let df: Vec<f64> = reader
                    .records()
                    .map(|r| r.unwrap().get(1).unwrap().parse::<f64>().unwrap())
                    .collect();

                if times.is_none() {
                    times = Some(
                        reader
                            .records()
                            .map(|r| r.unwrap().get(0).unwrap().parse::<f64>().unwrap())
                            .collect(),
                    );
                }
                if len_traces.is_none() {
                    len_traces = Some(df.len());
                }
            }
            xx = pos_x.clone();
            xx.sort_by(|a, b| a.partial_cmp(b).unwrap());
        }

        if glob(&format!("{}/y*.csv", path))?.count() > 0 {
            ny += 1;
            for entry in glob(&format!("{}/y*.csv", path))? {
                let file = entry.unwrap();
                let y = extract_substring(file.to_str().unwrap(), "y=", ".csv")
                    .unwrap()
                    .parse::<f64>()?;
                pos_y.push(y);

                let mut reader = ReaderBuilder::new().has_headers(true).from_path(file)?;
                let df: Vec<f64> = reader
                    .records()
                    .map(|r| r.unwrap().get(1).unwrap().parse::<f64>().unwrap())
                    .collect();

                if times.is_none() {
                    times = Some(
                        reader
                            .records()
                            .map(|r| r.unwrap().get(0).unwrap().parse::<f64>().unwrap())
                            .collect(),
                    );
                }
                if len_traces.is_none() {
                    len_traces = Some(df.len());
                }
            }
            yy = pos_y.clone();
            yy.sort_by(|a, b| a.partial_cmp(b).unwrap());
        }
    }

    for &x in &xx {
        let mut psf_x_t = vec![0.0; len_traces.unwrap()];
        for dir in dirs.iter() {
            let path = dir.to_str().unwrap();
            if glob(&format!("{}/x*.csv", path))?.count() > 0 {
                let file = format!("{}/x={:.2}.csv", path, x);
                let mut reader = ReaderBuilder::new().has_headers(true).from_path(file)?;
                let df: Vec<f64> = reader
                    .records()
                    .map(|r| r.unwrap().get(1).unwrap().parse::<f64>().unwrap())
                    .collect();
                for (i, &val) in df.iter().enumerate() {
                    psf_x_t[i] += val / nx as f64;
                }
            }
        }
        psf_t_x.push(psf_x_t);
    }

    for &y in &yy {
        let mut psf_y_t = vec![0.0; len_traces.unwrap()];
        for dir in dirs.iter() {
            let path = dir.to_str().unwrap();
            if glob(&format!("{}/y*.csv", path))?.count() > 0 {
                let file = format!("{}/y={:.2}.csv", path, y);
                let mut reader = ReaderBuilder::new().has_headers(true).from_path(file)?;
                let df: Vec<f64> = reader
                    .records()
                    .map(|r| r.unwrap().get(1).unwrap().parse::<f64>().unwrap())
                    .collect();
                for (i, &val) in df.iter().enumerate() {
                    psf_y_t[i] += val / ny as f64;
                }
            }
        }
        psf_t_y.push(psf_y_t);
    }

    let mut np_psf_t_x = Array2::zeros((psf_t_x.len(), len_traces.unwrap()));
    let mut np_psf_t_y = Array2::zeros((psf_t_y.len(), len_traces.unwrap()));

    for (i, row) in psf_t_x.iter().enumerate() {
        np_psf_t_x.row_mut(i).assign(&Array1::from(row.clone()));
    }
    for (i, row) in psf_t_y.iter().enumerate() {
        np_psf_t_y.row_mut(i).assign(&Array1::from(row.clone()));
    }

    if trim {
        for i in 0..np_psf_t_x.len_of(Axis(0)) {
            let (trimmed, _) =
                get_windowed_signal(&np_psf_t_x.row(i).to_vec(), 0.9, "left", "tukey", 0.1);
            np_psf_t_x.row_mut(i).assign(&Array1::from(trimmed));
        }
        for i in 0..np_psf_t_y.len_of(Axis(0)) {
            let (trimmed, _) =
                get_windowed_signal(&np_psf_t_y.row(i).to_vec(), 0.9, "left", "tukey", 0.1);
            np_psf_t_y.row_mut(i).assign(&Array1::from(trimmed));
        }
    }

    Ok((
        Array1::from_vec(xx),
        Array1::from_vec(yy),
        np_psf_t_x,
        np_psf_t_y,
        times.unwrap(),
    ))
}
