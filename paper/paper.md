---
title: 'THz Image Explorer - A Cross-Platform Open-Source THz Image Analysis Tool'
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
  - name: University of Bern, Bern, Switzerland
    index: 1
    ror: 02k7v4d05
  - name: University of Applied Sciences and Arts Western Switzerland Valais, HES-SO Valais-Wallis, Sion, Switzerland
    index: 2
    ror: 03r5zec51
date: 23 January 2025
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

We developed an interactive graphical user interface (GUI), written in [Rust](https://www.rust-lang.org), to aid
investigating acquired 2D scans. The
application implements the dotTHz standard [@lee_dotthz_2023] and is platform independent and open source, thus making
it
easier to maintain and increasing its reach.

# Statement of need

Interactive analysis tools for THz spectroscopy are essential to browse through images and analyse different regions of
interest efficiently.
Commercial suppliers provide closed-source analysis tools (e.g. [Menlo Systems](https://www.menlosystems.com)) where the
code cannot be adapted by the user, which is often essential in research environments and extends the maintainability of
the code.
Solutions published by the scientific community are not available on all platforms, are only applicable on single pixel
measurements and/or not focused on an interactive workflow [@peretti_thz-tds_2019; @loaiza_thztools_2024].
With this application, we provide a performant solution written in Rust, that allows an interactive analysis of 2D THz
scans.
This work is open-source and pre-built bundles are available for Linux, macOS and Windows, making it available to the
entire scientific community.

# Structure

The application is multithreaded with two main threads:

- GUI thread
- Data thread

The GUI uses [egui](https://www.egui.rs), an immediate-mode GUI library for rust with the native
back-end [glow](https://crates.io/crates/glow) based on openGL [@shreiner_opengl_2009].

The GUI thread handles all the user input and displaying of plots and other window elements. The configuration values
set in the GUI are sent to the Data thread
via multiple-producer-single-consumer (MPSC) channels.
The Data thread then handles the computation of the applied filters.
The output of the computation is then shared via mutexes with the GUI thread.
The entire thread communication is handled with the `GuiThreadCommunication` and `MainThreadCommunication` structs. To
extend the communication for additional data-types, these two structs need to be extended with `Arc<RwLock<T>>` or
`mpsc::Sender<T>`/`mpsc::Receiver<T>`.

# Installation

## Pre-built Bundles

Pre-built bundles are available for each release on [GitHub](https://github.com/hacknus/thz-image-explorer) for

- macOS (`.app` bundle for x86 and Apple Silicon)
- Linux (executable and `.deb` for x86)
- Windows (`.exe` and `.msi` for x86)

These bundles should work out of the box, but on macOS you might need to remove the quarantine flag after downloading by
running the following command:

```shell
xattr -rd com.apple.quarantine THz\ Image\ Explorer.app
```

## Compile from Source

Alternatively, to compile directly from source, rust needs to be installed and the following
command needs to be executed:

```shell
cargo run --release
```

or to only build the executable without running:

```shell
cargo build --release
```

With default settings `cmake` is required to build HDF5 from source, which is required for the implementation of the
dotTHz
standard. If HDF5 is already installed on the
system, the user can change remove the `hdf5-sys-static` feature from the `dotthz` dependency in the `Cargo.toml` file.

On Linux, the following dependencies need to be installed first as a requirement for `egui`:

- `libclang-dev`
- `libgtk-3-dev`
- `libxcb-render0-dev`
- `libxcb-shape0-dev`
- `libxcb-xfixes0-dev`
- `libxkbcommon-dev`
- `libssl-dev`

On Linux you need to first run:

```shell
sudo apt-get install -y libclang-dev libgtk-3-dev \
libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev \
libxkbcommon-dev libssl-dev
```

To create bundles `cargo-bundle` needs to be installed (macOS, Linux):

```shell
cargo bundle --release
```

or `cargo-wix` on Windows:

```shell
cargo wix --release
```

An update feature, which will download the latest release and upgrade the installed application, is implemented in the
settings window.

# Usage

The window is structured with the time domain trace and frequency domain spectrum for the selected pixel (default is
0,0) at the center.
The left side-panel contains the intensity plot of the 2D scan along with the meta-data. The right side-panel contains
the possible filters with configuration settings.
A pixel can be (de-)selected by clicking inside the intensity plot.

A sample scan is available in the `sample_data` directory.

## IO

THz Image Explorer is able to load scans saved as `.npy` files and scans in the `.thz` (dotTHz) format,
which are based on the HDF5 standard.
This allows the files to also contain meta-data, which will also be displayed by the THz Image Explorer. The meta-data
is shown in the file opening dialog, allowing to easily
browse through directories containing multiple scans and is also displayed upon opening a scan.

## Filters

### FFT Window

To reduce artefacts in the frequency domain, a window is multiplied to the time domain before applying the
Fast-Fourier-Transform (FFT). By default, the adapted Blackman
window is applied, but others are available. (TODO)

### Frequency Band Pass Filter

A simple band-pass filter can be applied in fourier space to only display certain frequency bands.

### Time Domain Slice

By selecting a slice in the time domain, it is possible to scan through the $z$-axis of the scan and analysing
sub-surface layers [@koch-dandolo_reflection_2015].

### Deconvolution

cite Arnaud's paper

Single dollars ($) are required for inline mathematics e.g. $f(x) = e^{\pi/x}$

Double dollars make self-standing equations:

$$\Theta(x) = \left\{\begin{array}{l}
0\textrm{ if } x < 0\cr
1\textrm{ else}
\end{array}\right.$$

You can also use plain \LaTeX for equations
\begin{equation}\label{eq:fourier}
\hat f(\omega) = \int_{-\infty}^{\infty} f(x) e^{i\omega x} dx
\end{equation}
and refer to \autoref{eq:fourier} from text.

### Custom Filters

The code-base can easily be extended with custom filters. The individual filter parameters will be wrapped in
`ParameterKind` structs to define how they should be displayed in the GUI. Each filter can be set to either be applied
in the time or frequency domain with the `FilterDomain` enum.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterDomain {
    Time,
    Frequency,
}

#[derive(Debug, Clone)]
pub enum ParameterKind {
    Int(isize),
    UInt(usize),
    Float(f64),
    Boolean(bool),
    Slider {
        value: f64,
        show_in_plot: bool,
        min: f64,
        max: f64,
    },
    DoubleSlider {
        values: [f64; 2],
        show_in_plot: bool,
        minimum_separation: f64,
        inverted: bool,
        min: f64,
        max: f64,
    },
}
```

The user needs to create a custom file in the `src/filters`
directory with a struct that implements the `Filter` trait. Additionally, the `#[register_filter]` procedural macro
needs to added to the custom filter struct to automatically add it to the application.

```rust
use crate::data_container::ScannedImage;
use crate::filters::filter::{Filter, FilterConfig,
                             FilterDomain, FilterParameter,
                             ParameterKind};
use filter_macros::register_filter;

#[derive(Debug)]
#[register_filter]
pub struct CustomFilter {
    pub param1: f32,
    pub param2: f32,
    pub param3: usize,
}

impl Filter for CustomFilter {
    fn filter(&self, _t: &mut ScannedImage) {
        todo!();
        // Implement your filter algorithm here
    }

    fn config(&self) -> FilterConfig {
        FilterConfig {
            name: "Custom Filter".to_string(),
            domain: FilterDomain::Frequency,
            parameters: vec![
                FilterParameter {
                    name: "Parameter 1".to_string(),
                    kind: ParameterKind::Float(self.param1),
                },
                FilterParameter {
                    name: "Parameter 2".to_string(),
                    kind: ParameterKind::Float(self.param2),
                },
                FilterParameter {
                    name: "Parameter 3".to_string(),
                    kind: ParameterKind::UInt(self.param3),
                },
            ],
        }
    }
}
```

To implement more complex methods, further adaptations are required, but the code structure has been set up with
modularity and simplicity in mind.

# Summary

THz Image Explorer primarily serves as a data analysis tool for THz 2D images. The main focus lies on preliminary
browsing of
measurements, rough analysis of scans and identifying regions of interest in each scan. It is designed in a modular way
to allow
possible implementation of more thorough analysis features in the future.

# Acknowledgements

This work was supported through a MARVIS (Multidisciplinary Advanced
Research Ventures in Space) programme of the Swiss Department for Business, Education,
and Research (SBFI) called SUBICE. SUBICE is a project of the University of Bern (UniBe),
the University of Applied Sciences and Arts, Western Switzerland (HES-SO), and Thales-
Alenia Space Switzerland (TASCH). The project has been partially funded by the European
Space Agency (ESA) under the ESA Initial Support for Innovation (EISI) program.
We acknowledge the support of the Open Space Innovation Platform (OSIP) and in
particular Nicolas Thiry and Leopold Summerer.

# References