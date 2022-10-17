use std::cmp::{max, min};
use std::f64::consts::{PI};
use realfft::RealFftPlanner;

use itertools_num::linspace;
use rand::Rng;
use crate::data::{DataContainer};
use crate::data::NUM_PULSE_LINES;

pub struct MovingAverage {
    populated_width: usize,
    pub ready: bool,
    pub width: usize,
    pub time: Vec<Vec<f64>>,
    pub signal_1: Vec<Vec<f64>>,
    pub ref_1: Vec<Vec<f64>>,
}

impl MovingAverage {
    pub fn default(width: usize) -> MovingAverage {
        return MovingAverage {
            populated_width: 0,
            ready: false,
            width: max(width, 1),
            time: vec![vec![0.0; NUM_PULSE_LINES]; width],
            signal_1: vec![vec![0.0; NUM_PULSE_LINES]; width],
            ref_1: vec![vec![0.0; NUM_PULSE_LINES]; width],
        };
    }

    pub fn from_mav(width: usize, old_mav: MovingAverage) -> MovingAverage {
        let mut time = vec![vec![0.0; NUM_PULSE_LINES]; width];
        let mut signal_1 = vec![vec![0.0; NUM_PULSE_LINES]; width];
        let mut ref_1 = vec![vec![0.0; NUM_PULSE_LINES]; width];
        let init_len = min(old_mav.time.len(), width);
        for i in 0..init_len {
            time[i] = old_mav.time[i].clone();
            signal_1[i] = old_mav.signal_1[i].clone();
            ref_1[i] = old_mav.ref_1[i].clone();
        }

        return MovingAverage {
            populated_width: init_len,
            ready: false,
            width: max(width, 1),
            time,
            signal_1,
            ref_1,
        };
    }

    pub fn reset(&mut self) {
        self.populated_width = 0;
        self.ready = false;
    }

    pub fn run(&mut self, data: DataContainer) -> DataContainer {
        let mut mav = DataContainer::default();
        if self.populated_width < self.width {
            self.populated_width += 1;
        } else {
            self.ready = true;
            mav.valid = true;
        }
        // shift all entries
        self.time.rotate_right(1);
        self.signal_1.rotate_right(1);
        self.ref_1.rotate_right(1);

        //self.time[0] = data.time;
        self.signal_1[0] = data.signal_1;
        //self.ref_1[0] = data.ref_1;

        // average out
        for i in 0..self.width {
            for j in 0..NUM_PULSE_LINES {
                // mav.time[j] = mav.time[j] + self.time[i][j];
                mav.signal_1[j] = mav.signal_1[j] + self.signal_1[i][j];
                // mav.ref_1[j] = mav.ref_1[j] + self.ref_1[i][j];
                // mav.ref_2[j] = mav.ref_2[j] + self.ref_2[i][j];
            }
        }
        for j in 0..NUM_PULSE_LINES {
            // mav.time[j] = mav.time[j] / self.populated_width as f64;
            mav.signal_1[j] = mav.signal_1[j] / self.populated_width as f64;
            // mav.ref_1[j] = mav.ref_1[j] / self.populated_width as f64;
        }
        // only average the signals, not the references!
        mav.time = data.time.iter().map(|x| (x * 1000.0).round() / 1000.0).collect();
        mav.ref_1 = data.ref_1;

        mav.frequencies_fft = data.frequencies_fft;
        mav.ref_1_fft = data.ref_1_fft;
        mav.ref_phase_1_fft = data.ref_phase_1_fft;
        mav.signal_1_fft = data.signal_1_fft;
        mav.phase_1_fft = data.phase_1_fft;
        mav
    }
}

pub fn generate_dummy_pulse(t: &[f64]) -> Vec<f64> {
    let mut offset: f64 = 1.0;
    let noise = rand::thread_rng().gen_range(-0.02..0.02);
    offset = offset + noise;
    let mut pulse: Vec<f64> = vec![0.0; t.len()];
    let t_internal: Vec<f64> = linspace::<f64>(-4.0, 4.0, t.len()).collect();
    let noise_rng: Vec<f64> = (0..t.len()).map(|_| rand::thread_rng().gen_range(0.0..0.02)).collect();
    for i in 0..t_internal.len() {
        if t_internal[i] > 2.0 {
            pulse[i] = 0.0;
        } else if t_internal[i] < -1.5 {
            pulse[i] = 0.0
        } else {
            pulse[i] = (t_internal[i] + offset).sin().powi(30) * (t_internal[i] + offset + 1.5).sin().powi(3);
        }
        pulse[i] = 10.0 * pulse[i] + noise_rng[i];
    }
    return pulse;
}

fn blackman_window(n: f64, m: f64) -> f64 {
    // blackman window as implemented by numpy (python)
    let res = 0.42 - 0.5 * (2.0 * PI * n / m).cos() + 0.08 * (4.0 * PI * n / m).cos();
    return if res.is_nan() {
        1.0
    } else if res < 0.0 {
        0.0
    } else if res > 1.0 {
        1.0
    } else {
        res
    };
}

pub fn apply_fft_window(signal: &mut [f64], time: &[f64], lower_bound: &f64, upper_bound: &f64) {
    for (s, t) in signal.iter_mut().zip(time.iter()) {
        if *t <= lower_bound + time[0] {
            // first half of blackman
            let bw = blackman_window(t - time[0], 2.0 * lower_bound);
            *s *= bw;
        } else if *t >= time[time.len() - 1] - upper_bound {
            // second half of blackman
            let bw = blackman_window(t - (time[time.len() - 1] - upper_bound * 2.0), 2.0 * upper_bound);
            *s *= bw;
        }
    }
}

pub fn make_fft(t_in: &[f64], p_in: &[f64], normalize: bool, df: &f64,
                lower_bound: &f64, upper_bound: &f64) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    // make a planner
    let mut real_planner = RealFftPlanner::<f64>::new();

    // implement zero padding!
    let dt = t_in[1] - t_in[0];
    let zero_padding = (1.0 / (*df * dt)) as usize;
    let padding_length;
    let mut t = t_in.to_vec();
    let mut p = p_in.to_vec();

    // apply fft window (modified blackman window as specified by Toptica)
    apply_fft_window(&mut p, &t, lower_bound, upper_bound);

    if zero_padding > t.len() {
        padding_length = zero_padding - t.len();
        let t_padded_bound = t[0] + dt * (zero_padding as f64);
        t = linspace::<f64>(t[0], t_padded_bound, zero_padding).collect();
        p.append(&mut vec![0.0; padding_length]);
    }


    // create a FFT
    let r2c = real_planner.plan_fft_forward(t.len());
    // make input and output vectors
    let mut in_data: Vec<f64> = p.iter().map(|x| *x as f64).collect();
    let mut spectrum = r2c.make_output_vec();
    // Forward transform the input data
    r2c.process(&mut in_data, &mut spectrum).unwrap();

    let mut amp: Vec<f64> = spectrum.iter().map(|s| s.norm()).collect();
    let rng = t[t.len() - 1] - t[0];
    let freq: Vec<f64> = (0..spectrum.len()).map(|i| i as f64 / rng).collect();
    let phase: Vec<f64> = spectrum.iter().map(|s| s.arg()).collect();
    if normalize {
        let max_amp = amp.iter().fold(f64::NEG_INFINITY, |ai, &bi| ai.max(bi));
        amp = amp.iter().map(|a| *a / max_amp).collect();
    }
    (freq, amp, phase)
}


#[cfg(test)]
mod tests {
    use crate::teraflash::NUM_PULSE_LINES;
    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;

    #[test]
    fn test_mav() {
        let mut mav = MovingAverage::default(2);
        let mut data = DataContainer::default();
        data = mav.run(data);
        println!("1: {:?}", data.signal_1);
        data.signal_1 = vec![1.0; NUM_PULSE_LINES];
        data = mav.run(data);
        println!("2: {:?}", data.signal_1);
        data.signal_1 = vec![1.0; NUM_PULSE_LINES];
        data = mav.run(data);
        println!("3: {:?}", data.signal_1);
    }
}