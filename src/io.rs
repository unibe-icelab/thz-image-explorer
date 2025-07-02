//! This module provides utilities for working with spectroscopic data, covering file I/O operations
//! and data processing tasks for various file formats such as `.npy`, `.npz`, `.csv`, `.thz` (HDF5),
//! and `.vtk`.
//!
//! # Features
//! - **File Loading**: Supports `.npz` files for loading filter data, `.json` for metadata, and `.csv` for raw data.
//! - **File Export**: Converts voxel data to VTK format for visualization in third-party tools.
//! - **Signal Processing**: Includes FFT setup and intensity calculations for spectroscopic data.
//! - **Pattern Search**: Finds files with specific extensions in directories.
//! - **THz File Handling**: Specialized support for `.thz` (HDF5) files with reading, writing, and metadata operations.
//!
//! These functionalities are essential for managing and analyzing large-scale spectroscopic or
//! imaging datasets efficiently.

use crate::data_container::ScannedImageFilterData;
use crate::filters::psf::PSF;
use bevy_voxel_plot::InstanceData;
use dotthz::{DotthzFile, DotthzMetaData};
use ndarray::{Array0, Array1, Array2, Array3, Axis, Ix0, Ix1, Ix2, OwnedRepr};
use ndarray_npy::NpzReader;
use realfft::RealFftPlanner;
use std::error::Error;
use std::fs;
use std::fs::File;
use std::path::{Path, PathBuf};
use vtkio::model::{Attributes, ByteOrder, Cells, Piece};
use vtkio::{
    model::{
        Attribute, CellType, DataArray, DataSet, UnstructuredGridPiece, Version, VertexNumbers, Vtk,
    },
    IOBuffer,
};

use bevy_egui::egui::ColorImage;
use image::RgbaImage;

/// Exports voxel data to VTK file format for visualization in external applications.
///
/// This function converts instance data from the voxel plot system into a structured VTK file,
/// preserving position, color, and opacity information for each voxel. The resulting file
/// can be loaded into visualization software like ParaView or VTK-based viewers.
///
/// # Arguments
/// * `instances` - A slice of `InstanceData` containing the voxel information to export
/// * `cube_width` - Width of each voxel in model units
/// * `cube_height` - Height of each voxel in model units
/// * `cube_depth` - Depth of each voxel in model units
/// * `filename` - The path where the VTK file will be written
///
/// # Returns
/// * `Ok(())` - If the file was successfully written
/// * `Err(Box<dyn std::error::Error>)` - If an error occurred during file creation or writing
///
/// # Examples
/// ```
/// let voxels = vec![instance1, instance2, instance3];
/// export_to_vtk(&voxels, 0.1, 0.1, 0.1, "output_visualization.vtk")?;
/// ```
pub fn export_to_vtk(
    instances: &[InstanceData],
    filename: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Create points for each voxel center
    let mut points_vec = Vec::new();
    let mut colors_vec = Vec::new();
    let mut opacities_vec = Vec::new();

    for instance in instances {
        let pos = instance.pos_scale;
        points_vec.push(pos[0] as f64);
        points_vec.push(pos[1] as f64);
        points_vec.push(pos[2] as f64);

        colors_vec.push(instance.color[0] as f64);
        colors_vec.push(instance.color[1] as f64);
        colors_vec.push(instance.color[2] as f64);

        opacities_vec.push(instance.color[3] as f64);
    }

    // Create points IOBuffer
    let points_buffer = IOBuffer::from(points_vec);

    // Create cell data
    let connectivity_vec = (0..instances.len() as u64).collect::<Vec<_>>();
    let offsets_vec = (1..=instances.len() as u64).collect::<Vec<_>>();
    let types_vec = vec![CellType::Vertex; instances.len()];

    // Create vertex numbers using the XML variant
    let vertex_numbers = VertexNumbers::XML {
        connectivity: connectivity_vec,
        offsets: offsets_vec,
    };

    // Create the cells structure
    let cells = Cells {
        cell_verts: vertex_numbers,
        types: types_vec,
    };

    // Create RGB and opacity attributes
    let colors_buffer = IOBuffer::from(colors_vec);
    let colors_array = DataArray::vectors("RGB").with_data(colors_buffer);

    let opacities_buffer = IOBuffer::from(opacities_vec);
    let opacity_array = DataArray::scalars("Opacity", 1).with_data(opacities_buffer);

    // Create piece data with attributes
    let mut piece_data = Attributes::new();
    piece_data.point.push(Attribute::DataArray(colors_array));
    piece_data.point.push(Attribute::DataArray(opacity_array));

    // Create unstructured grid piece
    let piece = UnstructuredGridPiece {
        points: points_buffer,
        cells,
        data: piece_data,
    };

    // Create VTK file
    let vtk = Vtk {
        version: Version::default(),
        title: "Voxel Plot".to_string(),
        byte_order: ByteOrder::BigEndian,
        data: DataSet::UnstructuredGrid {
            meta: Default::default(),
            pieces: vec![Piece::Inline(Box::new(piece))],
        },
        file_path: None,
    };

    // Write to file
    let mut file = std::fs::File::create(filename)?;
    vtk.write_xml(&mut file)?;

    Ok(())
}

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

    // I read that in as Array2, hope this works
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
        low_cut: low_cut as f32,
        high_cut: high_cut as f32,
        start_freq: start_freq as f32,
        end_freq: end_freq as f32,
        n_filters: n_filters as i32,
        filters: filters.map(|&x| x as f32),
        filt_freqs: filt_freqs.map(|&x| x as f32),
        popt_x: x.map(|&x| x as f32),
        popt_y: y.map(|&x| x as f32),
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

/// Loads metadata from a THz file without reading the full dataset.
///
/// This function extracts only the metadata from a `.thz` file, which is useful for
/// quickly inspecting file properties without loading the potentially large data arrays.
/// The metadata is stored in the provided `DotthzMetaData` structure.
///
/// # Arguments
/// * `file_path` - A reference to the path of the `.thz` file to be read
/// * `metadata` - A mutable reference to a `DotthzMetaData` structure where metadata will be stored
///
/// # Returns
/// * `Ok(())` - If metadata was successfully loaded
/// * `Err(Box<dyn Error>)` - If an error occurred while opening the file or reading metadata
///
/// # Errors
/// Will return an error if:
/// - The file cannot be opened
/// - The "Image" group does not exist in the file
/// - The metadata in the file is corrupted or in an unexpected format
pub fn load_meta_data_of_thz_file(
    file_path: &PathBuf,
    metadata: &mut DotthzMetaData,
) -> Result<(), Box<dyn Error>> {
    // Create a new DotthzFile for reading
    let file = DotthzFile::open(file_path)?;

    // Define a group name
    let group_name = "Image";

    // Create datasets and write data
    *metadata = file.get_meta_data(group_name)?;
    Ok(())
}

/// Updates the metadata in an existing THz file without modifying the data arrays.
///
/// This function is useful for correcting or enhancing metadata information in a file
/// without rewriting the entire dataset. It will clear existing metadata and replace it
/// with the provided metadata structure.
///
/// # Arguments
/// * `file_path` - A reference to the path of the `.thz` file to be updated
/// * `metadata` - A reference to the `DotthzMetaData` structure containing the new metadata
///
/// # Returns
/// * `Ok(())` - If metadata was successfully updated
/// * `Err(Box<dyn Error>)` - If an error occurred during file access or metadata writing
///
/// # Errors
/// Will return an error if:
/// - The file cannot be opened in read-write mode
/// - The "Image" group does not exist in the file
/// - Writing the new metadata fails
pub fn update_meta_data_of_thz_file(
    file_path: &PathBuf,
    metadata: &DotthzMetaData,
) -> Result<(), Box<dyn Error>> {
    // Create a new DotthzFile for writing
    let file = DotthzFile::open_rw(file_path)?;

    // Define a group name
    let group_name = "Image";

    // clear the existing meta-data
    file.clear_meta_data(group_name)?;

    // Create datasets and write data
    let mut group = file.get_group(group_name)?;
    file.set_meta_data(&mut group, metadata)?;
    Ok(())
}

/// Saves spectroscopic data and metadata to a THz file.
///
/// This function creates a new `.thz` file and writes both the time-domain data arrays
/// and associated metadata. The file follows the standard THz file format with data
/// organized in an "Image" group.
///
/// # Arguments
/// * `file_path` - A reference to the path where the `.thz` file will be created
/// * `scan` - A reference to the `ScannedImageFilterData` containing the spectroscopic data
/// * `metadata` - A reference to the `DotthzMetaData` structure with metadata to include
///
/// # Returns
/// * `Ok(())` - If the file was successfully created and data was written
/// * `Err(Box<dyn Error>)` - If an error occurred during file creation or writing
///
/// # Errors
/// Will return an error if:
/// - The file cannot be created at the specified location
/// - Creating the "Image" group fails
/// - Writing datasets or metadata fails
///
/// # Note
/// This function writes only the raw data and time vectors. Derived data such as
/// FFT results, phases, and amplitudes are not saved to the file.
pub fn save_to_thz(
    file_path: &PathBuf,
    scan: &ScannedImageFilterData,
    metadata: &DotthzMetaData,
) -> Result<(), Box<dyn Error>> {
    // Create a new DotthzFile for writing
    let mut file = DotthzFile::create(file_path)?;

    // Define a group name
    let group_name = "Image";

    // Create datasets and write data
    file.add_group(group_name, metadata)?;

    // Save raw data
    if let Some(ds) = metadata.ds_description.iter().position(|d| d == "time") {
        let name = format!("ds{}", ds + 1);
        file.add_dataset(group_name, name.as_str(), scan.time.view())?;
    }

    // Save time data
    if let Some(ds) = metadata.ds_description.iter().position(|d| d == "dataset") {
        let name = format!("ds{}", ds + 1);
        file.add_dataset(group_name, name.as_str(), scan.data.view())?;
    }

    Ok(())
}

pub fn open_pulse_from_thz(
    file_path: &PathBuf,
    metadata: &mut DotthzMetaData,
) -> Result<(Array1<f32>, Array1<f32>), Box<dyn Error>> {
    // Open the HDF5 file for reading
    let file = DotthzFile::open(file_path)?;

    let mut time = Array1::<f32>::zeros(0);
    let mut signal = Array1::<f32>::zeros(0);

    if let Some(group_name) = file.get_group_names()?.first() {
        if file.get_groups()?.len() > 1 {
            log::info!("found more than one group, opening only the first");
        }
        // For TeraFlash measurements we always just get the first entry
        let group = file.get_group(group_name)?;

        // get the metadata
        *metadata = file.get_meta_data(group_name)?;

        if let Ok(datasets) = group.datasets() {
            log::info!(
                "Found {} datasets in group: {}, taking first one",
                datasets.len(),
                group_name
            );

            if let Some(dataset) = datasets.first() {
                if let Ok(arr) = dataset.read_2d::<f32>() {
                    time = arr.column(0).to_owned();
                    signal = arr.column(1).to_owned();
                }
            }
        } else {
            log::warn!("No datasets found in group: {}", group_name);
        }
    }
    Ok((time, signal))
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
pub fn open_scan_from_thz(
    file_path: &PathBuf,
    scan: &mut ScannedImageFilterData,
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
        if let Some(ds) = metadata.ds_description.iter().position(|d| d == "time") {
            if let Ok(ds) = group.dataset(format!("ds{}", ds + 1).as_str()) {
                if let Ok(arr) = ds.read_1d() {
                    scan.time = arr;
                }
            }
        }

        if let Some(ds) = metadata.ds_description.iter().position(|d| d == "dataset") {
            if let Ok(ds) = group.dataset(format!("ds{}", ds + 1).as_str()) {
                if let Ok(arr) = ds.read_dyn::<f32>() {
                    if let Ok(arr3) = arr.into_dimensionality::<ndarray::Ix3>() {
                        // check dimensions to make sure
                        if arr3.shape().len() == 3 {
                            scan.data = arr3;
                        }
                    }
                }
            }
        }
    }

    if scan.data.is_empty() {
        return Err("No 2D scan data found in the THz file".into());
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

    scan.img = Array2::zeros((scan.width, scan.height));

    for x in 0..scan.width {
        for y in 0..scan.height {
            // subtract bias
            let offset = scan.data[[x, y, 0]];
            scan.data
                .index_axis_mut(Axis(0), x)
                .index_axis_mut(Axis(0), y)
                .mapv_inplace(|p| p - offset);

            // calculate the intensity by summing the squares
            let sig_squared_sum = scan
                .data
                .index_axis(Axis(0), x)
                .index_axis(Axis(0), y)
                .mapv(|xi| xi * xi)
                .sum();
            scan.img[[x, y]] = sig_squared_sum;
        }
    }

    if let Some(dx) = metadata.md.get("dx [mm]") {
        scan.dx = dx.parse::<f32>().ok();
    }

    if let Some(dy) = metadata.md.get("dy [mm]") {
        scan.dy = dy.parse::<f32>().ok();
    }

    let n = scan.time.len();
    let rng = scan.time[n - 1] - scan.time[0];
    let mut real_planner = RealFftPlanner::<f32>::new();
    let r2c = real_planner.plan_fft_forward(n);
    let c2r = real_planner.plan_fft_inverse(n);
    let spectrum = r2c.make_output_vec();
    let freq = (0..spectrum.len()).map(|i| i as f32 / rng).collect();
    scan.frequency = freq;

    scan.r2c = Some(r2c);
    scan.c2r = Some(c2r);

    scan.phases = Array3::zeros((scan.width, scan.height, scan.frequency.len()));
    scan.amplitudes = Array3::zeros((scan.width, scan.height, scan.frequency.len()));
    scan.fft = Array3::zeros((scan.width, scan.height, scan.frequency.len()));

    Ok(())
}

/// Saves an image to a given file location.
///
/// The image is saved as PNG in the specified directory.
/// Currently, the function does not support saving large images.
///
/// # Arguments
/// * `img` - The `ColorImage` object to be saved.
/// * `file_path` - The directory path where the image will be saved.
#[allow(dead_code)]
fn save_image(img: &ColorImage, file_path: &Path) {
    let height = img.height();
    let width = img.width();
    let mut raw: Vec<u8> = vec![];
    for p in img.pixels.clone().iter() {
        raw.push(p.r());
        raw.push(p.g());
        raw.push(p.b());
        raw.push(p.a());
    }
    let img_to_save = RgbaImage::from_raw(width as u32, height as u32, raw)
        .expect("container should have the right size for the image dimensions");
    let mut image_path = file_path.to_path_buf();
    image_path.push("image.png");
    match img_to_save.save(image_path) {
        Ok(_) => {}
        Err(err) => {
            log::error!("error in saving image: {err:?}");
        }
    }
}
