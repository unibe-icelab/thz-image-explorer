use anyhow::Result;
use dotthz::DotthzFile;
use ndarray::Array2;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Extract position value from group name
/// Example: "Beam Width Measurement x=-0.10" -> -0.10
fn extract_position_from_group_name(group_name: &str) -> Result<f64> {
    if let Some(idx) = group_name.find('=') {
        let after_equals = &group_name[idx + 1..];
        let num_str: String = after_equals
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-' || *c == '+')
            .collect();
        num_str
            .parse::<f64>()
            .map_err(|e| anyhow::anyhow!("Failed to parse position from '{}': {}", group_name, e))
    } else {
        anyhow::bail!("No '=' found in group name: {}", group_name)
    }
}

/// Represents a knife-edge measurement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnifeEdgeMeasurement {
    pub positions: Vec<f64>,
    pub time_traces: Array2<f64>,
    pub times: Vec<f64>,
}

impl KnifeEdgeMeasurement {
    /// Load knife-edge measurements from a .thz file.
    /// Each group = one spatial position; group name encodes the position via "x=<val>" or "y=<val>".
    pub fn from_thz_file(path: &Path) -> Result<Self> {
        let path_buf = path.to_path_buf();
        let file = DotthzFile::open(&path_buf)
            .map_err(|e| anyhow::anyhow!("Failed to open .thz file {}: {}", path.display(), e))?;

        let group_names = file
            .get_group_names()
            .map_err(|e| anyhow::anyhow!("Failed to get group names: {}", e))?;

        if group_names.is_empty() {
            anyhow::bail!("No groups found in .thz file");
        }

        let mut positions = Vec::new();
        let mut all_traces = Vec::new();
        let mut times: Option<Vec<f64>> = None;

        for group_name in &group_names {
            let position = match extract_position_from_group_name(group_name) {
                Ok(p) => p,
                Err(_) => {
                    log::warn!("Skipping group '{}' — could not parse position", group_name);
                    continue;
                }
            };

            // Use dotthz 0.3 API: get_datasets(group_name) instead of group.datasets()
            let datasets = file.get_datasets(group_name).map_err(|e| {
                anyhow::anyhow!("Failed to get datasets from {}: {}", group_name, e)
            })?;

            if datasets.is_empty() {
                log::warn!("Skipping group {} - no datasets", group_name);
                continue;
            }

            // First dataset should be 2D with [time, signal] columns
            let arr_2d: Array2<f32> = datasets[0].read_2d().map_err(|e| {
                anyhow::anyhow!(
                    "Failed to read dataset as 2D array in {}: {}",
                    group_name,
                    e
                )
            })?;

            let time_arr = arr_2d.column(0).to_owned();
            let signal_arr = arr_2d.column(1).to_owned();

            if times.is_none() {
                times = Some(time_arr.iter().map(|&x| x as f64).collect());
            }

            positions.push(position);
            all_traces.push(signal_arr.iter().map(|&x| x as f64).collect::<Vec<f64>>());
        }

        let times = times.ok_or_else(|| anyhow::anyhow!("No valid time data found"))?;

        if positions.is_empty() {
            anyhow::bail!("No valid position data found in any group");
        }

        let n_positions = positions.len();
        let n_time_points = times.len();
        let mut time_traces = Array2::zeros((n_positions, n_time_points));
        for (i, trace) in all_traces.iter().enumerate() {
            for (j, &val) in trace.iter().enumerate() {
                time_traces[[i, j]] = val;
            }
        }

        // Sort by position
        let mut indices: Vec<usize> = (0..n_positions).collect();
        indices.sort_by(|&a, &b| positions[a].partial_cmp(&positions[b]).unwrap());

        let sorted_positions: Vec<f64> = indices.iter().map(|&i| positions[i]).collect();
        let mut sorted_traces = Array2::zeros((n_positions, n_time_points));
        for (new_idx, &old_idx) in indices.iter().enumerate() {
            sorted_traces
                .row_mut(new_idx)
                .assign(&time_traces.row(old_idx));
        }

        Ok(Self {
            positions: sorted_positions,
            time_traces: sorted_traces,
            times,
        })
    }
}

/// Split measurement in half for double knife-edge processing.
/// Returns (left_half_flipped, right_half).
pub fn split_and_flip_measurement(
    meas: &KnifeEdgeMeasurement,
) -> (KnifeEdgeMeasurement, KnifeEdgeMeasurement) {
    let n_total = meas.positions.len();
    let n_half = n_total / 2;

    let left_positions: Vec<f64> = meas.positions[..n_half].to_vec();
    let right_positions: Vec<f64> = meas.positions[n_half..].to_vec();

    let left_traces = meas.time_traces.slice(ndarray::s![..n_half, ..]).to_owned();
    let right_traces = meas.time_traces.slice(ndarray::s![n_half.., ..]).to_owned();

    let mut flipped_left_positions: Vec<f64> = left_positions.iter().map(|&x| -x).collect();
    flipped_left_positions.reverse();

    let mut flipped_left_traces = Array2::zeros(left_traces.dim());
    for (i, row) in left_traces.rows().into_iter().enumerate() {
        let target_idx = n_half - 1 - i;
        flipped_left_traces.row_mut(target_idx).assign(&row);
    }

    let left_meas = KnifeEdgeMeasurement {
        positions: flipped_left_positions,
        time_traces: flipped_left_traces,
        times: meas.times.clone(),
    };

    let right_meas = KnifeEdgeMeasurement {
        positions: right_positions,
        time_traces: right_traces,
        times: meas.times.clone(),
    };

    (left_meas, right_meas)
}

/// Load both X and Y knife-edge measurements.
pub fn load_knife_edge_measurements(
    x_path: &Path,
    y_path: &Path,
) -> Result<(KnifeEdgeMeasurement, KnifeEdgeMeasurement)> {
    let x_data = KnifeEdgeMeasurement::from_thz_file(x_path)?;
    let y_data = KnifeEdgeMeasurement::from_thz_file(y_path)?;
    Ok((x_data, y_data))
}
