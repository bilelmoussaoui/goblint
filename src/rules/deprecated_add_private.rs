use super::Violation;
use crate::ast_context::AstContext;
use crate::config::Config;
use tree_sitter::{Node, Parser};

pub struct DeprecatedAddPrivate;

impl DeprecatedAddPrivate {
    pub fn check_all(
        &self,
        ast_context: &AstContext,
        _config: &Config,
        violations: &mut Vec<Violation>,
    ) {
        let mut parser = Parser::new();
        parser.set_language(&tree_sitter_c::LANGUAGE.into()).ok();

        // Check all C files
        for (path, file) in ast_context.project.files.iter() {
            if path.extension().is_none_or(|ext| ext != "c") {
                continue;
            }

            // Check each function definition
            for func in &file.functions {
                if !func.is_definition {
                    continue;
                }

                // Get function source
                if let Some(func_source) = ast_context.get_function_source(path, func) {
                    // Parse and check for deprecated call
                    if let Some(tree) = parser.parse(func_source, None) {
                        self.check_node(tree.root_node(), func_source, path, func.line, violations);
                    }
                }
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
        if node.kind() == "call_expression" {
            if let Some(function) = node.child_by_field_name("function") {
                let func_text = &source[function.byte_range()];
                if let Ok(text) = std::str::from_utf8(func_text) {
                    if text == "g_type_class_add_private" {
                        violations.push(Violation {
                            file: file_path.to_owned(),
                            line: base_line + node.start_position().row,
                            column: node.start_position().column + 1,
                            message: "g_type_class_add_private is deprecated since GLib 2.58. Use G_DEFINE_TYPE_WITH_PRIVATE or G_ADD_PRIVATE instead".to_string(),
                            rule: "deprecated_add_private",
                            snippet: None,
                        });
                    }
                }
            }
        }

        // Recurse into children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.check_node(child, source, file_path, base_line, violations);
        }
    }
}
