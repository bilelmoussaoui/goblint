use super::{Rule, Violation};
use crate::config::Config;
use std::path::Path;
use tree_sitter::Node;

pub struct GErrorInit;

impl GErrorInit {
    fn is_gerror_declaration(&self, node: Node, source: &[u8]) -> Option<(String, bool)> {
        if node.kind() != "declaration" {
            return None;
        }

        // Skip function declarations (e.g., const GError * func(...);)
        // Check if this declaration contains a function_declarator
        let mut check_cursor = node.walk();
        for child in node.children(&mut check_cursor) {
            if self.contains_function_declarator(child) {
                return None;
            }
        }

        // Get the type
        let type_node = node.child_by_field_name("type")?;

        let type_text = self.get_node_text(type_node, source);

        // Check if it's GError *
        if !type_text.contains("GError") {
            return None;
        }

        // Get the declarator to check for pointer and initializer
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "pointer_declarator" || child.kind() == "init_declarator" {
                let declarator_text = self.get_node_text(child, source);

                // Must be a pointer
                if !declarator_text.contains('*') {
                    continue;
                }

                // Check if it has an initializer
                if child.kind() == "init_declarator" {
                    // Has an initializer, check if it's NULL
                    if let Some(value) = child.child_by_field_name("value") {
                        let value_full = self.get_node_text(value, source);
                        let value_text = value_full.trim();
                        let is_null =
                            value_text == "NULL" || value_text == "0" || value_text == "((void*)0)";

                        // Get variable name
                        if let Some(declarator) = child.child_by_field_name("declarator") {
                            let var_name = self.extract_variable_name(declarator, source)?;
                            return Some((var_name, is_null));
                        }
                    }
                } else if child.kind() == "pointer_declarator" {
                    // No initializer - this is the problem case
                    let var_name = self.extract_variable_name(child, source)?;
                    return Some((var_name, false));
                }
            }
        }

        None
    }

    fn contains_function_declarator(&self, node: Node) -> bool {
        if node.kind() == "function_declarator" {
            return true;
        }

        // Recursively check children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if self.contains_function_declarator(child) {
                return true;
            }
        }

        false
    }

    fn extract_variable_name(&self, declarator: Node, source: &[u8]) -> Option<String> {
        // Handle pointer_declarator -> identifier
        if let Some(inner) = declarator.child_by_field_name("declarator") {
            if inner.kind() == "identifier" {
                return Some(self.get_node_text(inner, source));
            }
            // Recurse for nested declarators
            return self.extract_variable_name(inner, source);
        }

        // Direct identifier
        if declarator.kind() == "identifier" {
            return Some(self.get_node_text(declarator, source));
        }

        None
    }

    fn get_node_text(&self, node: Node, source: &[u8]) -> String {
        let text = &source[node.byte_range()];
        std::str::from_utf8(text).unwrap_or("").to_string()
    }
}

impl Rule for GErrorInit {
    fn name(&self) -> &str {
        "gerror_init"
    }

    fn check(&self, node: Node, source: &[u8], file_path: &Path) -> Vec<Violation> {
        let mut violations = Vec::new();

        if let Some((var_name, is_initialized_to_null)) = self.is_gerror_declaration(node, source) {
            if !is_initialized_to_null {
                let position = node.start_position();

                violations.push(Violation {
                    file: file_path.display().to_string(),
                    line: position.row + 1,
                    column: position.column + 1,
                    message: format!(
                        "GError *{} must be initialized to NULL (GError *{} = NULL;)",
                        var_name, var_name
                    ),
                    rule: self.name().to_string(),
                    snippet: None,
                });
            }
        }

        violations
    }

    fn is_enabled(&self, config: &Config) -> bool {
        config.rules.gerror_init
    }
}
