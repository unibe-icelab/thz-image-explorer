# THz Image Explorer - An Interactive Cross-Platform Open-Source THz Image Analysis Tool

<a href="https://github.com/unibe-icelab/thz-image-explorer/releases"><img src="assets/icons/icon.png" alt=“” width="100" height="100"> </img> </a>

This Application is written in Rust and the GUI is based on egui and bevy.

<p align="center">
  <img src="paper/screenshot.png" alt="Screenshot of the application on macOS" width="400">
  <img src="thz-image-explorer-gif-3d.gif" alt="3D render gif" width="400">
</p>

A sample scan of a THz time-domain 2D imaging system can be found in the `sample_data` directory.

---

## Table of Contents

- [Features](#features)
- [Installation](#installation)
  - [Download Pre-built Executables](#download-pre-built-executables)
  - [Compile from Source](#compile-from-source)
- [Usage](#usage)
- [Custom Filters](#custom-filters)
- [PSF File Generation](#psf-file-generation)
- [Tested Platforms](#tested-platforms)

---

## Features

- Cross-platform (Linux, macOS, Windows)
- Interactive 2D and 3D visualization
- Meta data editing
- Region of Interest (ROI) selection and comparison
- Reference dataset support
- Extensible with custom filters

---

## Installation

### Download Pre-built Executables

[Binary bundles](https://github.com/unibe-icelab/thz-image-explorer/releases) are available for Linux, macOS, and Windows.

#### macOS

If you see the message `"THz Image Explorer is damaged and cannot be opened."`, run:
```sh
xattr -rd com.apple.quarantine THz\ Image\ Explorer.app
```

#### Linux

Install dependencies:
```sh
sudo apt-get install libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libxkbcommon-dev libssl-dev libasound2-dev
```

---

### Compile from Source

1. Install Rust and Cargo.
2. Install dependencies (Linux only):
   ```sh
   sudo apt-get install libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libxkbcommon-dev libssl-dev libasound2-dev
   ```
3. Install bundling tools:
   - Linux/macOS: `cargo install cargo-bundle`
   - Windows: `cargo install cargo-wix`
4. Build:
   - Linux/macOS: `cargo bundle --release`
   - Windows: `cargo wix -p thz-image-explorer`

> **Note:** [cargo-bundle](https://github.com/burtonageo/cargo-bundle) only supports Linux and macOS. For Windows, use [cargo-wix](https://github.com/volks73/cargo-wix).

#### macOS Plugin

The `DotTHzQLExtension.appex` plugin is installed in `THz Image Explorer.app/Contents/PlugIns`.  
Source: [DotTHzQL](https://github.com/hacknus/DotTHzQL)  
Requires HDF5 to be installed system-wide.

---

## Usage

Example data is available in the `sample_data` directory of this repository.

- For files larger than 100 x 100 x 2000 datapoints, use **downscaling** for better responsiveness.
- Edit `.thz` file meta-data in the Meta Data Editor:

  <p align="center">
    <img src="paper/metadata.png" alt="Meta Data Editor" width="250">
  </p>

- Select a region of interest (ROI) in the 2D view:

  <p align="center">
    <img src="paper/roi.png" alt="ROI Selection" width="250">
  </p>

- 3D view supports rotation and zoom:

  <p align="center">
    <img src="paper/3d_scan.png" alt="3D View of a scan" width="400">
  </p>

- Load a reference dataset and select a reference pulse for optical property calculations, or compare different ROIs.

---

## Custom Filters

Extend the codebase with custom filters:

1. Create a file in `src/filters` with a struct implementing the `Filter` trait.
2. Attach the file to `mod.rs` in `src/filters`.
3. Implement the required functions (`config`, `filter`, `ui`, etc.).
4. Derive `Clone`, `Debug`, and `CopyStaticFields`.
5. Add the `#[register_filter]` macro to your struct.

Loops requiring heavy computation can be parallelized using the `rayon` crate. Use the `cancellable_loops` crate for abortable computations.

**Example:**
```rust
use crate::filters::filter::Filter;
use crate::data_container::ScannedImageFilterData;
use crate::gui::application::GuiSettingsContainer;

#[register_filter]
#[derive(Clone, Debug, CopyStaticFields)]
struct ExampleFilter;

impl Filter for ExampleFilter {
    fn new() -> Self { ExampleFilter }

    fn reset(&mut self, time: &Array1<f32>, shape: &[usize]) {
        // Reset any internal state if necessary
    }

    fn show_data(&mut self, data: &ScannedImageFilterData) {
        // Display any data in the GUI if needed
    }

    fn filter(
        &mut self,
        input_data: &ScannedImageFilterData,
        gui_settings: &mut GuiSettingsContainer,
        progress_lock: &mut Arc<RwLock<Option<f32>>>,
        abort_flag: &Arc<AtomicBool>,
    ) -> ScannedImageFilterData {
        // Apply filter logic here
        input_data.clone() // Placeholder, replace with actual filtering logic
    }


    fn config(&self) -> FilterConfig {
        FilterConfig {
            name: "Example Filter".to_string(),
            description: "Description of the example filter.".to_string(),
            hyperlink: None, // Optional DOI or reference link
            domain: FilterDomain::TimeBeforeFFT, // Specify the domain of the filter
        }
    }

    fn ui(
        &mut self,
        ui: &mut egui::Ui,
        thread_communication: &mut ThreadCommunication,
        panel_width: f32,
    ) -> egui::Response {
        // Render filter configuration UI here
    }
}
```

---

## PSF File Generation

1. Perform a knife-edge scan and create a `.thz` file (see `sample_data/example_beam_width/`).
2. Run:
   ```sh
   python scripts/generate_psf.py \
     --path_x sample_data/example_beam_width/measurement_x/data/1750085285.8557956_data.thz \
     --path_y sample_data/example_beam_width/measurement_y/data/1750163177.929295_data.thz
   ```
   This generates a `psf.npz` file for deblurring in THz Image Explorer.

---

## Tested Platforms

- macOS 12.4 Monterey x86
- macOS 10.14.6 Mojave x86
- Debian 12 (Testing) x86
- Windows 10 x86
