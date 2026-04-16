use gobject_ast::CallExpression;

use super::{Fix, Rule};
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct GParamSpecStaticStrings;

impl Rule for GParamSpecStaticStrings {
    fn name(&self) -> &'static str {
        "g_param_spec_static_strings"
    }

    fn description(&self) -> &'static str {
        "Ensure g_param_spec_* calls use G_PARAM_STATIC_STRINGS flag for string literals"
    }

    fn category(&self) -> super::Category {
        super::Category::Perf
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

        // Find all g_param_spec_* calls (but skip g_param_spec_override and
        // g_param_spec_internal)
        for call in func.find_calls_matching(|name| {
            name.starts_with("g_param_spec_")
                && name != "g_param_spec_override"
                && name != "g_param_spec_internal"
        }) {
            self.check_call(path, call, violations);
        }
    }
}

impl GParamSpecStaticStrings {
    fn check_call(
        &self,
        file_path: &std::path::Path,
        call: &CallExpression,
        violations: &mut Vec<Violation>,
    ) {
        // g_param_spec_*(name, nick, blurb, ..., flags) — need at least 4 args
        if call.arguments.len() < 4 {
            return;
        }

        // Check if arguments are string literals or NULL
        let name_is_literal = call.arguments[0].is_string_or_macro_string();
        let nick_is_literal = call.arguments[1].is_string_or_macro_string();
        let blurb_is_literal = call.arguments[2].is_string_or_macro_string();

        let nick_is_null = call.arguments[1].is_null();
        let blurb_is_null = call.arguments[2].is_null();

        // Only check when name is a string literal and nick/blurb are literals or NULL
        if !name_is_literal
            || (!nick_is_literal && !nick_is_null)
            || (!blurb_is_literal && !blurb_is_null)
        {
            return;
        }

        // Get the flags argument (last argument)
        let gobject_ast::Argument::Expression(flags_expr) = call.arguments.last().unwrap();

        // Check what static flags are present
        let has_static_strings = flags_expr.contains_identifier("G_PARAM_STATIC_STRINGS");
        let has_static_name = flags_expr.contains_identifier("G_PARAM_STATIC_NAME");
        let has_static_nick = flags_expr.contains_identifier("G_PARAM_STATIC_NICK");
        let has_static_blurb = flags_expr.contains_identifier("G_PARAM_STATIC_BLURB");

        // Is the minimal required set of static flags already present?
        let is_satisfied = if has_static_strings {
            // G_PARAM_STATIC_STRINGS covers everything — always satisfied
            true
        } else if nick_is_literal && blurb_is_literal {
            // All three strings are literals — need NAME + NICK + BLURB
            has_static_name && has_static_nick && has_static_blurb
        } else if nick_is_literal {
            has_static_name && has_static_nick
        } else if blurb_is_literal {
            has_static_name && has_static_blurb
        } else {
            // nick and blurb are NULL — only the name needs the static flag
            has_static_name
        };

        if is_satisfied {
            return;
        }

        // Build the fix
        let flags_identifiers = flags_expr.collect_identifiers();
        let flags_text = if flags_identifiers.is_empty() {
            "0".to_string()
        } else {
            flags_identifiers.join(" | ")
        };

        let needed = self.needed_flags(nick_is_literal, blurb_is_literal);
        let replacement = self.build_fixed_flags(&flags_text, nick_is_literal, blurb_is_literal);

        let fix = Fix::new(
            flags_expr.location().start_byte,
            flags_expr.location().end_byte,
            replacement,
        );

        violations.push(self.violation_with_fix(
            file_path,
            call.location.line,
            call.location.column,
            format!(
                "Add {} to {} flags (saves memory for static strings)",
                needed, call.function
            ),
            fix,
        ));
    }

    /// Return the flag expression that should be added, given which args are
    /// literals.
    fn needed_flags(&self, nick_is_literal: bool, blurb_is_literal: bool) -> &'static str {
        match (nick_is_literal, blurb_is_literal) {
            (true, true) => "G_PARAM_STATIC_STRINGS",
            (true, false) => "G_PARAM_STATIC_NAME | G_PARAM_STATIC_NICK",
            (false, true) => "G_PARAM_STATIC_NAME | G_PARAM_STATIC_BLURB",
            (false, false) => "G_PARAM_STATIC_NAME",
        }
    }

    /// Build the replacement flags string: remove any individual static flags
    /// already present, then append the minimal required ones.
    fn build_fixed_flags(
        &self,
        flags_text: &str,
        nick_is_literal: bool,
        blurb_is_literal: bool,
    ) -> String {
        const INDIVIDUAL: &[&str] = &[
            "G_PARAM_STATIC_NAME",
            "G_PARAM_STATIC_NICK",
            "G_PARAM_STATIC_BLURB",
            "G_PARAM_STATIC_STRINGS",
        ];

        // Strip existing static flags; keep everything else.
        let mut parts: Vec<&str> = if flags_text.is_empty() || flags_text == "0" {
            Vec::new()
        } else {
            flags_text
                .split('|')
                .map(|s| s.trim())
                .filter(|s| !INDIVIDUAL.contains(s))
                .collect()
        };

        // Append the minimal needed flags.
        match (nick_is_literal, blurb_is_literal) {
            (true, true) => parts.push("G_PARAM_STATIC_STRINGS"),
            (true, false) => {
                parts.push("G_PARAM_STATIC_NAME");
                parts.push("G_PARAM_STATIC_NICK");
            }
            (false, true) => {
                parts.push("G_PARAM_STATIC_NAME");
                parts.push("G_PARAM_STATIC_BLURB");
            }
            (false, false) => parts.push("G_PARAM_STATIC_NAME"),
        }

        parts.join(" | ")
    }
}
