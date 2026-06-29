# THz Image Explorer changelog

All notable changes to the `THz Image Explorer` project will be documented in this file.

# Unreleased 1.3.X - X.X.2026

*...

# 1.3.0 - 29.6.2026

* Update dependencies to `bevy 0.19`, `bevy-egui 0.41`, `egui 0.35`
* Fix DotTHz Quick Look plugin for macOS Tahoe
* Migrate the Settings panel from an egui floating window to a native OS secondary window using `bevy_egui`'s `EguiMultipassSchedule` system
* Fix theme propagation: changing the theme in Settings is now immediately reflected in the main window
* Fix initial theme of the Settings window: it now opens with the correct theme (including OS-follow mode) instead of always defaulting to dark

# 1.2.0 - 2.4.2026

* Update dependencies to `bevy 0.18`, `bevy-egui 0.39`

# 1.1.0 - 28.11.2025

* Fix 3D rendering bug (blending of pixels)
* Update to `bevy 0.17`, `ndarray 0.17`, `bevy-egui 0.38`

# 1.0.1 - 5.11.2025

* fix polygon averaging (x/y)
* fix color mapping for 2D plots (red - hot, blue - cold, bw - grayscale)
* add complete compilation instructions (dependencies for bevy and egui)
* added unit tests for filters and math tools
* release for publishing in JOSS

# 1.0.0 - 14.7.2025

* Initial stable release (submitted to JOSS)
* Fixed a bug in the file dialog on Windows and Linux
* Minor improvements / bug fixes

# 0.5.0 - 10.7.2025

* Updated dependencies
* Minor bug fixes
* Fixed release builds (macOS, Linux, Windows)
* ...

# 0.4.0 - 8.7.2025

* Bug Fixes for Reference Datasets

# 0.3.0 - 3.7.2025

* Moved from eframe to bevy
* Implemented 3D Voxel Plot
* Implemented `Filter` Trait
* Implemented Tilt Compensation Filter
* Implemented PSF Deconvolution Filter
* Implemented Band Pass Filters
* Implement Refractive Index Plot

# 0.2.2

* Initial release
