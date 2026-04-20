use gobject_ast::{CallExpression, types::Property};

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
        config: &Config,
        func: &gobject_ast::top_level::FunctionDefItem,
        path: &std::path::Path,
        violations: &mut Vec<Violation>,
    ) {
        // Get custom flags that already include static strings
        let static_flags = config
            .get_rule_config(self.name())
            .and_then(|rc| rc.options.get("static_flags"))
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        // Find all g_param_spec_* calls (but skip g_param_spec_internal)
        for call in func.find_calls_matching(|name| {
            name.contains("_param_spec_")
                && !name.ends_with("_param_spec_override")
                && !name.ends_with("_param_spec_internal")
        }) {
            self.check_call(path, call, &static_flags, violations);
        }
    }
}

impl GParamSpecNullNickBlurb {
    fn check_call(
        &self,
        file_path: &std::path::Path,
        call: &CallExpression,
        custom_static_flags: &[String],
        violations: &mut Vec<Violation>,
    ) {
        // g_param_spec_*(name, nick, blurb, ...) — need at least 3 args
        if call.arguments.len() < 3 {
            return;
        }

        // Parse the property using the AST helpers
        let Some(property) = Property::from_param_spec_call(call) else {
            return;
        };

        // Check if any custom flags that include static strings are present
        // If so, skip this rule as the project intentionally uses non-NULL nick/blurb
        use gobject_ast::types::ParamFlag;
        let has_custom_static_flag = property.flags.iter().any(|flag| {
            if let ParamFlag::Unknown(name) = flag {
                custom_static_flags.contains(name)
            } else {
                false
            }
        });

        if has_custom_static_flag {
            return; // Project uses custom flags that include static strings
        }

        // Check if nick/blurb are NULL by examining the actual expressions
        // The parsed Property only recognizes string literals, so it treats macro calls
        // like _("...") as None. We need to check the raw expressions instead.
        let Some(nick_expr) = call.get_arg(1) else {
            return;
        };
        let Some(blurb_expr) = call.get_arg(2) else {
            return;
        };

        let nick_is_null = nick_expr.is_null();
        let blurb_is_null = blurb_expr.is_null();

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

        if let Some(new_flags) = self.compute_new_flags(&property.flags) {
            let gobject_ast::Argument::Expression(flags_expr) = call.arguments.last().unwrap();
            fixes.push(Fix::new(
                flags_expr.location().start_byte,
                flags_expr.location().end_byte,
                new_flags,
            ));
        }

        violations.push(self.violation_with_fixes(
            file_path,
            call.location.line,
            call.location.column,
            format!(
                "{} should have NULL for {}",
                call.function_name(),
                issues.join(" and ")
            ),
            fixes,
        ));
    }

    /// After nick and blurb are set to NULL, compute the correct replacement
    /// flags string. Returns `None` if the flags are already correct.
    fn compute_new_flags(&self, current_flags: &[gobject_ast::types::ParamFlag]) -> Option<String> {
        use gobject_ast::types::ParamFlag;

        // Check if we need to remove any flags
        let needs_removal = current_flags.iter().any(|f| {
            matches!(
                f,
                ParamFlag::StaticNick | ParamFlag::StaticBlurb | ParamFlag::StaticStrings
            )
        });
        let has_name = current_flags
            .iter()
            .any(|f| matches!(f, ParamFlag::StaticName));

        if !needs_removal && has_name {
            return None; // Already correct
        }

        // Filter out STATIC_NICK, STATIC_BLURB, and STATIC_STRINGS
        let mut new_flags: Vec<ParamFlag> = current_flags
            .iter()
            .filter(|f| {
                !matches!(
                    f,
                    ParamFlag::StaticNick | ParamFlag::StaticBlurb | ParamFlag::StaticStrings
                )
            })
            .cloned()
            .collect();

        // Ensure STATIC_NAME is present
        if !new_flags.iter().any(|f| matches!(f, ParamFlag::StaticName)) {
            new_flags.push(ParamFlag::StaticName);
        }

        Some(if new_flags.is_empty() {
            "0".to_string()
        } else {
            new_flags
                .iter()
                .map(|f| f.as_str())
                .collect::<Vec<_>>()
                .join(" | ")
        })
    }
}
