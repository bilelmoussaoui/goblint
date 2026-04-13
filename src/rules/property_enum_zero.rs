use tree_sitter::Node;

use super::{Fix, Rule};
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct PropertyEnumZero;

impl Rule for PropertyEnumZero {
    fn name(&self) -> &'static str {
        "property_enum_zero"
    }

    fn description(&self) -> &'static str {
        "Ensure property enums start with PROP_0, not PROP_NAME = 0"
    }

    fn category(&self) -> super::Category {
        super::Category::Correctness
    }

    fn fixable(&self) -> bool {
        true
    }

    fn check_all(
        &self,
        ast_context: &AstContext,
        _config: &Config,
        violations: &mut Vec<Violation>,
    ) {
        // Check both C and header files
        for (path, file) in ast_context.iter_all_files() {
            // Parse the entire file since enums can be at top-level
            if let Some(tree) = ast_context.parse_c_source(&file.source) {
                self.check_node(
                    ast_context,
                    tree.root_node(),
                    &file.source,
                    path,
                    0,
                    violations,
                );
            }
        }
    }
}

impl PropertyEnumZero {
    fn check_node(
        &self,
        ast_context: &AstContext,
        node: Node,
        source: &[u8],
        file_path: &std::path::Path,
        base_line: usize,
        violations: &mut Vec<Violation>,
    ) {
        if self.is_property_enum(ast_context, node, source)
            && let Some((prop_name, name_node)) =
                self.check_first_enumerator(ast_context, node, source)
        {
            let fix = Fix::new(name_node.start_byte(), name_node.end_byte(), "PROP_0");

            violations.push(self.violation_with_fix(
                    file_path,
                    base_line + name_node.start_position().row,
                    name_node.start_position().column + 1,
                    format!(
                        "Property enum should start with PROP_0, not {} = 0. First property should be PROP_0, second should be {}",
                        prop_name, prop_name
                    ),
                    fix,
                ));
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.check_node(ast_context, child, source, file_path, base_line, violations);
        }
    }

    fn is_property_enum(&self, ast_context: &AstContext, node: Node, source: &[u8]) -> bool {
        if node.kind() != "enum_specifier" {
            return false;
        }

        let Some(body) = node.child_by_field_name("body") else {
            return false;
        };

        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            if child.kind() == "enumerator"
                && let Some(name) = child.child_by_field_name("name")
            {
                let name_text = ast_context.get_node_text(name, source);
                if name_text.starts_with("PROP_") {
                    return true;
                }
            }
        }

        false
    }

    fn check_first_enumerator<'a>(
        &self,
        ast_context: &AstContext,
        node: Node<'a>,
        source: &'a [u8],
    ) -> Option<(&'a str, Node<'a>)> {
        let body = node.child_by_field_name("body")?;

        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            if child.kind() == "enumerator"
                && let Some(name) = child.child_by_field_name("name")
            {
                let name_text = ast_context.get_node_text(name, source);

                if let Some(value) = child.child_by_field_name("value") {
                    let value_text = ast_context.get_node_text(value, source).trim();

                    if name_text.starts_with("PROP_")
                        && !name_text.ends_with("_0")
                        && value_text == "0"
                    {
                        return Some((name_text, name));
                    }
                } else {
                    if name_text.starts_with("PROP_") && !name_text.ends_with("_0") {
                        return Some((name_text, name));
                    }
                }

                if name_text.starts_with("PROP_") {
                    break;
                }
            }
        }

        None
    }
}
