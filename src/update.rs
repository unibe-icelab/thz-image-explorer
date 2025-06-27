//! This module implements functionalities for self-updating the application.
//!
//! It supports fetching releases from a GitHub repository, finding and downloading the
//! latest release, and replacing the currently running binary with the updated version.
//!
//! Features include:
//! - Checking for available updates using semantic versioning comparison.
//! - Custom update logic, including extracting and replacing binaries for different platforms.
//! - Handling platform-specific requirements (e.g., Windows, macOS, Linux).
//!
//! The module uses the `self_update` crate for managing downloads and replacements seamlessly.

#![cfg(feature = "self_update")]

use self_update::self_replace;
use self_update::update::Release;
use semver::Version;
use std::path::Path;
use std::{env, fs, io};

const REPO_OWNER: &str = "unibe-icelab";
const REPO_NAME: &str = "thz-image-explorer";
const MACOS_APP_NAME: &str = "THz Image Explorer.app";

/// Recursively copies the contents of the `src` directory to the `dest` directory, excluding a specified binary.
///
/// This function ensures that the current binary is skipped to avoid overwriting it during the update process
/// while the `self_replace` function takes care of replacing the binary.
///
/// # Arguments
/// * `src` - The source directory to copy from.
/// * `dest` - The destination directory to copy to.
/// * `binary_name` - The name of the binary file to exclude from copying.
///
/// # Errors
/// Returns an I/O error if any of the paths cannot be read, created, or copied.
///
/// # Platform-Specific Notes
/// On macOS, this function is used to copy the `Contents` of the application bundle.
///
/// # Examples
/// ```ignore
/// copy_dir(Path::new("source"), Path::new("destination"), "my_binary").unwrap();
/// ```
fn copy_dir(src: &Path, dest: &Path, binary_name: &str) -> io::Result<()> {
    // Ensure the destination directory exists
    if !dest.exists() {
        fs::create_dir_all(dest)?;
    }

    // Iterate through entries in the source directory
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let dest_path = dest.join(entry.file_name());

        if path.is_dir() {
            // Recursively copy subdirectories
            copy_dir(&path, &dest_path, binary_name)?;
        } else if let Some(file_name) = path.file_name() {
            if file_name != binary_name {
                // Copy files except for the binary
                fs::copy(&path, &dest_path)?;
            }
        }
    }

    Ok(())
}

/// Checks for updates by fetching the release list from the GitHub repository.
///
/// It compares the current version of the application with the versions available in the repository.
/// If a newer version exists, it returns the latest release.
///
/// # Returns
/// * `Option<Release>` - The latest release that is newer than the current version, or `None` if no updates are available.
///
/// # Errors
/// Returns `None` if fetching releases fails or if no newer version exists.
///
/// # Examples
/// ```ignore
/// if let Some(latest_release) = check_update() {
///     println!("New update available: {}", latest_release.version);
/// }
/// ```
pub fn check_update() -> Option<Release> {
    if let Ok(builder) = self_update::backends::github::ReleaseList::configure()
        .repo_owner(REPO_OWNER)
        .repo_name(REPO_NAME)
        .build()
    {
        if let Ok(releases) = builder.fetch() {
            let current_version = Version::parse(env!("CARGO_PKG_VERSION")).unwrap();
            return releases
                .iter()
                .filter_map(|release| {
                    let release_version_str = release
                        .version
                        .strip_prefix("v")
                        .unwrap_or(&release.version);
                    Version::parse(release_version_str)
                        .ok()
                        .map(|parsed_version| (parsed_version, release))
                })
                .filter(|(parsed_version, _)| parsed_version > &current_version) // Compare versions
                .max_by(|(a, _), (b, _)| a.cmp(b)) // Find the max version
                .map(|(_, release)| release.clone()); // Return the release
        }
    }
    None
}

/// Updates the current executable by downloading and replacing it with the latest release.
///
/// This function downloads the release asset matching the target platform and extracts it to a temporary directory.
/// Then, it replaces the executable with the new version, preserving the application's data and structure.
///
/// # Arguments
/// * `release` - The release to download and install.
///
/// # Platform-Specific Logic
/// - On Windows: Looks for an `exe` asset.
/// - On Linux: Looks for a `bin` asset.
/// - On macOS: Handles application bundles.
///
/// # Errors
/// * Returns an error if downloading, extracting, or replacing the binary fails.
/// * Will also error if the platform is unsupported.
///
/// # Examples
/// ```ignore
/// if let Some(latest_release) = check_update() {
///     update(latest_release).unwrap();
/// }
/// ```
pub fn update(release: Release) -> Result<(), Box<dyn std::error::Error>> {
    let target_asset = if cfg!(target_os = "windows") {
        release.asset_for(self_update::get_target(), Some("exe"))
    } else if cfg!(target_os = "linux") {
        release.asset_for(self_update::get_target(), Some("bin"))
    } else {
        release.asset_for(self_update::get_target(), None)
    }
    .ok_or("No asset found")?;
    let tmp_archive_dir = tempfile::TempDir::new()?;
    let tmp_archive_path = tmp_archive_dir.path().join(&target_asset.name);
    let tmp_archive = fs::File::create(&tmp_archive_path)?;

    self_update::Download::from_url(&target_asset.download_url)
        .set_header(reqwest::header::ACCEPT, "application/octet-stream".parse()?)
        .download_to(&tmp_archive)?;

    self_update::Extract::from_source(&tmp_archive_path).extract_into(tmp_archive_dir.path())?;
    let new_exe = if cfg!(target_os = "windows") {
        let binary = env::current_exe()?
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        tmp_archive_dir.path().join(binary)
    } else if cfg!(target_os = "macos") {
        let binary = env::current_exe()?
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        let app_dir = env::current_exe()?
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();

        let app_name = app_dir
            .clone()
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();

        let _ = copy_dir(&tmp_archive_dir.path().join(&app_name), &app_dir, &binary);

        // MACOS_APP_NAME either needs to be hardcoded or extracted from the downloaded and
        // extracted archive, but we cannot just assume that the parent directory of the
        // currently running executable is equal to the app name - this is especially not
        // the case if we run the code with `cargo run`.
        tmp_archive_dir
            .path()
            .join(format!("{}/Contents/MacOS/{}", MACOS_APP_NAME, binary))
    } else if cfg!(target_os = "linux") {
        let binary = env::current_exe()?
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        tmp_archive_dir.path().join(binary)
    } else {
        return Err("Running on unsupported OS".into());
    };

    self_replace::self_replace(new_exe)?;
    Ok(())
}
