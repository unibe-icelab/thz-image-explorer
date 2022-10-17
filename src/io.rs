use std::error::Error;
use csv::{ReaderBuilder, WriterBuilder};
use crate::data::{DataContainer, HouseKeeping};



pub fn open_hk(hk: &mut HouseKeeping, file_path: String) -> Result<(usize, usize), Box<dyn Error>> {
    let mut rdr = ReaderBuilder::new()
        .delimiter(b',')
        .has_headers(true)
        .from_path(file_path)?;
    // data
    let mut x = 0;
    let mut y = 0;
    if let Some(result) = rdr.records().next() {
        let record = result?;
        x = record[1].parse::<usize>().unwrap();
        y = record[2].parse::<usize>().unwrap();
        hk.ambient_temperature = record[3].parse::<f64>().unwrap();
        hk.sample_temperature = record[4].parse::<f64>().unwrap();
        hk.ambient_pressure = record[5].parse::<f64>().unwrap();
        hk.ambient_humidity = record[6].parse::<f64>().unwrap();
    }
    Ok((x, y))
}

pub fn open_conf(hk: &mut HouseKeeping, file_path: String) -> Result<(usize, usize), Box<dyn Error>> {
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
        hk.dx = record[3].parse::<f64>().unwrap();
        hk.x_range[0] = record[4].parse::<f64>().unwrap();
        hk.x_range[1] = record[5].parse::<f64>().unwrap();
        hk.dy = record[6].parse::<f64>().unwrap();
        hk.y_range[0] = record[7].parse::<f64>().unwrap();
        hk.y_range[1] = record[8].parse::<f64>().unwrap();
        hk.ambient_temperature = record[9].parse::<f64>().unwrap();
        hk.sample_temperature = record[10].parse::<f64>().unwrap();
        hk.ambient_pressure = record[11].parse::<f64>().unwrap();
        hk.ambient_humidity = record[12].parse::<f64>().unwrap();
    }
    Ok((width, height))
}


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