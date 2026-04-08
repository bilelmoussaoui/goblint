use crate::ast_context::AstContext;
use crate::config::{Config, RuleConfig};
use crate::rules::chainup::DisposeFinalizeChainsUp;
use crate::rules::deprecated_add_private::DeprecatedAddPrivate;
use crate::rules::g_param_spec::GParamSpecNullNickBlurb;
use crate::rules::gdeclare_semicolon::GDeclareSemicolon;
use crate::rules::gerror_init::GErrorInit;
use crate::rules::gtask_source_tag::GTaskSourceTag;
use crate::rules::missing_implementation::MissingImplementation;
use crate::rules::property_enum_zero::PropertyEnumZero;
use crate::rules::strcmp_equal::StrcmpForStringEqual;
use crate::rules::suggest_g_autoptr_goto::SuggestGAutoptrGoto;
use crate::rules::unnecessary_null_check::UnnecessaryNullCheck;
use crate::rules::use_clear_functions::UseClearFunctions;
use crate::rules::use_g_clear_error::SuggestGAutoptrError;
use crate::rules::use_g_set_str::UseGSetStr;
use crate::rules::use_g_strcmp0::UseGStrcmp0;
use crate::rules::{Rule, Violation};
use anyhow::Result;
use indicatif::ProgressBar;
use std::fs;
use std::path::Path;

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

/// New AST-based scanner - much simpler than the old one!
pub fn scan_with_ast(
    ast_context: &AstContext,
    config: &Config,
    project_root: &Path,
    spinner: Option<&ProgressBar>,
) -> Result<Vec<Violation>> {
    let mut violations = Vec::new();

    // Register all rules in execution order
    let rules: Vec<RuleEntry> = vec![
        RuleEntry {
            rule: Box::new(GDeclareSemicolon),
            enabled: config.rules.gdeclare_semicolon.enabled,
            rule_config: config.rules.gdeclare_semicolon.clone(),
        },
        RuleEntry {
            rule: Box::new(MissingImplementation),
            enabled: config.rules.missing_implementation.enabled,
            rule_config: config.rules.missing_implementation.clone(),
        },
        RuleEntry {
            rule: Box::new(DeprecatedAddPrivate),
            enabled: config.rules.deprecated_add_private.enabled,
            rule_config: config.rules.deprecated_add_private.clone(),
        },
        RuleEntry {
            rule: Box::new(UseGStrcmp0),
            enabled: config.rules.use_g_strcmp0.enabled,
            rule_config: config.rules.use_g_strcmp0.clone(),
        },
        RuleEntry {
            rule: Box::new(UseClearFunctions),
            enabled: config.rules.use_clear_functions.enabled,
            rule_config: config.rules.use_clear_functions.clone(),
        },
        RuleEntry {
            rule: Box::new(GParamSpecNullNickBlurb),
            enabled: config.rules.g_param_spec_null_nick_blurb.enabled,
            rule_config: config.rules.g_param_spec_null_nick_blurb.clone(),
        },
        RuleEntry {
            rule: Box::new(GErrorInit),
            enabled: config.rules.gerror_init.enabled,
            rule_config: config.rules.gerror_init.clone(),
        },
        RuleEntry {
            rule: Box::new(PropertyEnumZero),
            enabled: config.rules.property_enum_zero.enabled,
            rule_config: config.rules.property_enum_zero.clone(),
        },
        RuleEntry {
            rule: Box::new(DisposeFinalizeChainsUp),
            enabled: config.rules.dispose_finalize_chains_up.enabled,
            rule_config: config.rules.dispose_finalize_chains_up.clone(),
        },
        RuleEntry {
            rule: Box::new(GTaskSourceTag),
            enabled: config.rules.gtask_source_tag.enabled,
            rule_config: config.rules.gtask_source_tag.clone(),
        },
        RuleEntry {
            rule: Box::new(UnnecessaryNullCheck),
            enabled: config.rules.unnecessary_null_check.enabled,
            rule_config: config.rules.unnecessary_null_check.clone(),
        },
        RuleEntry {
            rule: Box::new(StrcmpForStringEqual),
            enabled: config.rules.strcmp_for_string_equal.enabled,
            rule_config: config.rules.strcmp_for_string_equal.clone(),
        },
        RuleEntry {
            rule: Box::new(UseGSetStr),
            enabled: config.rules.use_g_set_str.enabled,
            rule_config: config.rules.use_g_set_str.clone(),
        },
        RuleEntry {
            rule: Box::new(SuggestGAutoptrError),
            enabled: config.rules.suggest_g_autoptr_error.enabled,
            rule_config: config.rules.suggest_g_autoptr_error.clone(),
        },
        RuleEntry {
            rule: Box::new(SuggestGAutoptrGoto),
            enabled: config.rules.suggest_g_autoptr_goto_cleanup.enabled,
            rule_config: config.rules.suggest_g_autoptr_goto_cleanup.clone(),
        },
    ];

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

    // Deduplicate: keep only violations from later rules (higher index) when multiple rules fire on same line
    deduplicate_by_rule_precedence(&mut violations);

    Ok(violations)
}

/// Keep only the violation with the highest rule_index for each (file, line) pair
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
