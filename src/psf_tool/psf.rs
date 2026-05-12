use ndarray::Array2;
use serde::{Deserialize, Serialize};

/// PSF data structure compatible with Python version
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PsfData {
    pub low_cut: f64,
    pub high_cut: f64,
    pub start_freq: f64,
    pub end_freq: f64,
    pub n_filters: usize,
    pub filters: Array2<f64>,
    pub filt_freqs: Vec<f64>,
    pub x_params: Array2<f64>, // [x_0, w_x] for each frequency
    pub y_params: Array2<f64>, // [y_0, w_y] for each frequency
}

impl PsfData {
    /// Save PSF data to a numpy-compatible .npz file
    #[allow(dead_code)]
    pub fn save_npz(&self, path: &std::path::Path) -> anyhow::Result<()> {
        // For now, just save as JSON since npz requires specialized library
        // In production, you'd use ndarray-npy crate
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path.with_extension("json"), json)?;
        Ok(())
    }

    /// Load PSF data from a JSON file
    #[allow(dead_code)]
    pub fn load_json(path: &std::path::Path) -> anyhow::Result<Self> {
        let json = std::fs::read_to_string(path)?;
        let data = serde_json::from_str(&json)?;
        Ok(data)
    }
}
