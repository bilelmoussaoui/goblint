use super::{Rule, Violation};
use crate::config::Config;
use std::path::Path;
use tree_sitter::Node;

pub struct DeprecatedAddPrivate;

impl DeprecatedAddPrivate {
    fn is_add_private_call(&self, node: Node, source: &[u8]) -> bool {
        if node.kind() != "call_expression" {
            return false;
        }

        let Some(function) = node.child_by_field_name("function") else {
            return false;
        };

        let func_text = self.get_node_text(function, source);

        func_text == "g_type_class_add_private"
    }

    fn get_node_text(&self, node: Node, source: &[u8]) -> String {
        let text = &source[node.byte_range()];
        std::str::from_utf8(text).unwrap_or("").to_string()
    }
}

impl Rule for DeprecatedAddPrivate {
    fn name(&self) -> &str {
        "deprecated_add_private"
    }

    fn check(&self, node: Node, source: &[u8], file_path: &Path) -> Vec<Violation> {
        let mut violations = Vec::new();

        if !self.is_add_private_call(node, source) {
            return violations;
        }

        let position = node.start_position();

        violations.push(Violation {
            file: file_path.display().to_string(),
            line: position.row + 1,
            column: position.column + 1,
            message: "g_type_class_add_private is deprecated since GLib 2.58. Use G_DEFINE_TYPE_WITH_PRIVATE or G_ADD_PRIVATE instead".to_string(),
            rule: self.name().to_string(),
            snippet: None,
        });

        violations
    }

    fn is_enabled(&self, config: &Config) -> bool {
        config.rules.deprecated_add_private
    }
}
