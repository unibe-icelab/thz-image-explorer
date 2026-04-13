//! Temporary workaround for system theme detection until bevy_egui supports it natively.
//!
//! This module provides system theme detection for macOS, Windows, and Linux.
//! It includes caching to avoid performance issues from frequent system calls.
//!
//! TODO: Remove this module when bevy_egui properly supports egui::RawInput::system_theme

use bevy_egui::egui;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Cache for system theme detection to avoid frequent system calls
static THEME_CACHE: once_cell::sync::Lazy<Mutex<ThemeCache>> =
    once_cell::sync::Lazy::new(|| Mutex::new(ThemeCache::new()));

struct ThemeCache {
    is_dark: bool,
    last_check: Instant,
}

impl ThemeCache {
    fn new() -> Self {
        Self {
            is_dark: detect_system_dark_mode_uncached(),
            last_check: Instant::now(),
        }
    }
}

/// Detects if the system is in dark mode.
/// Results are cached for 500ms to avoid performance issues.
pub fn is_system_dark_mode() -> bool {
    if let Ok(mut cache) = THEME_CACHE.lock() {
        if cache.last_check.elapsed() < Duration::from_millis(500) {
            return cache.is_dark;
        }

        cache.is_dark = detect_system_dark_mode_uncached();
        cache.last_check = Instant::now();
        cache.is_dark
    } else {
        // Fallback if mutex is poisoned
        false
    }
}

#[cfg(target_os = "macos")]
fn detect_system_dark_mode_uncached() -> bool {
    use std::process::Command;

    if let Ok(output) = Command::new("defaults")
        .args(&["read", "-g", "AppleInterfaceStyle"])
        .output()
    {
        if output.status.success() {
            let appearance = String::from_utf8_lossy(&output.stdout);
            return appearance.trim() == "Dark";
        }
    }
    // Default to light if we can't determine (light mode doesn't set the key)
    false
}

#[cfg(target_os = "windows")]
fn detect_system_dark_mode_uncached() -> bool {
    use std::process::Command;

    // Query Windows registry for theme preference
    if let Ok(output) = Command::new("reg")
        .args(&[
            "query",
            "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize",
            "/v",
            "AppsUseLightTheme",
        ])
        .output()
    {
        if output.status.success() {
            let result = String::from_utf8_lossy(&output.stdout);
            // If AppsUseLightTheme is 0x0, it means dark mode is enabled
            return result.contains("0x0");
        }
    }
    // Default to light if we can't determine
    false
}

#[cfg(target_os = "linux")]
fn detect_system_dark_mode_uncached() -> bool {
    use std::process::Command;

    // Try GNOME/GTK settings first
    if let Ok(output) = Command::new("gsettings")
        .args(&["get", "org.gnome.desktop.interface", "color-scheme"])
        .output()
    {
        if output.status.success() {
            let scheme = String::from_utf8_lossy(&output.stdout);
            if scheme.contains("dark") {
                return true;
            }
        }
    }

    // Try KDE settings
    if let Ok(output) = Command::new("kreadconfig5")
        .args(&["--group", "General", "--key", "ColorScheme"])
        .output()
    {
        if output.status.success() {
            let scheme = String::from_utf8_lossy(&output.stdout);
            if scheme.to_lowercase().contains("dark") {
                return true;
            }
        }
    }

    // Default to light if we can't determine
    false
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
fn detect_system_dark_mode_uncached() -> bool {
    // For other systems, default to light mode
    false
}

/// Applies the system theme to an egui context if System theme preference is active.
/// Returns true if visuals were updated.
pub fn apply_system_theme_if_needed(
    ctx: &egui::Context,
    theme_preference: egui::ThemePreference,
) -> bool {
    if theme_preference != egui::ThemePreference::System {
        return false;
    }

    let is_dark = is_system_dark_mode();
    let current_is_dark = ctx.style().visuals.dark_mode;

    if is_dark != current_is_dark {
        log::info!(
            "System theme changed: {} -> {}",
            if current_is_dark { "dark" } else { "light" },
            if is_dark { "dark" } else { "light" }
        );
        ctx.set_visuals(if is_dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        });

        // Re-apply handle shape after changing visuals
        ctx.style_mut(|style| {
            style.visuals.handle_shape = egui::style::HandleShape::Circle;
        });

        true
    } else {
        false
    }
}
