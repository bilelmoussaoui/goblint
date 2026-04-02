use super::{Rule, Violation};
use crate::config::Config;
use std::path::Path;
use tree_sitter::Node;

pub struct UseGStrcmp0;

impl UseGStrcmp0 {
    fn is_strcmp_call(&self, node: Node, source: &[u8]) -> bool {
        if node.kind() != "call_expression" {
            return false;
        }

        let Some(function) = node.child_by_field_name("function") else {
            return false;
        };

        let func_text = self.get_node_text(function, source);

        // Check for strcmp, but not g_strcmp0
        func_text == "strcmp" || func_text == "strncmp"
    }

    fn get_node_text(&self, node: Node, source: &[u8]) -> String {
        let text = &source[node.byte_range()];
        std::str::from_utf8(text).unwrap_or("").to_string()
    }
}

impl Rule for UseGStrcmp0 {
    fn name(&self) -> &str {
        "use_g_strcmp0"
    }

    fn check(&self, node: Node, source: &[u8], file_path: &Path) -> Vec<Violation> {
        let mut violations = Vec::new();

        if !self.is_strcmp_call(node, source) {
            return violations;
        }

        let position = node.start_position();
        let function = node.child_by_field_name("function").unwrap();
        let func_name = self.get_node_text(function, source);

        let suggestion = if func_name == "strcmp" {
            "g_strcmp0"
        } else {
            "g_strcmp0 or check for NULL first"
        };

        violations.push(Violation {
            file: file_path.display().to_string(),
            line: position.row + 1,
            column: position.column + 1,
            message: format!("Use {} instead of {} (NULL-safe)", suggestion, func_name),
            rule: self.name().to_string(),
            snippet: None,
        });

        violations
    }

    fn is_enabled(&self, config: &Config) -> bool {
        config.rules.use_g_strcmp0
    }
}
