use crate::ast_context::AstContext;
use crate::config::Config;
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

/// New AST-based scanner - much simpler than the old one!
pub fn scan_with_ast(
    ast_context: &AstContext,
    config: &Config,
    spinner: Option<&ProgressBar>,
) -> Result<Vec<Violation>> {
    let mut violations = Vec::new();

    if let Some(sp) = spinner {
        sp.set_message("Running linter rules...");
    }

    // Run G_DECLARE semicolon checks
    let rule = GDeclareSemicolon;
    violations.extend(rule.check_all(ast_context, config));

    // Run missing implementation checks
    let rule = MissingImplementation;
    violations.extend(rule.check_all(ast_context, config));

    // Run deprecated API checks
    let rule = DeprecatedAddPrivate;
    violations.extend(rule.check_all(ast_context, config));

    // Run string comparison checks
    let rule = UseGStrcmp0;
    violations.extend(rule.check_all(ast_context, config));

    // Run g_param_spec checks
    let rule = GParamSpecNullNickBlurb;
    violations.extend(rule.check_all(ast_context, config));

    // Run GError initialization checks
    let rule = GErrorInit;
    violations.extend(rule.check_all(ast_context, config));

    // Run property enum checks
    let rule = PropertyEnumZero;
    violations.extend(rule.check_all(ast_context, config));

    // Run dispose/finalize chain-up checks
    let rule = DisposeFinalizeChainsUp;
    violations.extend(rule.check_all(ast_context, config));

    // Run GTask source tag checks
    let rule = GTaskSourceTag;
    violations.extend(rule.check_all(ast_context, config));

    // Run unnecessary NULL check detection
    let rule = UnnecessaryNullCheck;
    violations.extend(rule.check_all(ast_context, config));

    // Run use clear functions checks
    let rule = UseClearFunctions;
    violations.extend(rule.check_all(ast_context, config));

    Ok(violations)
}
