use std::{fs, path::Path};

use anyhow::Result;
use colored::Colorize;
use indicatif::ProgressBar;

use crate::{
    ast_context::AstContext,
    config::{Config, RuleConfig},
    rules::{
        Rule, Violation, chainup::DisposeFinalizeChainsUp,
        deprecated_add_private::DeprecatedAddPrivate,
        g_param_spec_null_nick_blurb::GParamSpecNullNickBlurb,
        gdeclare_semicolon::GDeclareSemicolon, gerror_init::GErrorInit,
        gtask_source_tag::GTaskSourceTag, missing_implementation::MissingImplementation,
        prefer_g_new::PreferGNew, prefer_g_variant_new_typed::PreferGVariantNewTyped,
        property_enum_zero::PropertyEnumZero, strcmp_equal::StrcmpForStringEqual,
        suggest_g_autofree::SuggestGAutofree, suggest_g_autoptr_goto::SuggestGAutoptrGoto,
        suggest_g_autoptr_inline::SuggestGAutoptrInline,
        unnecessary_null_check::UnnecessaryNullCheck, use_clear_functions::UseClearFunctions,
        use_g_clear_error::SuggestGAutoptrError, use_g_clear_handle_id::UseGClearHandleId,
        use_g_clear_list::UseGClearList, use_g_file_load_bytes::UseGFileLoadBytes,
        use_g_object_new_with_properties::UseGObjectNewWithProperties,
        use_g_object_notify_by_pspec::UseGObjectNotifyByPspec, use_g_set_str::UseGSetStr,
        use_g_strcmp0::UseGStrcmp0, use_g_string_free_and_steal::UseGStringFreeAndSteal,
    },
};

/// Extract a source snippet from a file at the given line
fn get_source_snippet(file_path: &Path, line: usize) -> Option<String> {
    let content = fs::read_to_string(file_path).ok()?;
    content
        .lines()
        .nth(line.saturating_sub(1))
        .map(|s| s.trim().to_string())
}

/// Populate snippets for violations that don't have them
fn populate_snippets(violations: &mut [Violation], start_index: usize) {
    for violation in violations.iter_mut().skip(start_index) {
        if violation.snippet.is_none() {
            let path = Path::new(&violation.file);
            violation.snippet = get_source_snippet(path, violation.line);
        }
    }
}

/// Filter violations in-place based on per-rule ignore patterns
/// Only filters violations added after `start_index`
fn filter_violations_in_place(
    violations: &mut Vec<Violation>,
    start_index: usize,
    project_root: &Path,
    config: &Config,
    rule_config: &RuleConfig,
) -> Result<()> {
    let ignore_matcher = config.build_rule_ignore_matcher(rule_config)?;

    // Keep all violations before start_index, and filter the new ones
    let mut i = start_index;
    while i < violations.len() {
        let path = Path::new(&violations[i].file);

        // Try to make path relative to project root for matching
        let relative_path = path.strip_prefix(project_root).unwrap_or(path);

        if ignore_matcher.is_match(relative_path) {
            violations.remove(i);
        } else {
            i += 1;
        }
    }

    Ok(())
}

struct RuleEntry {
    rule: Box<dyn Rule>,
    enabled: bool,
    rule_config: RuleConfig,
}

/// Macro to define all rules in execution order with their minimum GLib version
/// requirements
#[macro_export]
macro_rules! for_each_rule {
    ($callback:ident) => {
        $callback! {
            (gdeclare_semicolon, GDeclareSemicolon, 2, 0),
            (missing_implementation, MissingImplementation, 2, 0),
            (deprecated_add_private, DeprecatedAddPrivate, 2, 0),
            (prefer_g_new, PreferGNew, 2, 0),
            (prefer_g_variant_new_typed, PreferGVariantNewTyped, 2, 24),
            (use_g_strcmp0, UseGStrcmp0, 2, 16),
            (use_clear_functions, UseClearFunctions, 2, 28),
            (g_param_spec_null_nick_blurb, GParamSpecNullNickBlurb, 2, 0),
            (gerror_init, GErrorInit, 2, 0),
            (property_enum_zero, PropertyEnumZero, 2, 0),
            (dispose_finalize_chains_up, DisposeFinalizeChainsUp, 2, 0),
            (gtask_source_tag, GTaskSourceTag, 2, 36),
            (unnecessary_null_check, UnnecessaryNullCheck, 2, 0),
            (strcmp_for_string_equal, StrcmpForStringEqual, 2, 0),
            (use_g_set_str, UseGSetStr, 2, 76),
            (suggest_g_autoptr_error, SuggestGAutoptrError, 2, 44),
            (suggest_g_autoptr_goto_cleanup, SuggestGAutoptrGoto, 2, 44),
            (suggest_g_autoptr_inline_cleanup, SuggestGAutoptrInline, 2, 44),
            (suggest_g_autofree, SuggestGAutofree, 2, 44),
            (use_g_clear_handle_id, UseGClearHandleId, 2, 56),
            (use_g_clear_list, UseGClearList, 2, 64),
            (use_g_file_load_bytes, UseGFileLoadBytes, 2, 56),
            (use_g_object_new_with_properties, UseGObjectNewWithProperties, 2, 0),
            (use_g_object_notify_by_pspec, UseGObjectNotifyByPspec, 2, 26),
            (use_g_string_free_and_steal, UseGStringFreeAndSteal, 2, 76),
        }
    };
}

macro_rules! impl_create_all_rules {
    ($(($config_field:ident, $rule_type:ident, $major:literal, $minor:literal)),* $(,)?) => {
        /// Create all rule instances in execution order
        fn create_all_rules(config: &Config) -> Vec<RuleEntry> {
            vec![
                $(
                    RuleEntry {
                        rule: Box::new($rule_type),
                        enabled: config.rules.$config_field.enabled && is_rule_compatible(config, $major, $minor),
                        rule_config: config.rules.$config_field.clone(),
                    },
                )*
            ]
        }
    };
}

/// Check if a rule is compatible with the configured minimum GLib version
fn is_rule_compatible(config: &Config, required_major: u32, required_minor: u32) -> bool {
    if let Some((major, minor)) = config.min_glib_version {
        // Compare versions: config version must be >= required version
        (major > required_major) || (major == required_major && minor >= required_minor)
    } else {
        // No minimum version set, all rules are compatible
        true
    }
}

for_each_rule!(impl_create_all_rules);

/// New AST-based scanner - much simpler than the old one!
pub fn scan_with_ast(
    ast_context: &AstContext,
    config: &Config,
    project_root: &Path,
    spinner: Option<&ProgressBar>,
) -> Result<Vec<Violation>> {
    let mut violations = Vec::new();

    // Register all rules in execution order
    let rules = create_all_rules(config);

    if let Some(sp) = spinner {
        sp.set_message("Running linter rules...");
    }

    // Run all registered rules
    for (rule_index, entry) in rules.iter().enumerate() {
        if !entry.enabled {
            continue;
        }

        let start = violations.len();
        entry.rule.check_all(ast_context, config, &mut violations);

        // Set rule index for precedence
        for violation in violations.iter_mut().skip(start) {
            violation.rule_index = rule_index;
        }

        populate_snippets(&mut violations, start);
        filter_violations_in_place(
            &mut violations,
            start,
            project_root,
            config,
            &entry.rule_config,
        )?;
    }

    // Deduplicate: keep only violations from later rules (higher index) when
    // multiple rules fire on same line
    deduplicate_by_rule_precedence(&mut violations);

    Ok(violations)
}

/// List all available rules with their descriptions
pub fn list_all_rules(config: &Config) {
    let rules = create_all_rules(config);

    let fixable_count = rules.iter().filter(|e| e.rule.fixable()).count();

    println!(
        "{} {}",
        "Available lint rules".bold(),
        format!("({} total, {} auto-fixable)", rules.len(), fixable_count).dimmed()
    );

    for entry in &rules {
        let status = if entry.enabled {
            "✓".green()
        } else {
            "✗".red()
        };
        let name = entry.rule.name().cyan().bold();
        let desc = entry.rule.description().dimmed();
        let fixable = if entry.rule.fixable() {
            format!(" {}", "[auto-fix]".yellow())
        } else {
            "".to_string()
        };
        println!("  {} {}{} - {}", status, name, fixable, desc);
    }
}

/// Keep only the violation with the highest rule_index for each (file, line)
/// pair
fn deduplicate_by_rule_precedence(violations: &mut Vec<Violation>) {
    use std::collections::HashMap;

    // Group violations by (file, line), keeping the one with highest rule_index
    let mut best: HashMap<(std::path::PathBuf, usize), usize> = HashMap::new();

    for (i, v) in violations.iter().enumerate() {
        let key = (v.file.clone(), v.line);
        match best.get(&key) {
            Some(&existing_idx) => {
                if v.rule_index > violations[existing_idx].rule_index {
                    best.insert(key, i);
                }
            }
            None => {
                best.insert(key, i);
            }
        }
    }

    // Keep only the violations that are in best
    let best_indices: std::collections::HashSet<_> = best.values().copied().collect();
    let mut i = 0;
    violations.retain(|_| {
        let keep = best_indices.contains(&i);
        i += 1;
        keep
    });
}
