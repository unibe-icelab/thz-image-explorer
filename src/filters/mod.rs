//! Signal processing filters for terahertz time-domain spectroscopy data.
//!
//! This module contains various filter implementations that can be applied to
//! time-domain and frequency-domain data in a THz-TDS imaging system. Filters are
//! organized by their domain of operation and processing order.
//!
//! # Filter Categories
//!
//! * **Time Domain Filters (Pre-FFT)**: Applied to raw time-domain signals before
//!   frequency transformation.
//!
//! * **Frequency Domain Filters**: Applied to data after FFT transformation.
//!
//! * **Time Domain Filters (Post-FFT)**: Applied to time-domain signals after
//!   inverse transformation from frequency domain.
//!
//! # Filter Implementations
//!
//! Each filter implements the `Filter` trait defined in the `filter` module,
//! providing a consistent interface for configuration, application, and visualization.

/// Frequency domain bandpass filter for isolating specific frequency ranges.
mod band_pass_fd;

/// Time domain bandpass filter that operates after FFT processing.
/// Allows temporal selection of signals after frequency domain operations.
mod band_pass_td_after_fft;

/// Time domain bandpass filter that operates before FFT processing.
/// Allows temporal selection of signals in the raw data.
mod band_pass_td_before_fft;

/// Signal deconvolution filter for removing system response effects.
/// Improves signal quality by compensating for the measurement system's transfer function.
mod deconvolution;

/// Core filter interfaces and shared components.
/// Defines the `Filter` trait and supporting structures used by all filter implementations.
pub mod filter;

/// Point Spread Function utilities for deconvolution operations.
/// Contains tools for measuring and applying PSF-based corrections.
pub mod psf;

/// Compensates for physical sample tilt by applying position-dependent time shifts.
/// Corrects for optical path length differences across tilted samples.
mod tilt_compensation;
