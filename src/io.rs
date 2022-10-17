use std::error::Error;
use csv::{ReaderBuilder, WriterBuilder};
use crate::data::DataContainer;


pub fn open_from_csv(data: &mut DataContainer, file_path: &String, file_path_fft: &String) -> Result<(), Box<dyn Error>> {
    data.time = vec![];
    data.signal_1 = vec![];
    data.ref_1 = vec![];

    data.frequencies_fft = vec![];
    data.signal_1_fft = vec![];
    data.phase_1_fft = vec![];
    data.ref_1_fft = vec![];
    data.ref_phase_1_fft = vec![];

    let mut rdr = ReaderBuilder::new()
        .has_headers(true)
        .from_path(file_path)?;

    for result in rdr.records() {
        let row = result?;
        data.time.push(row[0].parse::<f64>().unwrap());
        data.signal_1.push(row[1].parse::<f64>().unwrap());
        data.ref_1.push(row[2].parse::<f64>().unwrap());
    }

    let mut rdr = ReaderBuilder::new()
        .has_headers(true)
        .from_path(file_path_fft)?;

    for result in rdr.records() {
        let row = result?;
        data.frequencies_fft.push(row[0].parse::<f64>().unwrap() / 1000.0);
        data.signal_1_fft.push(row[1].parse::<f64>().unwrap());
        data.phase_1_fft.push(row[2].parse::<f64>().unwrap());
        data.ref_1_fft.push(row[3].parse::<f64>().unwrap());
        data.ref_phase_1_fft.push(row[4].parse::<f64>().unwrap());
    }
    Ok(())
}

pub fn save_to_csv(data: &DataContainer, file_path: &String, file_path_fft: &String) -> Result<(), Box<dyn Error>> {
    let mut wtr = WriterBuilder::new()
        .has_headers(false)
        .from_path(file_path)?;
    // serialize does not work, so we do it with a loop..
    wtr.write_record(&["Time_abs/ps", " Signal 1/nA", " Reference 1/nA"])?;
    for i in 0..data.time.len() {
        wtr.write_record(&[
            data.time[i].to_string(),
            data.signal_1[i].to_string(),
            data.ref_1[i].to_string(),
        ])?;
    }
    wtr.flush()?;

    let mut wtr = WriterBuilder::new()
        .has_headers(false)
        .from_path(file_path_fft)?;
    // serialize does not work, so we do it with a loop..
    wtr.write_record(&["Frequency/GHz", " Amplitude rel. 1", " Phase 1", " Ref.Amplitude rel. 1", " Ref.Phase 1"])?;
    for i in 0..data.frequencies_fft.len() {
        wtr.write_record(&[
            (data.frequencies_fft[i] * 1_000.0).round().to_string(),
            data.signal_1_fft[i].to_string(),
            data.phase_1_fft[i].to_string(),
            data.ref_1_fft[i].to_string(),
            data.ref_phase_1_fft[i].to_string(),
        ])?;
    }
    wtr.flush()?;

    Ok(())
}