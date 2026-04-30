use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result};
use serde::Deserialize;

/// Represents the output of `meson introspect --installed`
/// This is a JSON object mapping source paths to install destinations
#[derive(Debug, Deserialize)]
struct InstalledFiles(std::collections::HashMap<String, String>);

/// Find the meson build directory by looking for meson-info/meson-info.json
/// Searches in:
/// 1. User-specified build_dir from config
/// 2. Common build directory names (build/, builddir/, _build/)
/// 3. Any directory containing meson-info/
pub fn find_build_dir(project_root: &Path, config_build_dir: Option<&str>) -> Option<PathBuf> {
    // 1. Check user-specified build_dir
    if let Some(dir) = config_build_dir {
        let path = project_root.join(dir);
        if is_build_dir(&path) {
            return Some(path);
        }
    }

    // 2. Check common build directory names
    for name in &["build", "builddir", "_build"] {
        let path = project_root.join(name);
        if is_build_dir(&path) {
            return Some(path);
        }
    }

    // 3. Search for any directory containing meson-info/
    if let Ok(entries) = std::fs::read_dir(project_root) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_dir() && is_build_dir(&path) {
                return Some(path);
            }
        }
    }

    None
}

/// Check if a directory is a valid meson build directory
fn is_build_dir(path: &Path) -> bool {
    path.join("meson-info").join("meson-info.json").exists()
}

/// Get the set of installed header files by running meson introspection
/// Returns paths as they appear in the source tree (absolute paths)
pub fn get_installed_headers(build_dir: &Path) -> Result<HashSet<PathBuf>> {
    // Run meson introspect --installed
    let output = Command::new("meson")
        .arg("introspect")
        .arg(build_dir)
        .arg("--installed")
        .output()
        .context("Failed to run meson introspect (is meson installed?)")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("meson introspect failed: {}", stderr);
    }

    // Parse JSON output
    let stdout =
        String::from_utf8(output.stdout).context("meson introspect output is not valid UTF-8")?;

    let installed: InstalledFiles =
        serde_json::from_str(&stdout).context("Failed to parse meson introspect JSON output")?;

    // Filter for .h files and convert to absolute paths
    let mut headers = HashSet::new();
    for (source_path, _install_path) in installed.0 {
        if source_path.ends_with(".h") {
            // Source paths in meson introspect output are absolute
            headers.insert(PathBuf::from(source_path));
        }
    }

    Ok(headers)
}

/// Get installed headers for a project
/// This is the main entry point that combines build dir finding and header
/// extraction
pub fn get_public_headers(
    project_root: &Path,
    config_build_dir: Option<&str>,
) -> Result<Option<HashSet<PathBuf>>> {
    // Find build directory
    let Some(build_dir) = find_build_dir(project_root, config_build_dir) else {
        // No build directory found - this is not an error, just means we can't
        // determine public/private distinction
        return Ok(None);
    };

    // Get installed headers
    let headers = get_installed_headers(&build_dir)?;
    Ok(Some(headers))
}
