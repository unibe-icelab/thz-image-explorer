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
  - name: Nicolas Thomas
    orcid: 0000-0002-0146-0071
    affiliation: "1"
affiliations:
  - name: University of Bern, Bern, Switzerland
    index: 1
    ror: 02k7v4d05
date: 23 January 2025
bibliography: paper.bib

---

# Introduction

THz time-domain spectroscopy (TDS) is a fast-growing field with applications to perform non-destructive studies of material properties [@neu_tutorial_2018].
Different sources of THz radiations have been implemented in commercial products, e.g. photo-conductive antennas. The
pulses can either be measured in transmission or reflection and are recorded in the time domain. By transforming the
acquired trace into frequency domain (Fourier space), the magnitude and phase can be extracted, which allows to investigate the complex
refractive index and absorption coefficient of the sample.
By placing either the sample or the optical setup on a moving stage, the sample can be imaged in 2D. Analysing these
images pixel by pixel without an interactive user interface can be tedious.

![THz Image Explorer icon.\label{fig:icon}](icon.png){#id .class width=20%}

We developed an interactive graphical user interface (GUI), written in [rust](https://www.rust-lang.org), to aid investigating acquired 2D scans. The
application implements the dotTHz standard [@lee_dotthz_2023] and is cross-platform and open source, thus making it easier to maintain and making it available to the
entire scientific community.

# Statement of need

Interactive analysis tools for THz spectroscopy are essential to browse through images and analyse different regions of interest efficiently.
Commercial suppliers provide closed-source (e.g. [Menlo Systems](https://www.menlosystems.com)) where the code cannot be adapted by the user.
Solutions published by the scientific community are not available on all platforms, only focus on single pixel measurements and/or not focused on an interactive workflow [@peretti_thz-tds_2019; @loaiza_thztools_2024].
With this application, we provide a performant solution written in Rust, that allows an interactive analysis of 2D THz scans.
This work is open-source and pre-built bundles are available for Linux, macOS and Windows, making it available to the entire scientific community.

# Structure

The application is multi-threaded with two main thread:

- GUI thread
- Data thread

The GUI uses [egui](https://www.egui.rs), an immediate-mode egui library for rust with the native
back-end [glow](https://crates.io/crates/glow) based on openGL [@shreiner_opengl_2009].

The GUI thread handles all the user input and displaying of plots and other window elements. The configuration values set in the GUI are sent to the Data thread
via multiple-producer-single-consumer (MPSC) channels.
The Data thread then handles the computation of the applied filters.
The output of the computation is then shared via mutexes with the GUI thread.

# Installation

Pre-built bundles are available for each release on [GitHub](https://github.com/hacknus/thz-image-explorer) for

- macOS (`.app` bundle for x86 and apple silicon)
- Linux (executable and `.deb` for x86)
- Windows (`.exe` and `.msi` for x86)

These bundles should work out of the box. To compile directly from source, rust needs to be installed and the following
command needs to be executed:

```shell
cargo run --release
```

or to only build the executable without running

```shell
cargo build --release
```

With default settings `cmake` is required to install HDF5, which is required for the implementation of the dotTHz standard. If HDF5 is already installed on the
system, the user can change remove the `hdf5-sys-static` feature from the `dotthz` dependency in the `Cargo.toml` file.

On Linux, the following dependencies need to be installed first as a requirement for egui:
- `libclang-dev`
- `libgtk-3-dev`
- `libxcb-render0-dev`
- `libxcb-shape0-dev`
- `libxcb-xfixes0-dev`
- `libxkbcommon-dev`
- `libssl-dev`

On Linux you need to first run:  
`sudo apt-get install -y libclang-dev libgtk-3-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libxkbcommon-dev libssl-dev`

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

The window is structured with the time domain trace and frequency domain spectrum for the selected pixel (default is 0,0) at the center.
The left side-panel contains the intensity plot of the 2D scan along with the meta-data. The right side-panel contains the possible filters with configuration settings.
A pixel can be (de-)selected by clicking inside the intensity plot.

## IO

THz Image Explorer is able to load scans saved as `.npy` files and scans in the `.thz` (dotTHz) format,
which are based on the HDF5 standard.
This allows the files to also contain meta-data, which will also be displayed by the THz Image Explorer. The meta-data
is shown in the file opening dialog, allowing to easily
browse through directories containing multiple scans and is also displayed upon opening a scan.

## Filters

### FFT Window

To reduce artefacts in the frequency domain, a window is multiplied to the time domain before applying the Fast-Fourier-Transform (FFT). By default, the adapted Blackman
window is applied, but others are available. (TODO)

### Frequency Band Pass Filter

A simple band-pass filter can be applied in fourier space to only display certain frequency bands.

### Time Domain Slice

By selecting a slice in the time domain, it is possible to scan through the $z$-axis of the scan and analysing sub-surface layers [@koch-dandolo_reflection_2015].

### Deconvolution

cite Arnaud's paper

### Custom Filters

The code-base can easily be extended with custom filters. The user needs to create a custom file in the `src/filters` directory with a struct that implements the `Filter` trait.
Minor adaptations in required in the `right_panel.rs` file to implement the input parameters in the GUI. (TODO)



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

# Figures

Figures can be included like this:
![Caption for example figure.\label{fig:example}](figure.png)
and referenced from text using \autoref{fig:example}.

Figure sizes can be customized by adding an optional second parameter:
![Caption for example figure.](figure.png){ width=20% }

# Summary

THz Image Explorer primarily serves as a data analysis tool for THz 2D images. The main focus lies on preliminary browsing of
measurements, rough analysis of scans and identifying regions of interest in each scan. It is designed in a modular way to allow
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