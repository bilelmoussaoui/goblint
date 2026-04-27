use std::{collections::HashMap, fs, path::Path};

use anyhow::{Context, Result};
use clap::ValueEnum;
use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::Deserialize;

use crate::rules::*;

#[derive(Default, Debug, Clone, Copy, ValueEnum, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OutputFormat {
    /// Human-readable colorized output (default)
    #[default]
    Text,
    /// JSON format
    Json,
    /// SARIF JSON format for GitHub Code Scanning, VS Code, etc.
    Sarif,
    /// GCC-compatible format for Emacs, Vim, and other tools
    Gcc,
    /// Gitlab specific Code Quality Report
    GitlabCodequality,
}

/// Parse a GLib version string like "2.76" into (major, minor)
pub fn parse_glib_version(version: &str) -> Option<(u32, u32)> {
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() != 2 {
        return None;
    }
    let major = parts[0].parse::<u32>().ok()?;
    let minor = parts[1].parse::<u32>().ok()?;
    Some((major, minor))
}

/// Deserialize GLib version from string to (major, minor) tuple
fn deserialize_glib_version<'de, D>(deserializer: D) -> Result<Option<(u32, u32)>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de;

    let version_str: Option<String> = Option::deserialize(deserializer)?;

    match version_str {
        Some(s) => parse_glib_version(&s).map(Some).ok_or_else(|| {
            de::Error::custom(format!(
                "Invalid GLib version format: '{}'. Expected format: 'major.minor' (e.g., '2.76')",
                s
            ))
        }),
        None => Ok(None),
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub rules: RulesConfig,

    #[serde(default)]
    pub ignore: Vec<String>,

    /// Minimum supported GLib version as (major, minor)
    /// Rules requiring newer GLib versions will be automatically disabled
    #[serde(default, deserialize_with = "deserialize_glib_version")]
    pub min_glib_version: Option<(u32, u32)>,

    /// Target MSVC-compatible code
    #[serde(default)]
    pub msvc_compatible: bool,

    /// Output format
    pub format: Option<OutputFormat>,

    /// Editor URL format for clickable links
    /// Available placeholders: {path}, {line}, {column}
    /// Examples:
    ///   VSCode: "vscode://file{path}:{line}:{column}"
    ///   IntelliJ: "idea://open?file={path}&line={line}"
    ///   Sublime: "subl://open?url=file://{path}&line={line}&column={column}"
    pub editor_url: Option<String>,
}

/// Rule severity level
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum RuleLevel {
    /// Report as error and exit with failure code
    Error,
    /// Report as warning but don't fail
    Warn,
    /// Disabled/ignored
    Ignore,
}

impl RuleLevel {
    pub fn is_enabled(&self) -> bool {
        !matches!(self, RuleLevel::Ignore)
    }

    pub fn is_error(&self) -> bool {
        matches!(self, RuleLevel::Error)
    }

    pub fn is_warn(&self) -> bool {
        matches!(self, RuleLevel::Warn)
    }
}

/// Per-rule configuration
#[derive(Debug, Clone)]
pub struct RuleConfig {
    pub level: RuleLevel,
    pub ignore: Vec<String>,
    /// Rule-specific options (e.g., config_header for include_order)
    pub options: HashMap<String, toml::Value>,
}

impl Default for RuleConfig {
    fn default() -> Self {
        Self {
            level: RuleLevel::Warn,
            ignore: Vec::new(),
            options: HashMap::new(),
        }
    }
}

impl<'de> Deserialize<'de> for RuleConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use std::fmt;

        use serde::de::{self, MapAccess, Visitor};

        struct RuleConfigVisitor;

        impl<'de> Visitor<'de> for RuleConfigVisitor {
            type Value = RuleConfig;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter
                    .write_str("a boolean, \"error\"/\"warn\"/\"ignore\", or a RuleConfig struct")
            }

            fn visit_bool<E>(self, value: bool) -> Result<RuleConfig, E>
            where
                E: de::Error,
            {
                Ok(RuleConfig {
                    level: if value {
                        RuleLevel::Error
                    } else {
                        RuleLevel::Ignore
                    },
                    ignore: Vec::new(),
                    options: HashMap::new(),
                })
            }

            fn visit_str<E>(self, value: &str) -> Result<RuleConfig, E>
            where
                E: de::Error,
            {
                let level = match value {
                    "error" => RuleLevel::Error,
                    "warn" => RuleLevel::Warn,
                    "ignore" => RuleLevel::Ignore,
                    _ => {
                        return Err(de::Error::unknown_variant(
                            value,
                            &["error", "warn", "ignore"],
                        ));
                    }
                };
                Ok(RuleConfig {
                    level,
                    ignore: Vec::new(),
                    options: HashMap::new(),
                })
            }

            fn visit_map<M>(self, mut map: M) -> Result<RuleConfig, M::Error>
            where
                M: MapAccess<'de>,
            {
                let mut level = None;
                let mut ignore = None;
                let mut options = HashMap::new();

                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "level" => {
                            if level.is_some() {
                                return Err(de::Error::duplicate_field("level"));
                            }
                            let level_str: String = map.next_value()?;
                            level = Some(match level_str.as_str() {
                                "error" => RuleLevel::Error,
                                "warn" => RuleLevel::Warn,
                                "ignore" => RuleLevel::Ignore,
                                _ => {
                                    return Err(de::Error::unknown_variant(
                                        &level_str,
                                        &["error", "warn", "ignore"],
                                    ));
                                }
                            });
                        }
                        "ignore" => {
                            if ignore.is_some() {
                                return Err(de::Error::duplicate_field("ignore"));
                            }
                            ignore = Some(map.next_value()?);
                        }
                        _ => {
                            // Capture unknown fields as rule-specific options
                            let value: toml::Value = map.next_value()?;
                            options.insert(key, value);
                        }
                    }
                }

                Ok(RuleConfig {
                    level: level.unwrap_or(RuleLevel::Error),
                    ignore: ignore.unwrap_or_default(),
                    options,
                })
            }
        }

        deserializer.deserialize_any(RuleConfigVisitor)
    }
}

macro_rules! impl_rules_config {
    ($(($config_field:ident, $rule_type:ident, $major:literal, $minor:literal, $requires_auto_cleanup:literal)),* $(,)?) => {
        #[derive(Debug, Clone, Deserialize, Default)]
        pub struct RulesConfig {
            $(
                #[serde(default)]
                pub $config_field: RuleConfig,
            )*
        }
    };
}

crate::for_each_rule!(impl_rules_config);

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            // Return default config if file doesn't exist
            return Ok(Config::default());
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

    /// Build an ignore matcher for a specific rule, combining global and
    /// per-rule ignores
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

    /// Get reference to a rule config by field name
    pub fn get_rule_config(&self, field_name: &str) -> Option<&RuleConfig> {
        macro_rules! impl_get_rule_config {
            ($(($config_field:ident, $rule_type:ident, $major:literal, $minor:literal, $requires_auto_cleanup:literal)),* $(,)?) => {
                match field_name {
                    $(
                        stringify!($config_field) => Some(&self.rules.$config_field),
                    )*
                    _ => None,
                }
            };
        }

        crate::for_each_rule!(impl_get_rule_config)
    }

    /// Get mutable reference to a rule config by field name
    pub fn get_rule_config_mut(&mut self, field_name: &str) -> Option<&mut RuleConfig> {
        macro_rules! impl_get_rule_config_mut {
            ($(($config_field:ident, $rule_type:ident, $major:literal, $minor:literal, $requires_auto_cleanup:literal)),* $(,)?) => {
                match field_name {
                    $(
                        stringify!($config_field) => Some(&mut self.rules.$config_field),
                    )*
                    _ => None,
                }
            };
        }

        crate::for_each_rule!(impl_get_rule_config_mut)
    }

    /// Enable only specific rules, disabling all others
    pub fn enable_only_rules(&mut self, rule_names: &[String]) -> Result<()> {
        // First, validate that all provided rule names exist
        let valid_rules: Vec<&str> = {
            macro_rules! collect_rule_names {
                ($(($config_field:ident, $rule_type:ident, $major:literal, $minor:literal, $requires_auto_cleanup:literal)),* $(,)?) => {
                    vec![$(stringify!($config_field)),*]
                };
            }
            crate::for_each_rule!(collect_rule_names)
        };

        for rule_name in rule_names {
            if !valid_rules.contains(&rule_name.as_str()) {
                anyhow::bail!("Unknown rule: {}", rule_name);
            }
        }

        // Now enable only the specified rules
        macro_rules! impl_enable_only_rules {
            ($(($config_field:ident, $rule_type:ident, $major:literal, $minor:literal, $requires_auto_cleanup:literal)),* $(,)?) => {
                {
                    $(
                        self.rules.$config_field.level = if rule_names.iter().any(|r| r == stringify!($config_field)) {
                            RuleLevel::Error
                        } else {
                            RuleLevel::Ignore
                        };
                    )*
                }
            };
        }

        crate::for_each_rule!(impl_enable_only_rules);
        Ok(())
    }

    /// Disable specific rules (all others remain enabled according to config)
    pub fn disable_rules(&mut self, rule_names: &[String]) -> Result<()> {
        // First, validate that all provided rule names exist
        let valid_rules: Vec<&str> = {
            macro_rules! collect_rule_names {
                ($(($config_field:ident, $rule_type:ident, $major:literal, $minor:literal, $requires_auto_cleanup:literal)),* $(,)?) => {
                    vec![$(stringify!($config_field)),*]
                };
            }
            crate::for_each_rule!(collect_rule_names)
        };

        for rule_name in rule_names {
            if !valid_rules.contains(&rule_name.as_str()) {
                anyhow::bail!("Unknown rule: {}", rule_name);
            }
        }

        // Now disable the specified rules
        macro_rules! impl_disable_rules {
            ($(($config_field:ident, $rule_type:ident, $major:literal, $minor:literal, $requires_auto_cleanup:literal)),* $(,)?) => {
                {
                    $(
                        if rule_names.iter().any(|r| r == stringify!($config_field)) {
                            self.rules.$config_field.level = RuleLevel::Ignore;
                        }
                    )*
                }
            };
        }

        crate::for_each_rule!(impl_disable_rules);
        Ok(())
    }

    /// Filter rules by category, disabling all others
    pub fn filter_by_category(&mut self, category: crate::rules::Category) -> Result<()> {
        macro_rules! impl_filter_by_category {
            ($(($config_field:ident, $rule_type:ident, $major:literal, $minor:literal, $requires_auto_cleanup:literal)),* $(,)?) => {
                {
                    $(
                        self.rules.$config_field.level = if $rule_type.category() == category {
                            RuleLevel::Error
                        } else {
                            RuleLevel::Ignore
                        };
                    )*
                }
            };
        }

        crate::for_each_rule!(impl_filter_by_category);
        Ok(())
    }
}
