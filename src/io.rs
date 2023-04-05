use std::error::Error;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

use csv::{ReaderBuilder, WriterBuilder};
use ndarray::{Array1, Array2, Array3, Axis};
use ndarray_npy::NpzReader;
use ndarray_npy::ReadNpyExt;
use realfft::num_traits::real::Real;
use realfft::RealFftPlanner;
use serde_json::{Number, Value};

use crate::data::{DataPoint, HouseKeeping, Meta, ScannedImage};
use crate::math_tools::make_fft;

pub fn open_hk(
    hk: &mut HouseKeeping,
    file_path: &PathBuf,
) -> Result<(usize, usize), Box<dyn Error>> {
    let mut rdr = ReaderBuilder::new()
        .delimiter(b',')
        .has_headers(true)
        .from_path(file_path)?;
    // data
    let mut x = 0;
    let mut y = 0;
    if let Some(result) = rdr.records().next() {
        let record = result?;
        x = record[1].parse::<f64>().unwrap() as usize;
        y = record[2].parse::<f64>().unwrap() as usize;
        hk.ambient_temperature = record[3].parse::<f64>().unwrap();
        hk.sample_temperature = record[4].parse::<f64>().unwrap();
        hk.ambient_pressure = record[5].parse::<f64>().unwrap();
        hk.ambient_humidity = record[6].parse::<f64>().unwrap();
    }
    Ok((x, y))
}

pub fn open_conf(
    hk: &mut HouseKeeping,
    file_path: &PathBuf,
) -> Result<(usize, usize), Box<dyn Error>> {
    let mut rdr = ReaderBuilder::new()
        .delimiter(b',')
        .has_headers(true)
        .from_path(file_path)?;
    let mut width = 0;
    let mut height = 0;
    if let Some(result) = rdr.records().next() {
        let record = result?;
        width = record[1].parse::<usize>().unwrap();
        height = record[2].parse::<usize>().unwrap();
        hk.dx = record[3].parse::<f32>().unwrap();
        hk.x_range[0] = record[4].parse::<f32>().unwrap();
        hk.x_range[1] = record[5].parse::<f32>().unwrap();
        hk.dy = record[6].parse::<f32>().unwrap();
        hk.y_range[0] = record[7].parse::<f32>().unwrap();
        hk.y_range[1] = record[8].parse::<f32>().unwrap();
        hk.ambient_temperature = record[9].parse::<f64>().unwrap();
        hk.sample_temperature = record[10].parse::<f64>().unwrap();
        hk.ambient_pressure = record[11].parse::<f64>().unwrap();
        hk.ambient_humidity = record[12].parse::<f64>().unwrap();
    }
    Ok((width, height))
}

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

pub fn open_from_npy(
    data: &mut DataPoint,
    file_path: &PathBuf,
    file_path_fft: &PathBuf,
) -> Result<(), Box<dyn Error>> {
    let reader = File::open(file_path)?;
    let arr = Array2::<f32>::read_npy(reader)?;
    todo!();
    // data.time = arr.row(0).iter().copied().collect();
    // data.signal_1 = arr.row(1).iter().copied().collect();
    // data.ref_1 = arr.row(2).iter().copied().collect();
    //
    // let reader = File::open(file_path_fft)?;
    // let arr = Array2::<f32>::read_npy(reader)?;
    //
    // data.frequencies_fft = arr.row(0).iter().copied().collect();
    // data.signal_1_fft = arr.row(1).iter().copied().collect();
    // data.phase_1_fft = arr.row(2).iter().copied().collect();
    // data.ref_1_fft = arr.row(3).iter().copied().collect();
    // data.ref_phase_1_fft = arr.row(4).iter().copied().collect();
    //
    // Ok(())
}

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
    scan.filtered_data = scan.raw_data.clone();
    scan.filtered_img = scan.raw_img.clone();

    scan.scaled_data = scan.raw_data.clone();
    scan.scaled_img = scan.raw_img.clone();

    let mut real_planner = RealFftPlanner::<f32>::new();
    let r2c = real_planner.plan_fft_forward(n);
    let c2r = real_planner.plan_fft_inverse(n);
    let mut spectrum = r2c.make_output_vec();
    let freq = (0..spectrum.len()).map(|i| i as f32 / rng).collect();
    scan.frequencies = freq;
    scan.r2c = Some(r2c);
    scan.c2r = Some(c2r);
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::data::DataPoint;
    use crate::io::open_from_npy;

    #[test]
    fn open_binary() {
        let path = PathBuf::from("pixel_ID=00000-00000.npy");
        let fft_path = PathBuf::from("pixel_ID=00000-00000_spectrum.npy");
        let mut data = DataPoint::default();

        open_from_npy(&mut data, &path, &fft_path).expect("TODO: panic message");
    }
}
