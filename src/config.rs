use std::{fs, path::Path};

use anyhow::{Context, Result};
use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::Deserialize;

use crate::rules::*;

/// Parse a GLib version string like "2.76" into (major, minor)
fn parse_glib_version(version: &str) -> Option<(u32, u32)> {
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

    /// Editor URL format for clickable links
    /// Available placeholders: {path}, {line}, {column}
    /// Examples:
    ///   VSCode: "vscode://file{path}:{line}:{column}"
    ///   IntelliJ: "idea://open?file={path}&line={line}"
    ///   Sublime: "subl://open?url=file://{path}&line={line}&column={column}"
    pub editor_url: Option<String>,
}

/// Per-rule configuration
#[derive(Debug, Clone)]
pub struct RuleConfig {
    pub enabled: bool,
    pub ignore: Vec<String>,
}

impl Default for RuleConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            ignore: Vec::new(),
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
    pub g_param_spec_static_strings: RuleConfig,

    #[serde(default)]
    pub dispose_finalize_chains_up: RuleConfig,

    #[serde(default)]
    pub use_clear_functions: RuleConfig,

    #[serde(default)]
    pub use_explicit_default_flags: RuleConfig,

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
    pub matching_declare_define: RuleConfig,

    #[serde(default)]
    pub prefer_g_new: RuleConfig,

    #[serde(default)]
    pub prefer_g_object_class_install_properties: RuleConfig,

    #[serde(default)]
    pub prefer_g_settings_typed: RuleConfig,

    #[serde(default)]
    pub prefer_g_value_set_static_string: RuleConfig,

    #[serde(default)]
    pub prefer_g_variant_new_typed: RuleConfig,

    #[serde(default)]
    pub strcmp_for_string_equal: RuleConfig,

    #[serde(default)]
    pub use_g_set_str: RuleConfig,

    #[serde(default)]
    pub suggest_g_autoptr_error: RuleConfig,

    #[serde(default)]
    pub suggest_g_autoptr_goto_cleanup: RuleConfig,

    #[serde(default)]
    pub suggest_g_autoptr_inline_cleanup: RuleConfig,

    #[serde(default)]
    pub suggest_g_autofree: RuleConfig,

    #[serde(default)]
    pub use_g_clear_handle_id: RuleConfig,

    #[serde(default)]
    pub use_g_clear_list: RuleConfig,

    #[serde(default)]
    pub use_g_clear_weak_pointer: RuleConfig,

    #[serde(default)]
    pub use_g_file_load_bytes: RuleConfig,

    #[serde(default)]
    pub use_g_object_new_with_properties: RuleConfig,

    #[serde(default)]
    pub use_g_object_notify_by_pspec: RuleConfig,

    #[serde(default)]
    pub use_g_string_free_and_steal: RuleConfig,
}

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

    /// Get mutable reference to a rule config by field name
    pub fn get_rule_config_mut(&mut self, field_name: &str) -> Option<&mut RuleConfig> {
        macro_rules! impl_get_rule_config_mut {
            ($(($config_field:ident, $rule_type:ident, $major:literal, $minor:literal)),* $(,)?) => {
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
    pub fn enable_only_rules(&mut self, rule_names: &[String]) {
        macro_rules! impl_enable_only_rules {
            ($(($config_field:ident, $rule_type:ident, $major:literal, $minor:literal)),* $(,)?) => {
                {
                    $(
                        self.rules.$config_field.enabled = rule_names.iter().any(|r| r == stringify!($config_field));
                    )*
                }
            };
        }

        crate::for_each_rule!(impl_enable_only_rules);
    }

    /// Filter rules by category, disabling all others
    pub fn filter_by_category(&mut self, category: crate::rules::Category) -> Result<()> {
        macro_rules! impl_filter_by_category {
            ($(($config_field:ident, $rule_type:ident, $major:literal, $minor:literal)),* $(,)?) => {
                {
                    $(
                        self.rules.$config_field.enabled = $rule_type.category() == category;
                    )*
                }
            };
        }

        crate::for_each_rule!(impl_filter_by_category);
        Ok(())
    }
}
