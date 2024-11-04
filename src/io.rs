use std::error::Error;
use std::fs::File;
use std::path::PathBuf;

use ndarray::{Array1, Array2, Array3, Axis};
use ndarray_npy::NpzReader;
use realfft::RealFftPlanner;

use crate::data::{HouseKeeping, Meta, ScannedImage};

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
