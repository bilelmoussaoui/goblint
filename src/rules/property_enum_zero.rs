use super::Violation;
use crate::ast_context::AstContext;
use crate::config::Config;
use tree_sitter::Node;

pub struct PropertyEnumZero;

impl PropertyEnumZero {
    pub fn check_all(
        &self,
        ast_context: &AstContext,
        _config: &Config,
        violations: &mut Vec<Violation>,
    ) {
        // Check both C and header files
        for (path, file) in ast_context.iter_all_files() {
            // Parse the entire file since enums can be at top-level
            if let Some(tree) = ast_context.parse_c_source(&file.source) {
                self.check_node(tree.root_node(), &file.source, path, 0, violations);
            }
        }
    }

    fn check_node(
        &self,
        node: Node,
        source: &[u8],
        file_path: &std::path::Path,
        base_line: usize,
        violations: &mut Vec<Violation>,
    ) {
        if self.is_property_enum(node, source) {
            if let Some((prop_name, line_offset)) = self.check_first_enumerator(node, source) {
                violations.push(Violation {
                    file: file_path.to_owned(),
                    line: base_line + line_offset,
                    column: 1,
                    message: format!(
                        "Property enum should start with PROP_0, not {} = 0. First property should be PROP_0, second should be {}",
                        prop_name, prop_name
                    ),
                    rule: "property_enum_zero",
                    snippet: None,
                });
            }
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.check_node(child, source, file_path, base_line, violations);
        }
    }

    fn is_property_enum(&self, node: Node, source: &[u8]) -> bool {
        if node.kind() != "enum_specifier" {
            return false;
        }

        let Some(body) = node.child_by_field_name("body") else {
            return false;
        };

        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            if child.kind() == "enumerator" {
                if let Some(name) = child.child_by_field_name("name") {
                    let name_text = self.get_node_text(name, source);
                    if name_text.starts_with("PROP_") {
                        return true;
                    }
                }
            }
        }

        false
    }

    fn check_first_enumerator(&self, node: Node, source: &[u8]) -> Option<(String, usize)> {
        let body = node.child_by_field_name("body")?;

        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            if child.kind() == "enumerator" {
                if let Some(name) = child.child_by_field_name("name") {
                    let name_text = self.get_node_text(name, source);

                    if let Some(value) = child.child_by_field_name("value") {
                        let value_text = self.get_node_text(value, source).trim().to_string();

                        if name_text.starts_with("PROP_")
                            && !name_text.ends_with("_0")
                            && value_text == "0"
                        {
                            let position = child.start_position();
                            return Some((name_text, position.row + 1));
                        }
                    } else {
                        if name_text.starts_with("PROP_") && !name_text.ends_with("_0") {
                            let position = child.start_position();
                            return Some((name_text, position.row + 1));
                        }
                    }

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
