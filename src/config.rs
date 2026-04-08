use anyhow::{Context, Result};
use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::Deserialize;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
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

/// Per-rule configuration
#[derive(Debug, Default, Clone)]
pub struct RuleConfig {
    pub enabled: bool,
    pub ignore: Vec<String>,
}

impl<'de> Deserialize<'de> for RuleConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{self, MapAccess, Visitor};
        use std::fmt;

        struct RuleConfigVisitor;

        impl<'de> Visitor<'de> for RuleConfigVisitor {
            type Value = RuleConfig;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a boolean or a RuleConfig struct")
            }

            fn visit_bool<E>(self, value: bool) -> Result<RuleConfig, E>
            where
                E: de::Error,
            {
                Ok(RuleConfig {
                    enabled: value,
                    ignore: Vec::new(),
                })
            }

            fn visit_map<M>(self, mut map: M) -> Result<RuleConfig, M::Error>
            where
                M: MapAccess<'de>,
            {
                let mut enabled = None;
                let mut ignore = None;

                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "enabled" => {
                            if enabled.is_some() {
                                return Err(de::Error::duplicate_field("enabled"));
                            }
                            enabled = Some(map.next_value()?);
                        }
                        "ignore" => {
                            if ignore.is_some() {
                                return Err(de::Error::duplicate_field("ignore"));
                            }
                            ignore = Some(map.next_value()?);
                        }
                        _ => {
                            let _: serde::de::IgnoredAny = map.next_value()?;
                        }
                    }
                }

                Ok(RuleConfig {
                    enabled: enabled.unwrap_or(true),
                    ignore: ignore.unwrap_or_default(),
                })
            }
        }

        deserializer.deserialize_any(RuleConfigVisitor)
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct RulesConfig {
    #[serde(default)]
    pub g_param_spec_null_nick_blurb: RuleConfig,

    #[serde(default)]
    pub dispose_finalize_chains_up: RuleConfig,

    #[serde(default)]
    pub use_clear_functions: RuleConfig,

    #[serde(default)]
    pub use_g_strcmp0: RuleConfig,

    #[serde(default)]
    pub property_enum_zero: RuleConfig,

    #[serde(default)]
    pub deprecated_add_private: RuleConfig,

    #[serde(default)]
    pub gerror_init: RuleConfig,

    #[serde(default)]
    pub gtask_source_tag: RuleConfig,

    #[serde(default)]
    pub unnecessary_null_check: RuleConfig,

    #[serde(default)]
    pub missing_implementation: RuleConfig,

    #[serde(default)]
    pub gdeclare_semicolon: RuleConfig,

    #[serde(default)]
    pub strcmp_for_string_equal: RuleConfig,

    #[serde(default)]
    pub use_g_set_str: RuleConfig,
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

    /// Build an ignore matcher for a specific rule, combining global and per-rule ignores
    pub fn build_rule_ignore_matcher(&self, rule_config: &RuleConfig) -> Result<GlobSet> {
        let mut builder = GlobSetBuilder::new();

        // Add global ignore patterns
        for pattern in &self.ignore {
            let glob = Glob::new(pattern)
                .with_context(|| format!("Invalid ignore pattern: {}", pattern))?;
            builder.add(glob);
        }

        // Add per-rule ignore patterns
        for pattern in &rule_config.ignore {
            let glob = Glob::new(pattern)
                .with_context(|| format!("Invalid ignore pattern: {}", pattern))?;
            builder.add(glob);
        }

        builder.build().context("Failed to build ignore matcher")
    }
}
