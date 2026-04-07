use anyhow::{Context, Result};
use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::Deserialize;
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub rules: RulesConfig,

    #[serde(default)]
    pub ignore: Vec<String>,

    /// Editor URL format for clickable links
    /// Available placeholders: {path}, {line}, {column}
    /// Examples:
    ///   VSCode: "vscode://file{path}:{line}:{column}"
    ///   IntelliJ: "idea://open?file={path}&line={line}"
    ///   Sublime: "subl://open?url=file://{path}&line={line}&column={column}"
    pub editor_url: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct RulesConfig {
    #[serde(default = "default_true")]
    pub g_param_spec_null_nick_blurb: bool,

    #[serde(default = "default_true")]
    pub dispose_finalize_chains_up: bool,

    #[serde(default = "default_true")]
    pub use_clear_functions: bool,

    #[serde(default = "default_true")]
    pub use_g_strcmp0: bool,

    #[serde(default = "default_true")]
    pub property_enum_zero: bool,

    #[serde(default = "default_true")]
    pub deprecated_add_private: bool,

    #[serde(default = "default_true")]
    pub gerror_init: bool,

    #[serde(default = "default_true")]
    pub gtask_source_tag: bool,

    #[serde(default = "default_true")]
    pub unnecessary_null_check: bool,

    #[serde(default = "default_true")]
    pub missing_implementation: bool,
}

fn default_true() -> bool {
    true
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            // Return default config if file doesn't exist
            return Ok(Config {
                rules: RulesConfig::default(),
                ignore: Vec::new(),
                editor_url: None,
            });
        }

        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;

        let config: Config = toml::from_str(&content)
            .with_context(|| format!("Failed to parse config file: {}", path.display()))?;

        Ok(config)
    }

    pub fn build_ignore_matcher(&self) -> Result<GlobSet> {
        let mut builder = GlobSetBuilder::new();

        for pattern in &self.ignore {
            let glob = Glob::new(pattern)
                .with_context(|| format!("Invalid ignore pattern: {}", pattern))?;
            builder.add(glob);
        }

        builder.build().context("Failed to build ignore matcher")
    }
}
