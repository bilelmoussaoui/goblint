use std::{fs, path::Path};

use anyhow::Result;
use colored::Colorize;
use indicatif::ProgressBar;
use rayon::prelude::*;

use crate::{
    ast_context::AstContext,
    config::{Config, RuleConfig},
    rules::*,
};

/// Extract a source snippet from a file at the given line with context
fn get_source_snippet(file_path: &Path, line: usize) -> Option<String> {
    let content = fs::read_to_string(file_path).ok()?;
    let lines: Vec<&str> = content.lines().collect();

    if line == 0 || line > lines.len() {
        return None;
    }

    // Get 7 lines before and 3 lines after for context (11 lines total)
    let start_line = line.saturating_sub(8); // -1 for 0-indexing, -7 for context
    let end_line = (line + 3).min(lines.len());

    let mut snippet_lines = Vec::new();
    let mut last_was_collapsed = false;

    for (i, line_text) in lines.iter().enumerate().take(end_line).skip(start_line) {
        let trimmed = line_text.trim();
        let is_target_line = i + 1 == line;

        // Check if line is just braces/whitespace (but always show target line)
        let is_noise = !is_target_line && matches!(trimmed, "" | "{" | "}" | "{}" | "};");

        if is_noise {
            // Collapse consecutive noise lines into ...
            if !last_was_collapsed {
                snippet_lines.push("...".to_string());
                last_was_collapsed = true;
            }
        } else {
            let prefix = if is_target_line { ">" } else { "" };
            snippet_lines.push(format!("{}{}", prefix, line_text));
            last_was_collapsed = false;
        }
    }

    Some(snippet_lines.join("\n"))
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

pub struct RuleEntry {
    pub rule: Box<dyn Rule>,
    pub level: crate::config::RuleLevel,
    pub rule_config: RuleConfig,
}

/// Macro to define all rules in execution order with their minimum GLib version
/// requirements
#[macro_export]
macro_rules! for_each_rule {
    ($callback:ident) => {
        $callback! {
            (g_declare_semicolon, GDeclareSemicolon, 2, 0),
            (include_order, IncludeOrder, 2, 0),
            (use_pragma_once, UsePragmaOnce, 2, 0),
            (missing_implementation, MissingImplementation, 2, 0),
            (deprecated_add_private, DeprecatedAddPrivate, 2, 0),
            (matching_declare_define, MatchingDeclareDefine, 2, 70),
            (use_g_new, UseGNew, 2, 0),
            (use_g_object_class_install_properties, UseGObjectClassInstallProperties, 2, 26),
            (use_g_settings_typed, UseGSettingsTyped, 2, 26),
            (use_g_value_set_static_string, UseGValueSetStaticString, 2, 0),
            (use_g_variant_new_typed, UseGVariantNewTyped, 2, 24),
            (strcmp_explicit_comparison, StrcmpExplicitComparison, 2, 0),
            (use_g_strcmp0, UseGStrcmp0, 2, 16),
            (use_clear_functions, UseClearFunctions, 2, 28),
            (use_explicit_default_flags, UseExplicitDefaultFlags, 2, 0),
            (g_param_spec_null_nick_blurb, GParamSpecNullNickBlurb, 2, 0),
            (g_param_spec_static_strings, GParamSpecStaticStrings, 2, 0),
            (g_param_spec_static_name_canonical, GParamSpecStaticNameCanonical, 2, 0),
            (g_error_init, GErrorInit, 2, 0),
            (g_error_leak, GErrorLeak, 2, 0),
            (g_source_id_not_stored, GSourceIdNotStored, 2, 0),
            (property_enum_convention, PropertyEnumConvention, 2, 0),
            (property_enum_coverage, PropertyEnumCoverage, 2, 0),
            (g_object_virtual_methods_chain_up, GObjectVirtualMethodsChainUp, 2, 0),
            (g_task_source_tag, GTaskSourceTag, 2, 36),
            (unnecessary_null_check, UnnecessaryNullCheck, 2, 0),
            (use_g_set_str, UseGSetStr, 2, 76),
            (use_g_autoptr_error, UseGAutoptrError, 2, 44),
            (use_g_autoptr_goto_cleanup, UseGAutoptrGotoCleanup, 2, 44),
            (use_g_autoptr_inline_cleanup, UseGAutoptrInlineCleanup, 2, 44),
            (use_g_autofree, UseGAutofree, 2, 44),
            (use_g_bytes_unref_to_data, UseGBytesUnrefToData, 2, 32),
            (use_g_clear_handle_id, UseGClearHandleId, 2, 56),
            (use_g_clear_list, UseGClearList, 2, 64),
            (use_g_clear_signal_handler, UseGClearSignalHandler, 2, 0),
            (use_g_clear_weak_pointer, UseGClearWeakPointer, 2, 56),
            (use_g_file_load_bytes, UseGFileLoadBytes, 2, 56),
            (use_g_object_new_with_properties, UseGObjectNewWithProperties, 2, 0),
            (use_g_object_notify_by_pspec, UseGObjectNotifyByPspec, 2, 26),
            (use_g_string_free_and_steal, UseGStringFreeAndSteal, 2, 76),
            (use_g_source_once, UseGSourceOnce, 2, 74),
            (use_g_source_constants, UseGSourceConstants, 2, 0),
            (use_g_steal_pointer, UseGStealPointer, 2, 0),
            (use_g_str_has_prefix_suffix, UseGStrHasPrefixSuffix, 2, 0),
            (use_g_ascii_functions, UseGAsciiFunctions, 2, 0),
            (use_g_strlcpy, UseGStrlcpy, 2, 0),
            (untranslated_string, UntranslatedString, 2, 0),
        }
    };
}

macro_rules! impl_create_all_rules {
    ($(($config_field:ident, $rule_type:ident, $major:literal, $minor:literal)),* $(,)?) => {
        /// Create all rule instances in execution order
        pub fn create_all_rules(config: &Config) -> Vec<RuleEntry> {
            vec![
                $(
                    RuleEntry {
                        rule: Box::new($rule_type),
                        level: if is_rule_compatible(config, $major, $minor) {
                            config.rules.$config_field.level
                        } else {
                            crate::config::RuleLevel::Ignore
                        },
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

/// Validate that all rule names in inline ignore directives are valid
/// Returns a list of warnings about unknown rules
fn validate_inline_ignores(
    inline_ignores: &std::collections::HashMap<
        &Path,
        std::collections::HashMap<usize, Vec<String>>,
    >,
    rules: &[RuleEntry],
    project_root: &Path,
) -> Vec<String> {
    use std::collections::HashSet;

    let mut warnings = Vec::new();

    // Collect all valid rule names
    let valid_rules: HashSet<String> = rules
        .iter()
        .map(|entry| entry.rule.name().to_string())
        .collect();

    // Check each file's ignore directives
    for (file_path, file_ignores) in inline_ignores {
        for (line_num, ignored_rules) in file_ignores {
            for rule_name in ignored_rules {
                // Skip wildcards
                if rule_name == "all" || rule_name == "*" {
                    continue;
                }

                // Check if rule exists
                if !valid_rules.contains(rule_name) {
                    let relative_path = file_path.strip_prefix(project_root).unwrap_or(file_path);
                    let warning = format!(
                        "{}:{}:1: {} Unknown rule '{}' in ignore directive",
                        relative_path.display(),
                        line_num,
                        "warning:".yellow(),
                        rule_name
                    );
                    warnings.push(warning);
                }
            }
        }
    }

    warnings
}

/// New AST-based scanner - much simpler than the old one!
pub fn scan_with_ast(
    ast_context: &AstContext,
    config: &Config,
    project_root: &Path,
    spinner: Option<&ProgressBar>,
) -> Result<Vec<Violation>> {
    let mut violations = Vec::new();

    // Parse inline ignore directives from all files
    let inline_ignores: std::collections::HashMap<
        &Path,
        std::collections::HashMap<usize, Vec<String>>,
    > = ast_context
        .project
        .files
        .iter()
        .map(|(path, file)| {
            let ignores = crate::inline_ignore::parse_ignore_directives(file);
            (path.as_path(), ignores)
        })
        .collect();

    // Register all rules in execution order
    let rules = create_all_rules(config);

    // Validate that all rule names in ignore directives are valid
    let warnings = validate_inline_ignores(&inline_ignores, &rules, project_root);
    for warning in warnings {
        eprintln!("{}", warning);
    }

    if let Some(sp) = spinner {
        sp.set_message("Running linter rules...");
    }

    // Run all rules in parallel — each gets its own violations vec
    let per_rule: Vec<Result<Vec<Violation>>> = rules
        .par_iter()
        .enumerate()
        .map(|(rule_index, entry)| {
            if !entry.level.is_enabled() {
                return Ok(Vec::new());
            }

            let mut rule_violations = Vec::new();
            entry
                .rule
                .check_all(ast_context, config, &mut rule_violations);

            for v in &mut rule_violations {
                v.rule_index = rule_index;
                v.level = entry.level;
            }

            populate_snippets(&mut rule_violations, 0);
            filter_violations_in_place(
                &mut rule_violations,
                0,
                project_root,
                config,
                &entry.rule_config,
            )?;

            Ok(rule_violations)
        })
        .collect();

    for rule_violations in per_rule {
        violations.extend(rule_violations?);
    }

    // Deduplicate: keep only violations from later rules (higher index) when
    // multiple rules fire on same line
    deduplicate_by_rule_precedence(&mut violations);

    // Filter out violations that have inline ignore directives
    violations.retain(|v| {
        !crate::inline_ignore::should_ignore_violation(&v.file, v.line, v.rule, &inline_ignores)
    });

    violations.sort_by(|a, b| {
        a.file
            .cmp(&b.file)
            .then(a.line.cmp(&b.line))
            .then(a.column.cmp(&b.column))
            .then(a.rule.cmp(b.rule))
    });

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
        let status = match entry.level {
            crate::config::RuleLevel::Error => "E".red().bold(),
            crate::config::RuleLevel::Warn => "W".yellow().bold(),
            crate::config::RuleLevel::Ignore => "-".dimmed(),
        };
        let name = entry.rule.name().cyan().bold();
        let category = format!("[{}]", entry.rule.category().as_str()).magenta();
        let desc = entry.rule.description().dimmed();
        let fixable = if entry.rule.fixable() {
            format!(" {}", "[auto-fix]".yellow())
        } else {
            "".to_string()
        };
        println!("  {} {} {}{} - {}", status, name, category, fixable, desc);
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
