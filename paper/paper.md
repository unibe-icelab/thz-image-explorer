---
title: "THz Image Explorer - An Interactive Cross-Platform Open-Source THz Image Analysis Tool"
tags:
  - THz
  - Rust
  - Imaging
  - Data Analysis
authors:
  - name: Linus Leo Stöckli
    orcid: 0000-0002-7916-2592
    corresponding: true
    affiliation: "1"
  - name: Arnaud Demion
    orcid: 0009-0002-6920-475X
    affiliation: "2"
  - name: Nicolas Thomas
    orcid: 0000-0002-0146-0071
    affiliation: "1"
affiliations:
  - name: Space Research & Planetary Sciences Division, University of Bern, Bern, Switzerland
    index: 1
    ror: 02k7v4d05
  - name: University of Applied Sciences and Arts Western Switzerland Valais, HES-SO Valais-Wallis, Sion, Switzerland
    index: 2
    ror: 03r5zec51
date: 3 July 2025
bibliography: paper.bib
---

# Introduction

THz time-domain spectroscopy (TDS) is a fast-growing field with applications to perform non-destructive studies of
material properties [@neu_tutorial_2018].
Different sources of THz radiations have been implemented in commercial products, e.g. photo-conductive antennas. The
pulses can either be measured after passing through (transmission spectrum) or after being reflected (reflection
spectrum) by a sample and are recorded in the time domain. By transforming the
acquired trace into frequency domain (Fourier space), the magnitude and phase can be extracted to
investigate the complex refractive index and absorption coefficient of the sample.
By placing either the sample or the optical setup on a moving stage the sample can be imaged in 2D. Analysing these
images pixel by pixel or by selecting a region of interest (ROI) without an interactive user interface can be tedious.

![THz Image Explorer icon.\label{fig:icon}](icon.png){#id .class width=20%}

We developed an interactive graphical user interface (GUI), written
in [Rust](https://www.rust-lang.org) [@matsakis2014rust], to aid
investigating acquired 2D scans. The
application implements the dotTHz standard [@lee_dotthz_2023] and is platform independent and open-source, thus making
it
easier to maintain and increasing its reach.

![THz Image Explorer screenshot.\label{fig:screenshot}](screenshot.png)

# Statement of need

Interactive analysis tools for THz spectroscopy are essential to browse through images and analyse different regions of
interest efficiently.
Commercial suppliers provide closed-source analysis tools (e.g. [Menlo Systems](https://www.menlosystems.com)) where the
code cannot be adapted by the user, which is often essential in research environments and extends the maintainability of
the code.
Solutions published by the scientific community are not available on all platforms, are only applicable on single pixel
measurements and/or not focused on an interactive workflow [@peretti_thz-tds_2019; @loaiza_thztools_2024].
With this application, we provide a performant solution written in Rust, that allows an interactive analysis of 2D THz
scans with multiple filters and a 3D viewer.
This work is open-source and pre-built bundles are available for Linux, macOS and Windows, thus available to the
entire scientific community.

# Structure

The application is multithreaded with two main threads:

- GUI thread
- Data thread

The GUI uses [egui](https://www.egui.rs), an immediate-mode GUI library for rust
and [bevy](https://bevy.org) [@bevyengine], a game
engine used for rendering.

The GUI thread handles all the user input and displaying of plots and other window elements. The configuration values
set in the GUI are sent to the Data thread
via multiple-producer-multi-consumer (MPMC) channels.
The Data thread then handles the computation of the applied filters.
The output of the computation is then shared via mutexes with the GUI thread.
The entire thread communication is handled with the `ThreadCommunication` struct which holds many `Arc<RwLock<T>>` or
`crossbeam_channel::Sender<T>`/`crossbeam_channel::Receiver<T>`.

The structure of the software architecture is shown in figure \ref{fig:software-architecture}.

![Software Architecture.\label{fig:software-architecture}](thz-image-explorer.drawio.png){width=80% .center}

For each filter, an entry in the `filter_data_pipeline` vector is created, which contains the dataset. Each filter is
assigned an input and output index. This is memory intensive, but for the size of usual THz TDS datasets (tens to
hundreds of MB) it is acceptable. This structure defines the processing pipeline in a clean and easily extendable way.

# Installation

## Pre-built Bundles

Pre-built bundles are available for each release on [GitHub](https://github.com/unibe-icelab/thz-image-explorer) for

- macOS (`.app` bundle for x86 and Apple Silicon)
- Linux (executable and `.deb` for x86)
- Windows (`.exe` and `.msi` for x86)

These bundles should work out of the box, but on macOS you might need to remove the quarantine flag after downloading by
running the following command:

```shell
xattr -rd com.apple.quarantine THz\ Image\ Explorer.app
```

On macOS the `DotTHzQLExtension.appex` Plugin will automatically be installed in the
`THz Image Explorer.app/Contents/PlugIns` directory. The source code of the plugin can be
found [here](https://github.com/hacknus/DotTHzQL). Note: This plugin requires HDF5 to be installed system-wide.

## Compile from Source

Alternatively, to compile directly from source, rust 1.87 or higher needs to be installed and the following
command needs to be executed:

```shell
cargo run --release
```

or to only build the executable without running:

```shell
cargo build --release
```

With default settings `cmake` is required to build HDF5 from source, which is required for the implementation of the
dotTHz standard. If HDF5 is already installed on the
system, the user can change remove the `hdf5-sys-static` feature from the `dotthz` dependency in the `Cargo.toml` file.

On Linux, the following dependencies need to be installed first as a requirement for `egui` and `bevy`:

- `libclang-dev`
- `libgtk-3-dev`
- `libxcb-render0-dev`
- `libxcb-shape0-dev`
- `libxcb-xfixes0-dev`
- `libxkbcommon-dev`
- `libssl-dev`
- `libasound2-dev`

On Linux you need to first run:

```shell
sudo apt-get install -y libclang-dev libgtk-3-dev \
  libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev \
  libxkbcommon-dev libssl-dev libasound2-dev
```

To create bundles `cargo-bundle` needs to be installed (macOS, Linux):

```shell
cargo bundle --release
```

or `cargo-wix` on Windows:

```shell
cargo wix -p thz-image-explorer
```

An update feature, which will download the latest release and upgrade the installed application, is implemented in the
settings window.

# Usage

The window is structured with the time domain trace and frequency domain spectrum for the selected pixel (default is
0,0) in the center panel. A different tab showing the optical properties (refractive index and absorption coefficient)
can be selected, as
well as a tab containing an interactive 3D viewer.
The left side-panel contains the intensity plot of the 2D scan along with the meta-data editor. The right
side-panel contains the possible filters with configuration settings.
A pixel can be (de-)selected by clicking inside the intensity plot.
For large scans, it is recommended to down-scale the image. This will average the pixel values in a $2 \times 2$
(or $4 \times 4$ and so on) pixel block, depending on the down-scaling factor. Note that the "Signal" trace in
time-domain will still show the raw trace, but the "Signal" spectrum in the FFT plot will show the averaged spectrum of
the down-scaled image.

A sample scan (of a resolution target) is available in the `sample_data` directory. The measurement has been acquired
using the COCoNuT setup [@coconut_2025].

## IO

THz Image Explorer is able to load scans in the `.thz` (dotTHz) format, which are based on the HDF5 standard.
This allows the files to also contain meta-data, which will also be displayed by the THz Image Explorer. The meta-data
is shown in the file opening dialog on Linux and Windows, and using QuickView on macOS, allowing to easily
browse through directories containing multiple scans.

THz Image Explorer supports drag & drop of `.thz`, `.thzimg` and `.thzswp` files.

The 3D structure can be exported as a `.vtu` file for further analysis (e.g.
with [ParaView](https://www.paraview.org) ).

A reference file (standard `.thz`) can be loaded, which is used to compute the optical properties of the sample. The
first
entry will be used.

## Regions of Interest (ROI)

By holding the Shift key and selecting pixels, a
region of interest (ROI) can be selected. This ROI is a convex polygon, which is closed if the last corner is selected
reasonably close to the first one (< 5 % of width/height of the image). This ROI can then be saved in the meta-data of
the dotTHz file for future analysis. The full averaged scan as well as the averages of all selected ROIs can be
displayed in the center plot.

## Meta Data Editor

The meta-data editor allows the user to edit the meta-data of the loaded scan. The meta-data is stored in the `.thz`
file. Certain fields are mandatory as per the dotTHz standard [@lee_dotthz_2023], and cannot be deleted. But any further
attributes can be added, modified and deleted.


## Optical Properties

The optical properties can be computed from the frequency domain spectrum using the following
relations [@Jepsen2019]:

### Refractive Index

The refractive index $n(\omega)$ is calculated from the phase difference between sample and reference:

$$n(\omega) = 1 + \frac{c \Delta\phi(\omega)}{\omega d}$$

where:

- $\Delta\phi(\omega) = \phi_{\text{sample}}(\omega) - \phi_{\text{reference}}(\omega)$ is the phase difference
- $c$ is the speed of light
- $\omega = 2\pi f$ is the angular frequency
- $d$ is the sample thickness

### Absorption Coefficient

The absorption coefficient $\alpha(\omega)$ is derived from the amplitude ratio:

$$\alpha(\omega) = -\frac{2}{d} \ln\left(\frac{(n+1)^2}{4n} \cdot \frac{A_{\text{sample}}(\omega)}{A_{\text{reference}}(\omega)}\right)$$

where $A_{\text{sample}}(\omega)$ and $A_{\text{reference}}(\omega)$ are the amplitude spectra of the sample and
reference measurements, respectively.

### Extinction Coefficient

The extinction coefficient $\kappa(\omega)$ is related to the absorption coefficient by:

$$\kappa(\omega) = \frac{\alpha(\omega) c}{4\pi f}$$

The refractive index and absorption coefficient are computed for the selected source and selected reference. The user
can select a pixel or a ROI in the 2D plot to display the refractive index and absorption coefficient for that pixel in
the center plot.

## Interactive 3D Viewer

A THz time domain scan produces a 3D data array with dimensions $n_x \times n_y \times n_t$, where $(n_x, n_y)$
represent the spatial coordinates and $n_t$ represents the time axis.

Scans performed in reflection can be visualized in 3D. First, we transform each time trace into an intensity value by
computing
the squared amplitude and
applying a Gaussian envelope function:
$$
I(x,y,t) = |s(x,y,t)|^2 * G_{\sigma}(t)
$$

where $G_{\sigma}(t)$ is a normalized 1D Gaussian kernel with standard deviation $\sigma = 6.0$ and radius of 12
samples,
applied via convolution to smooth the squared signal and extract the envelope as shon in figure \ref{fig:envelope}.

![The convoluted envelope of the signal. All datapoints below the indicated threshold are treated as transparent. \label{fig:envelope}](convolution_example.pdf)

The time axis is converted to a spatial distance coordinate by assuming a refractive index of $n=1$ and using the
relation $z = ct/2$, where $c$ is the speed of light and the factor of 2 accounts for the round-trip propagation. This
transformation yields a three-dimensional intensity cube $I(x,y,z)$.

Each element (voxel) in this cube represents the THz signal intensity at a specific point in 3D space, enabling
visualization of reflections from internal interfaces and sub-surface structures. The computed intensities are mapped to
voxel opacity values - regions with high intensity appear opaque while low-intensity regions become transparent.

Users can adjust the opacity threshold to optimize rendering performance and reduce visual noise from weak signals. The
3D viewer is implemented using the `bevy` game engine with a custom WGSL shader, available as a separate
crate: [bevy_voxel_plot](https://github.com/hacknus/bevy_voxel_plot).

## Filtering pipeline

The filtering process is a simple linear pipeline, where the output of one filter is the input of the next filter.

### Time Domain Before FFT

Before applying the Fast-Fourier-Transform (FFT), a tilt-compensation can be applied to the time domain trace to
compensate any misalignment along the $x$ axis and/or $y$ axis.
Additionally, a simple band-pass filter can be applied to exclude secondary peaks.

### FFT

To reduce artefacts in frequency domain, a window is multiplied to the time domain signal before applying the
Fast-Fourier-Transform (FFT). By default, an adapted Blackman
window is applied, but the user can also select other windows:

- Adapted Blackman (default)
- Blackman
- Hanning
- Hamming
- FlatTop

The adapted Blackman window is a modified version of the Blackman window, where most of the signal is preserved and only
the first and last couple of datapoints are modified.

All windows are defined in `math_tools.rs`.

### Frequency Band Pass Filter

A simple band-pass filter can be applied in fourier space to only display certain frequency bands.

### Inverse FFT

The inverse FFT is applied to convert the data from frequency domain back to time domain.

### Time Domain After FFT

After converting back to time domain, another band-pass filter can be applied to the time traces.
By selecting a slice in time domain, it is possible to scan through the $z$-axis of the scan and analysing
sub-surface layers [@koch-dandolo_reflection_2015]. The double-slider can be controlled with zoom and scroll/pan
gestures on the trackpad/mouse wheel. The user can step through the data along the time axis using the left/right arrow
keys, when hovered above the filter UI.

### Deconvolution

The deconvolution filter is an implementation of the Frequency-dependent Richardson-Lucy algorithm described
in [@demion_frequency-dependent_2025].

You need to perform a knife-edge scan and create a `.thz` file with all entries. An example can be found in the
`sample_data/example_beam_width/` directories.
Then, use the following command (replace the paths with your own):

```shell
python scripts/generate_psf.py \
  --path_x sample_data/example_beam_width/measurement_x/data/1750085285.8557956_data.thz \
  --path_y sample_data/example_beam_width/measurement_y/data/1750163177.929295_data.thz
  ```

to generate a `psf.npz` file that can be loaded in the settings of THz Image Explorer to remove the PSF blur by applying
the deconvolution filter.

The Richardson-Lucy algorithm is defined as

$$
\hat u_\xi^{(t)} = \hat u_\xi^{(t-1)} \cdot \frac{d_\xi}{\hat u_\xi^{(t-1)} * P_\xi} * P_\xi^{*},
$$
where $d$ is the observed scan composed of pixels, $\hat u$ is the reconstructed image, $P_\xi$ is the Point Spread
Function (PSF) around the frequency $\xi$, and $P_\xi^{*}(x,y)=P_\xi(-x,-y)$ is the flipped PSF.

In order to process the different frequency regions of the time traces, Linear phase FIR filters are designed such that,
$$
\begin{aligned}
\mathbf s_i & = \sum_{\xi=0}^{M-1} \mathbf s _{i\xi}\\
& =\sum_{\xi=0}^{M-1} \mathbf b _\xi * \mathbf s _{i},
\end{aligned}
$$
where $\mathbf{b}_\xi$ and $\mathbf{s}_i$ are respectively the FIR filters and the time traces. $\xi=0,\dots, M-1$ is
the index of the filter determining its center frequency and $\mathbf{s}_{i\xi}$ is the filtered time trace.

Assuming that the PSF does not induce phase modifications on the underlying signal, an
estimation $\mathbf{\hat s}_i^\prime$ of the underlying terahertz traces $\mathbf{s}_i^\prime$ for each pixel $i$ with
intensity $\hat u_i = \sum_\xi \hat u_{i\xi}$ can be computed with,
$$
\begin{aligned}
\mathbf{\hat s}_i^\prime & = \sum_{\xi=0}^{M-1} \mathbf{\hat s}_{i\xi}^\prime,\\
& = \sum_{\xi=0}^{M-1} g_{i\xi} \cdot \mathbf b_\xi * \mathbf s_{i},\\
& = \sum_{\xi=0}^{M-1} g_{i\xi} \cdot \mathbf s_{i\xi},
\end{aligned}
$$
where $g_{i\xi}$ is a gain factor for the frequency range $\xi$ at the pixel $i$.

The estimation of the underlying filtered intensity can be written,
$$
\begin{aligned}
\hat u_{i\xi} & = | \mathbf {\hat s}_{i\xi}^\prime |^2,\\
& = | g_{i\xi} \cdot \mathbf s_{i\xi} |^2 = g_{i\xi}^2 \cdot | \mathbf s_{i\xi} |^2,\\
& = g_{i\xi}^2 \cdot d_{i\xi}.
\end{aligned}
$$
Therefore, the gains can be computed with the output $\hat u_{i\xi}$ of the deconvolution algorithm applied on the
filtered data using the frequency range dependent PSFs,
$$
g_{i\xi} = \sqrt{\frac{\hat u_{i\xi}}{d_{i\xi}}}.
$$

### Custom Filters

The code-base can easily be extended with custom filters. The user needs to create a custom file in the `src/filters`
directory with a struct that implements the `Filter` trait. The file needs to be attached to the `mod.rs` file in the
`src/filters` directory, so that it is included in the compilation.
By defining the `config()` function, the user can supply a name, description and specify in which domain the filter
operates (time or frequency).
The math of the filter needs to be placed in the `filter()` function and the user-interface (UI) in the `ui()` function
the using `egui` library.
`Clone`, `Debug` and `CopyStaticFields` need to be derived.
Additionally, the `#[register_filter]` procedural macro
needs to added to the custom filter struct to automatically add it to the application and the user does not need to
adapt any other files.

Loops that require a lot of computation can be parallelized using the `rayon` crate, which is already included in the
project dependencies. It is recommended to use the  `cancellable_loops` crate to allow the user to abort long
running computations. This crate provides a convenient way to check for an abort flag and handle progress updates in the
GUI.

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

To implement more complex methods, further adaptations might be required, but the code structure has been set up with
modularity and simplicity in mind.

# Further processing (with Python)

After regions of interest (ROIs) have been selected, they can be saved in the meta-data of the dotTHz file. The
coordinates are saved for each ROI with label "ROI {i}" as a list, e.g.:

```
"ROI 1": [[27.57,34.72],[37.96,23.65],[40.35,32.06],[35.06,37.92]]
```

while the label of the ROI is saved in the "ROI Labels" meta-data field as a comma-separated list.
The file can then be opened with Python using the `pydotthz` package
to further process the data.
A Python code snipped for ROI extraction and the PSF generation script can be found in the `scripts` directory of the
repository.

# Summary

THz Image Explorer primarily serves as a performant data analysis tool for THz 2D images. The main focus lies on
preliminary
browsing of
measurements, rough analysis of scans and identifying regions of interest in each scan. It is designed in a modular way
to allow
possible implementation of more thorough analysis features in the future.

# Acknowledgements

This work was supported through a MARVIS (Multidisciplinary Advanced
Research Ventures in Space) programme of the Swiss Department for Business, Education,
and Research (SBFI) called SUBICE. SUBICE is a project of the University of Bern (UniBe),
the University of Applied Sciences and Arts, Western Switzerland (HES-SO), and Thales-Alenia Space Switzerland (TASCH).
The project has been partially funded by the European
Space Agency (ESA) under the ESA Initial Support for Innovation (EISI) program.
We acknowledge the support of the Open Space Innovation Platform (OSIP) and in
particular Nicolas Thiry and Leopold Summerer.

# References
