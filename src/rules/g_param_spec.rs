use super::{Rule, Violation};
use crate::config::Config;
use std::path::Path;
use tree_sitter::Node;

pub struct GParamSpecNullNickBlurb;

impl GParamSpecNullNickBlurb {
    fn is_g_param_spec_call(&self, node: Node, source: &[u8]) -> bool {
        if node.kind() != "call_expression" {
            return false;
        }

        let Some(function_node) = node.child_by_field_name("function") else {
            return false;
        };

        let function_name = &source[function_node.byte_range()];
        let function_str = std::str::from_utf8(function_name).unwrap_or("");

        function_str.starts_with("g_param_spec_")
    }

    fn check_argument_is_null(&self, arg_node: Node, source: &[u8]) -> bool {
        let arg_text = &source[arg_node.byte_range()];
        let arg_str = std::str::from_utf8(arg_text).unwrap_or("").trim();

        arg_str == "NULL" || arg_str == "((void*)0)" || arg_str == "0"
    }
}

impl Rule for GParamSpecNullNickBlurb {
    fn name(&self) -> &str {
        "g_param_spec_null_nick_blurb"
    }

    fn check(&self, node: Node, source: &[u8], file_path: &Path) -> Vec<Violation> {
        let mut violations = Vec::new();

        if !self.is_g_param_spec_call(node, source) {
            return violations;
        }

        let Some(arguments_node) = node.child_by_field_name("arguments") else {
            return violations;
        };

        // Get all argument nodes (skip parentheses and commas)
        let mut args = Vec::new();
        let mut cursor = arguments_node.walk();
        for child in arguments_node.children(&mut cursor) {
            if child.is_named() && child.kind() != "," {
                args.push(child);
            }
        }

        // Check if we have at least 3 arguments (name, nick, blurb)
        if args.len() < 3 {
            return violations;
        }

        let nick_arg = args[1]; // Second argument (0-indexed, so position 1)
        let blurb_arg = args[2]; // Third argument (0-indexed, so position 2)

        let mut issues = Vec::new();

        if !self.check_argument_is_null(nick_arg, source) {
            issues.push("nick (parameter 2)");
        }

        if !self.check_argument_is_null(blurb_arg, source) {
            issues.push("blurb (parameter 3)");
        }

        if !issues.is_empty() {
            let position = node.start_position();
            let function_node = node.child_by_field_name("function").unwrap();
            let function_name = std::str::from_utf8(&source[function_node.byte_range()])
                .unwrap_or("g_param_spec_*");

            violations.push(Violation {
                file: file_path.display().to_string(),
                line: position.row + 1,
                column: position.column + 1,
                message: format!(
                    "{} should have NULL for {}",
                    function_name,
                    issues.join(" and ")
                ),
                rule: self.name().to_string(),
                snippet: None,
            });
        }

        violations
    }

    fn is_enabled(&self, config: &Config) -> bool {
        config.rules.g_param_spec_null_nick_blurb
    }
}
