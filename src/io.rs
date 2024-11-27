use crate::data::{HouseKeeping, Meta, ScannedImage};
use dotthz::{DotthzFile, DotthzMetaData};
use ndarray::{arr3, Array1, Array2, Array3, ArrayBase, Axis, Ix3, OwnedRepr};
use ndarray_npy::NpzReader;
use realfft::RealFftPlanner;
use std::error::Error;
use std::fs::File;
use std::path::PathBuf;

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
    scan.scaled_img = scan.raw_img.clone();

    scan.filtered_data = scan.scaled_data.clone();
    scan.filtered_img = scan.scaled_img.clone();

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

//
// // Function to save the data and metadata to an HDF5 file
// pub fn save_to_thz(
//     data_container: OutputFile,
//     metadata: &DotthzMetaData,
// ) -> Result<(), Box<dyn Error>> {
//     let data_signal_1 = arr2(
//         &data_container
//             .data
//             .time
//             .iter()
//             .zip(data_container.data.signal_1.iter())
//             .map(|(t, r)| [*t, *r]) // Interleave time and ref data
//             .collect::<Vec<[f32; 2]>>(),
//     );
//
//     let data_ref_1 = Array2::from_shape_vec(
//         (data_container.data.time_ref_1.len(), 2), // N rows, 2 columns
//         data_container
//             .data
//             .time_ref_1
//             .iter()
//             .zip(data_container.data.ref_1.iter())
//             .flat_map(|(t, r)| vec![*t, *r]) // Flatten and interleave the two vectors
//             .collect(), // Collect them into a Vec
//     )?;
//
//     let data_signal_2 = Array2::from_shape_vec(
//         (data_container.data.time_2.len(), 2), // Shape of the data: (N, 2)
//         data_container
//             .data
//             .time_2
//             .iter()
//             .zip(data_container.data.signal_2.iter())
//             .flat_map(|(t, r)| vec![*t, *r]) // Interleave time and ref data
//             .collect(),
//     )?;
//
//     let data_ref_2 = Array2::from_shape_vec(
//         (data_container.data.time_ref_2.len(), 2), // Shape of the data: (N, 2)
//         data_container
//             .data
//             .time_ref_2
//             .iter()
//             .zip(data_container.data.ref_2.iter())
//             .flat_map(|(t, r)| vec![*t, *r]) // Interleave time and ref data
//             .collect(),
//     )?;
//
//     let mut file = DotthzFile::new();
//
//     let mut measurement = DotthzMeasurement::default();
//     measurement
//         .datasets
//         .insert("Sample 1".to_string(), data_signal_1);
//     measurement
//         .datasets
//         .insert("Reference 1".to_string(), data_ref_1);
//     measurement
//         .datasets
//         .insert("Sample 2".to_string(), data_signal_2);
//     measurement
//         .datasets
//         .insert("Reference 2".to_string(), data_ref_2);
//     measurement.meta_data = metadata.clone();
//
//     file.groups.insert("Measurement 1".to_string(), measurement);
//
//     file.save(&data_container.filename)?; // open for writing
//
//     Ok(())
// }

// Function to open and read an HDF5 file
pub fn open_from_thz(
    file_path: &PathBuf,
    scan: &mut ScannedImage,
    metadata: &mut DotthzMetaData,
) -> Result<(), Box<dyn Error>> {
    // Open the HDF5 file for reading
    let file = DotthzFile::load(file_path)?;

    if let Some(group_name) = file.get_group_names()?.first() {
        if file.get_groups()?.len() > 1 {
            println!("found more than one group, opening only the first");
        }

        // For TeraFlash measurements we always just get the first entry
        let group = file.get_group(group_name)?;

        // get the metadata
        *metadata = file.get_meta_data(group_name)?;

        // Read datasets and populate DataContainer fields, skipping any that are missing
        // we do not care about the names given for the datasets, we assume the first is time, second contains the image cube
        if let Some(ds) = group.datasets()?.first() {
            if let Ok(arr) = ds.read_1d() {
                scan.time = arr;
            }
        }
        if let Some(ds) = group.datasets()?.get(1) {
            if let Ok(arr) = ds.read_dyn::<f32>() {
                if let Ok(arr3) = arr.into_dimensionality::<ndarray::Ix3>() {
                    scan.raw_data = arr3;
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
    scan.scaled_img = scan.raw_img.clone();

    scan.filtered_data = scan.scaled_data.clone();
    scan.filtered_img = scan.scaled_img.clone();

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
