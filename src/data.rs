use std::time::Instant;

use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Meta {
    pub timestamp: f64,
    pub width: usize,
    pub height: usize,
    pub dx: f32,
    pub x_min: f32,
    pub x_max: f32,
    pub dy: f32,
    pub y_min: f32,
    pub y_max: f32,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct HouseKeeping {
    pub dx: f32,
    pub x_range: [f32; 2],
    pub dy: f32,
    pub y_range: [f32; 2],
    pub t_begin: f32,
    pub range: f32,
    pub ambient_temperature: f64,
    pub ambient_pressure: f64,
    pub ambient_humidity: f64,
    pub sample_temperature: f64,
}

impl Default for HouseKeeping {
    fn default() -> Self {
        Self {
            dx: 1.0,
            x_range: [0.0, 10.0],
            dy: 1.0,
            y_range: [0.0, 10.0],
            t_begin: 1000.0,
            range: 50.0,
            ambient_temperature: 22.0,
            ambient_pressure: 950.0,
            ambient_humidity: 50.0,
            sample_temperature: 0.0,
        }
    }
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct DataPoint {
    pub hk: HouseKeeping,
    pub cut_off: f64,
    pub valid: bool,
    pub time: Vec<f32>,
    pub signal_1: Vec<f32>,
    pub filtered_signal_1: Vec<f32>,
    pub ref_1: Vec<f32>,
    pub frequencies_fft: Vec<f32>,
    pub signal_1_fft: Vec<f32>,
    pub phase_1_fft: Vec<f32>,
    pub filtered_signal_1_fft: Vec<f32>,
    pub filtered_phase_1_fft: Vec<f32>,
    pub ref_1_fft: Vec<f32>,
    pub ref_phase_1_fft: Vec<f32>,
}

#[derive(Clone)]
pub struct ScannedImage {
    pub path: String,
    pub x_min: f32,
    pub dx: f32,
    pub y_min: f32,
    pub dy: f32,
    pub height: usize,
    pub width: usize,
    pub img: Vec<f32>,
    pub data: Vec<DataPoint>,
    // do we need the ids?
}

impl Default for ScannedImage {
    fn default() -> Self {
        ScannedImage {
            path: "".to_string(),
            x_min: 0.0,
            dx: 0.0,
            y_min: 0.0,
            dy: 0.0,
            height: 0,
            width: 0,
            img: vec![],
            data: vec![],
        }
    }
}

impl ScannedImage {
    pub fn new(
        height: usize,
        width: usize,
        x_min: f32,
        y_min: f32,
        dx: f32,
        dy: f32,
    ) -> ScannedImage {
        ScannedImage {
            path: "test".to_string(),
            x_min,
            dx,
            y_min,
            dy,
            height,
            width,
            img: vec![0.0; width * height],
            data: vec![DataPoint::default(); width * height],
        }
    }
    pub fn get_data(&mut self, x: usize, y: usize) -> DataPoint {
        self.data[x * self.width + y].clone()
    }

    pub fn set_data(&mut self, x: usize, y: usize, data: DataPoint) {
        self.data[x * self.width + y] = data;
    }

    pub fn get_img(&mut self, x: usize, y: usize) -> f32 {
        self.img[x * self.width + y]
    }

    pub fn set_img(&mut self, x: usize, y: usize, val: f32) {
        self.img[x * self.width + y] = val;
    }
}

impl ScannedImage {
    fn iter(
        &self,
    ) -> ScannedImageIter<impl Iterator<Item = &DataPoint>, impl Iterator<Item = &f32>> {
        ScannedImageIter {
            data: self.data.iter(),
            img: self.img.iter(),
        }
    }
    fn iter_mut(
        &mut self,
    ) -> ScannedImageIterMut<impl Iterator<Item = &mut DataPoint>, impl Iterator<Item = &mut f32>>
    {
        ScannedImageIterMut {
            data: self.data.iter_mut(),
            img: self.img.iter_mut(),
        }
    }
}

struct ScannedImageIter<D, I> {
    data: D,
    img: I,
}

impl<'r, D, I> Iterator for ScannedImageIter<D, I>
where
    D: Iterator<Item = &'r DataPoint>,
    I: Iterator<Item = &'r f32>,
{
    type Item = (&'r DataPoint, &'r f32);

    fn next(&mut self) -> Option<Self::Item> {
        match self.data.next() {
            Some(d) => self.img.next().map(|i| (d, i)),
            None => None,
        }
    }
}

struct ScannedImageIterMut<D, I> {
    data: D,
    img: I,
}

impl<'r, D, I> Iterator for ScannedImageIterMut<D, I>
where
    D: Iterator<Item = &'r mut DataPoint>,
    I: Iterator<Item = &'r mut f32>,
{
    type Item = (&'r mut DataPoint, &'r mut f32);

    fn next(&mut self) -> Option<Self::Item> {
        match self.data.next() {
            Some(d) => self.img.next().map(|i| (d, i)),
            None => None,
        }
    }
}
