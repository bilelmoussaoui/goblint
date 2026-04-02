use super::{Rule, Violation};
use crate::config::Config;
use std::path::Path;
use tree_sitter::Node;

pub struct PropertyEnumZero;

impl PropertyEnumZero {
    fn is_property_enum(&self, node: Node, source: &[u8]) -> bool {
        if node.kind() != "enum_specifier" {
            return false;
        }

        // Check if this looks like a property enum by examining the enumerators
        let Some(body) = node.child_by_field_name("body") else {
            return false;
        };

        let mut cursor = body.walk();
        let mut has_prop_prefix = false;

        for child in body.children(&mut cursor) {
            if child.kind() == "enumerator" {
                if let Some(name) = child.child_by_field_name("name") {
                    let name_text = self.get_node_text(name, source);
                    if name_text.starts_with("PROP_") {
                        has_prop_prefix = true;
                        break;
                    }
                }
            }
        }

        has_prop_prefix
    }

    fn check_first_enumerator(&self, node: Node, source: &[u8]) -> Option<(String, usize)> {
        let body = node.child_by_field_name("body")?;

        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            if child.kind() == "enumerator" {
                if let Some(name) = child.child_by_field_name("name") {
                    let name_text = self.get_node_text(name, source);

                    // Check if it has an explicit value
                    if let Some(value) = child.child_by_field_name("value") {
                        let value_text = self.get_node_text(value, source).trim().to_string();

                        // Check if the first PROP_ entry is set to 0
                        if name_text.starts_with("PROP_")
                            && name_text != "PROP_0"
                            && value_text == "0"
                        {
                            let position = child.start_position();
                            return Some((name_text, position.row + 1));
                        }
                    } else {
                        // First enumerator without explicit value defaults to 0
                        if name_text.starts_with("PROP_") && name_text != "PROP_0" {
                            let position = child.start_position();
                            return Some((name_text, position.row + 1));
                        }
                    }

                    // Only check the first PROP_ entry
                    if name_text.starts_with("PROP_") {
                        break;
                    }
                }
            }
        }

        None
    }

    fn get_node_text(&self, node: Node, source: &[u8]) -> String {
        let text = &source[node.byte_range()];
        std::str::from_utf8(text).unwrap_or("").to_string()
    }
}

impl Rule for PropertyEnumZero {
    fn name(&self) -> &str {
        "property_enum_zero"
    }

    fn check(&self, node: Node, source: &[u8], file_path: &Path) -> Vec<Violation> {
        let mut violations = Vec::new();

        if !self.is_property_enum(node, source) {
            return violations;
        }

        if let Some((prop_name, line)) = self.check_first_enumerator(node, source) {
            violations.push(Violation {
                file: file_path.display().to_string(),
                line,
                column: 1,
                message: format!(
                    "Property enum should start with PROP_0, not {} = 0. First property should be PROP_0, second should be {}",
                    prop_name, prop_name
                ),
                rule: self.name().to_string(),
                snippet: None,
            });
        }

        violations
    }

    fn is_enabled(&self, config: &Config) -> bool {
        config.rules.property_enum_zero
    }
}
