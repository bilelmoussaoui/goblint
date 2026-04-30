use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result};
use serde::Deserialize;

/// Represents the full output of `meson introspect --all`
#[derive(Debug, Deserialize)]
pub struct MesonIntrospection {
    #[serde(default)]
    pub benchmarks: Vec<Benchmark>,
    #[serde(default)]
    pub buildoptions: Vec<BuildOption>,
    #[serde(default)]
    pub buildsystem_files: Vec<String>,
    #[serde(default)]
    pub compilers: HashMap<String, HashMap<String, Compiler>>,
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
    #[serde(default)]
    pub install_plan: HashMap<String, HashMap<String, InstallPlan>>,
    #[serde(default)]
    pub installed: HashMap<String, String>,
    #[serde(default)]
    pub machines: HashMap<String, MachineInfo>,
    pub projectinfo: ProjectInfo,
    #[serde(default)]
    pub targets: Vec<Target>,
    #[serde(default)]
    pub tests: Vec<Test>,
}

#[derive(Debug, Deserialize)]
pub struct Benchmark {
    pub name: String,
    pub suite: Vec<String>,
    pub cmd: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub depends: Vec<String>,
    pub workdir: Option<String>,
    pub timeout: u32,
    pub protocol: String,
    pub priority: i32,
}

#[derive(Debug, Deserialize)]
pub struct BuildOption {
    pub name: String,
    #[serde(rename = "type")]
    pub option_type: String,
    pub value: serde_json::Value,
    pub section: String,
    pub machine: String,
    pub description: String,
    #[serde(default)]
    pub choices: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct Compiler {
    pub id: String,
    #[serde(default)]
    pub exelist: Vec<String>,
    #[serde(default)]
    pub linker_exelist: Vec<String>,
    #[serde(default)]
    pub file_suffixes: Vec<String>,
    #[serde(default)]
    pub default_suffix: Option<String>,
    pub version: String,
    pub full_version: String,
    #[serde(default)]
    pub linker_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Dependency {
    pub name: String,
    #[serde(rename = "type")]
    pub dep_type: String,
    pub version: String,
    #[serde(default)]
    pub compile_args: Vec<String>,
    #[serde(default)]
    pub link_args: Vec<String>,
    #[serde(default)]
    pub include_directories: Vec<String>,
    #[serde(default)]
    pub sources: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct InstallPlan {
    pub destination: String,
    #[serde(default)]
    pub tag: Option<String>,
    #[serde(default)]
    pub subproject: Option<String>,
    #[serde(default)]
    pub install_rpath: Option<String>,
    #[serde(default)]
    pub exclude_dirs: Vec<String>,
    #[serde(default)]
    pub exclude_files: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct MachineInfo {
    pub system: String,
    pub cpu_family: String,
    pub cpu: String,
    pub endian: String,
}

#[derive(Debug, Deserialize)]
pub struct ProjectInfo {
    pub version: String,
    pub descriptive_name: String,
    #[serde(default)]
    pub license: Vec<String>,
    #[serde(default)]
    pub subprojects: Vec<SubprojectInfo>,
}

#[derive(Debug, Deserialize)]
pub struct SubprojectInfo {
    pub name: String,
    pub version: String,
    pub descriptive_name: String,
}

#[derive(Debug, Deserialize)]
pub struct Target {
    pub name: String,
    pub id: String,
    #[serde(rename = "type")]
    pub target_type: String,
    pub defined_in: String,
    pub filename: Vec<String>,
    pub build_by_default: bool,
    #[serde(default)]
    pub target_sources: Vec<TargetSource>,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub subproject: Option<String>,
    pub installed: bool,
}

#[derive(Debug, Deserialize)]
pub struct TargetSource {
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub compiler: Vec<String>,
    #[serde(default)]
    pub parameters: Vec<String>,
    #[serde(default)]
    pub sources: Vec<String>,
    #[serde(default)]
    pub generated_sources: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct Test {
    pub name: String,
    pub suite: Vec<String>,
    pub cmd: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub depends: Vec<String>,
    pub workdir: Option<String>,
    pub timeout: u32,
    pub protocol: String,
    pub priority: i32,
}

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

/// Get full meson introspection data by running `meson introspect --all`
pub fn get_introspection(build_dir: &Path) -> Result<MesonIntrospection> {
    // Run meson introspect --all
    let output = Command::new("meson")
        .arg("introspect")
        .arg(build_dir)
        .arg("--all")
        .output()
        .context("Failed to run meson introspect (is meson installed?)")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("meson introspect --all failed: {}", stderr);
    }

    // Parse JSON output
    let stdout =
        String::from_utf8(output.stdout).context("meson introspect output is not valid UTF-8")?;

    let introspection: MesonIntrospection =
        serde_json::from_str(&stdout).context("Failed to parse meson introspect JSON output")?;

    Ok(introspection)
}

/// Extract GObject introspection headers from meson introspection data
/// These are the headers scanned by g-ir-scanner to generate .gir files
/// This is more precise than just "installed headers" as it represents the
/// actual public GObject API
impl MesonIntrospection {
    /// Load meson introspection data for a project
    /// Returns None if no build directory is found
    pub fn for_project(
        project_root: &Path,
        config_build_dir: Option<&str>,
    ) -> Result<Option<Self>> {
        // Find build directory
        let Some(build_dir) = find_build_dir(project_root, config_build_dir) else {
            return Ok(None);
        };

        // Get introspection data
        let introspection = get_introspection(&build_dir)?;
        Ok(Some(introspection))
    }

    /// Get headers scanned by g-ir-scanner (GObject introspection headers)
    pub fn get_introspected_headers(&self) -> Result<HashSet<PathBuf>> {
        let mut headers = HashSet::new();

        // Find all targets that produce .gir files
        for target in &self.targets {
            // Check if this target produces a .gir file
            let produces_gir = target.filename.iter().any(|f| f.ends_with(".gir"));
            if !produces_gir {
                continue;
            }

            // Look for --filelist argument in g-ir-scanner command
            for source in &target.target_sources {
                for arg in &source.compiler {
                    if let Some(filelist_path) = arg.strip_prefix("--filelist=") {
                        // Read the filelist and extract .h files
                        let content = std::fs::read_to_string(filelist_path)
                            .context(format!("Failed to read GIR filelist: {}", filelist_path))?;

                        for line in content.lines() {
                            let line = line.trim();
                            if line.ends_with(".h") {
                                headers.insert(PathBuf::from(line));
                            }
                        }
                    }
                }
            }
        }

        Ok(headers)
    }

    /// Get installed header files (all headers that will be installed)
    pub fn get_installed_headers(&self) -> HashSet<PathBuf> {
        let mut headers = HashSet::new();
        for source_path in self.installed.keys() {
            if source_path.ends_with(".h") {
                headers.insert(PathBuf::from(source_path));
            }
        }
        headers
    }
}

/// Get installed headers for a project
/// This is the main entry point that combines build dir finding and header
/// extraction
pub fn get_public_headers(
    project_root: &Path,
    config_build_dir: Option<&str>,
) -> Result<Option<HashSet<PathBuf>>> {
    let Some(introspection) = MesonIntrospection::for_project(project_root, config_build_dir)?
    else {
        return Ok(None);
    };

    Ok(Some(introspection.get_installed_headers()))
}
