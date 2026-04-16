use gobject_ast::CallExpression;

use super::{Fix, Rule};
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct GParamSpecNullNickBlurb;

impl Rule for GParamSpecNullNickBlurb {
    fn name(&self) -> &'static str {
        "g_param_spec_null_nick_blurb"
    }

    fn description(&self) -> &'static str {
        "Ensure g_param_spec_* functions have NULL for nick and blurb parameters"
    }

    fn category(&self) -> super::Category {
        super::Category::Pedantic
    }

    fn fixable(&self) -> bool {
        true
    }

    fn check_func_impl(
        &self,
        _ast_context: &AstContext,
        _config: &Config,
        func: &gobject_ast::FunctionInfo,
        path: &std::path::Path,
        violations: &mut Vec<Violation>,
    ) {
        if !func.is_definition {
            return;
        }

        // Find all g_param_spec_* calls (but skip g_param_spec_internal)
        for call in func.find_calls_matching(|name| {
            name.starts_with("g_param_spec_") && name != "g_param_spec_internal"
        }) {
            self.check_call(path, call, violations);
        }
    }
}

impl GParamSpecNullNickBlurb {
    fn check_call(
        &self,
        file_path: &std::path::Path,
        call: &CallExpression,
        violations: &mut Vec<Violation>,
    ) {
        // g_param_spec_*(name, nick, blurb, ...) — need at least 3 args
        if call.arguments.len() < 3 {
            return;
        }

        let nick_is_null = call.arguments[1].is_null();
        let blurb_is_null = call.arguments[2].is_null();

        // Collect which parameters need fixing
        let mut issues = Vec::new();
        if !nick_is_null {
            issues.push("nick (parameter 2)");
        }
        if !blurb_is_null {
            issues.push("blurb (parameter 3)");
        }

        if issues.is_empty() {
            return; // Already correct
        }

        // Create fix to replace non-NULL arguments with NULL
        let gobject_ast::Argument::Expression(nick_expr) = &call.arguments[1];
        let gobject_ast::Argument::Expression(blurb_expr) = &call.arguments[2];

        let string_fix = if !nick_is_null && !blurb_is_null {
            // Replace both nick and blurb with NULL
            Fix::new(
                nick_expr.location().start_byte,
                blurb_expr.location().end_byte,
                "NULL, NULL",
            )
        } else if !nick_is_null {
            // Replace only nick with NULL
            Fix::new(
                nick_expr.location().start_byte,
                nick_expr.location().end_byte,
                "NULL",
            )
        } else {
            // Replace only blurb with NULL
            Fix::new(
                blurb_expr.location().start_byte,
                blurb_expr.location().end_byte,
                "NULL",
            )
        };

        // Also fix the flags: after this rule runs, both nick and blurb will be NULL,
        // so remove STATIC_NICK, STATIC_BLURB, and STATIC_STRINGS, and ensure
        // STATIC_NAME is present (name is always a literal).
        let mut fixes = vec![string_fix];

        if call.arguments.len() >= 4 {
            let gobject_ast::Argument::Expression(flags_expr) = call.arguments.last().unwrap();
            let flags_identifiers = flags_expr.collect_identifiers();
            let flags_text = if flags_identifiers.is_empty() {
                "0".to_string()
            } else {
                flags_identifiers.join(" | ")
            };

            if let Some(new_flags) = self.compute_new_flags(&flags_text) {
                fixes.push(Fix::new(
                    flags_expr.location().start_byte,
                    flags_expr.location().end_byte,
                    new_flags,
                ));
            }
        }

        violations.push(self.violation_with_fixes(
            file_path,
            call.location.line,
            call.location.column,
            format!(
                "{} should have NULL for {}",
                call.function,
                issues.join(" and ")
            ),
            fixes,
        ));
    }

    /// After nick and blurb are set to NULL, compute the correct replacement
    /// flags string. Returns `None` if the flags are already correct.
    fn compute_new_flags(&self, flags_text: &str) -> Option<String> {
        const REMOVE: &[&str] = &[
            "G_PARAM_STATIC_NICK",
            "G_PARAM_STATIC_BLURB",
            "G_PARAM_STATIC_STRINGS",
        ];

        let parts: Vec<&str> = flags_text.split('|').map(|s| s.trim()).collect();
        let needs_removal = parts.iter().any(|p| REMOVE.contains(p));
        let has_name = parts.contains(&"G_PARAM_STATIC_NAME");

        if !needs_removal && has_name {
            return None;
        }

        let mut new_parts: Vec<&str> = parts.into_iter().filter(|p| !REMOVE.contains(p)).collect();

        if !new_parts.contains(&"G_PARAM_STATIC_NAME") {
            new_parts.push("G_PARAM_STATIC_NAME");
        }

        Some(new_parts.join(" | "))
    }
}
