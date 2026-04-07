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
use crate::rules::unnecessary_null_check::UnnecessaryNullCheck;
use crate::rules::use_clear_functions::UseClearFunctions;
use crate::rules::use_g_strcmp0::UseGStrcmp0;
use crate::rules::Violation;
use anyhow::Result;
use indicatif::ProgressBar;
use std::path::Path;

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

/// New AST-based scanner - much simpler than the old one!
pub fn scan_with_ast(
    ast_context: &AstContext,
    config: &Config,
    project_root: &Path,
    spinner: Option<&ProgressBar>,
) -> Result<Vec<Violation>> {
    let mut violations = Vec::new();

    if let Some(sp) = spinner {
        sp.set_message("Running linter rules...");
    }

    // Run G_DECLARE semicolon checks
    if config.rules.gdeclare_semicolon.enabled {
        let start = violations.len();
        let rule = GDeclareSemicolon;
        rule.check_all(ast_context, config, &mut violations);
        filter_violations_in_place(
            &mut violations,
            start,
            project_root,
            config,
            &config.rules.gdeclare_semicolon,
        )?;
    }

    // Run missing implementation checks
    if config.rules.missing_implementation.enabled {
        let start = violations.len();
        let rule = MissingImplementation;
        rule.check_all(ast_context, config, &mut violations);
        filter_violations_in_place(
            &mut violations,
            start,
            project_root,
            config,
            &config.rules.missing_implementation,
        )?;
    }

    // Run deprecated API checks
    if config.rules.deprecated_add_private.enabled {
        let start = violations.len();
        let rule = DeprecatedAddPrivate;
        rule.check_all(ast_context, config, &mut violations);
        filter_violations_in_place(
            &mut violations,
            start,
            project_root,
            config,
            &config.rules.deprecated_add_private,
        )?;
    }

    // Run string comparison checks
    if config.rules.use_g_strcmp0.enabled {
        let start = violations.len();
        let rule = UseGStrcmp0;
        rule.check_all(ast_context, config, &mut violations);
        filter_violations_in_place(
            &mut violations,
            start,
            project_root,
            config,
            &config.rules.use_g_strcmp0,
        )?;
    }

    // Run g_param_spec checks
    if config.rules.g_param_spec_null_nick_blurb.enabled {
        let start = violations.len();
        let rule = GParamSpecNullNickBlurb;
        rule.check_all(ast_context, config, &mut violations);
        filter_violations_in_place(
            &mut violations,
            start,
            project_root,
            config,
            &config.rules.g_param_spec_null_nick_blurb,
        )?;
    }

    // Run GError initialization checks
    if config.rules.gerror_init.enabled {
        let start = violations.len();
        let rule = GErrorInit;
        rule.check_all(ast_context, config, &mut violations);
        filter_violations_in_place(
            &mut violations,
            start,
            project_root,
            config,
            &config.rules.gerror_init,
        )?;
    }

    // Run property enum checks
    if config.rules.property_enum_zero.enabled {
        let start = violations.len();
        let rule = PropertyEnumZero;
        rule.check_all(ast_context, config, &mut violations);
        filter_violations_in_place(
            &mut violations,
            start,
            project_root,
            config,
            &config.rules.property_enum_zero,
        )?;
    }

    // Run dispose/finalize chain-up checks
    if config.rules.dispose_finalize_chains_up.enabled {
        let start = violations.len();
        let rule = DisposeFinalizeChainsUp;
        rule.check_all(ast_context, config, &mut violations);
        filter_violations_in_place(
            &mut violations,
            start,
            project_root,
            config,
            &config.rules.dispose_finalize_chains_up,
        )?;
    }

    // Run GTask source tag checks
    if config.rules.gtask_source_tag.enabled {
        let start = violations.len();
        let rule = GTaskSourceTag;
        rule.check_all(ast_context, config, &mut violations);
        filter_violations_in_place(
            &mut violations,
            start,
            project_root,
            config,
            &config.rules.gtask_source_tag,
        )?;
    }

    // Run unnecessary NULL check detection
    if config.rules.unnecessary_null_check.enabled {
        let start = violations.len();
        let rule = UnnecessaryNullCheck;
        rule.check_all(ast_context, config, &mut violations);
        filter_violations_in_place(
            &mut violations,
            start,
            project_root,
            config,
            &config.rules.unnecessary_null_check,
        )?;
    }

    // Run use clear functions checks
    if config.rules.use_clear_functions.enabled {
        let start = violations.len();
        let rule = UseClearFunctions;
        rule.check_all(ast_context, config, &mut violations);
        filter_violations_in_place(
            &mut violations,
            start,
            project_root,
            config,
            &config.rules.use_clear_functions,
        )?;
    }

    Ok(violations)
}
