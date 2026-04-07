use super::Violation;
use crate::ast_context::AstContext;
use crate::config::Config;
use tree_sitter::{Node, Parser};

pub struct GErrorInit;

impl GErrorInit {
    pub fn check_all(
        &self,
        ast_context: &AstContext,
        _config: &Config,
        violations: &mut Vec<Violation>,
    ) {
        let mut parser = Parser::new();
        parser.set_language(&tree_sitter_c::LANGUAGE.into()).ok();

        for (path, file) in ast_context.project.files.iter() {
            if path.extension().is_none_or(|ext| ext != "c") {
                continue;
            }

            for func in &file.functions {
                if !func.is_definition {
                    continue;
                }

                if let Some(func_source) = ast_context.get_function_source(path, func) {
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
        if let Some((var_name, is_initialized_to_null)) = self.is_gerror_declaration(node, source) {
            if !is_initialized_to_null {
                violations.push(Violation {
                    file: file_path.to_owned(),
                    line: base_line + node.start_position().row,
                    column: node.start_position().column + 1,
                    message: format!(
                        "GError *{} must be initialized to NULL (GError *{} = NULL;)",
                        var_name, var_name
                    ),
                    rule: "gerror_init",
                    snippet: None,
                });
            }
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.check_node(child, source, file_path, base_line, violations);
        }
    }

    fn is_gerror_declaration(&self, node: Node, source: &[u8]) -> Option<(String, bool)> {
        if node.kind() != "declaration" {
            return None;
        }

        let mut check_cursor = node.walk();
        for child in node.children(&mut check_cursor) {
            if self.contains_function_declarator(child) {
                return None;
            }
        }

        let type_node = node.child_by_field_name("type")?;
        let type_text = self.get_node_text(type_node, source);

        if !type_text.contains("GError") {
            return None;
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "pointer_declarator" || child.kind() == "init_declarator" {
                let declarator_text = self.get_node_text(child, source);

                if !declarator_text.contains('*') {
                    continue;
                }

                if child.kind() == "init_declarator" {
                    if let Some(value) = child.child_by_field_name("value") {
                        let value_full = self.get_node_text(value, source);
                        let value_text = value_full.trim();
                        let is_null =
                            value_text == "NULL" || value_text == "0" || value_text == "((void*)0)";

                        if let Some(declarator) = child.child_by_field_name("declarator") {
                            let var_name = self.extract_variable_name(declarator, source)?;
                            return Some((var_name, is_null));
                        }
                    }
                } else if child.kind() == "pointer_declarator" {
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

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if self.contains_function_declarator(child) {
                return true;
            }
        }

        false
    }

    fn extract_variable_name(&self, declarator: Node, source: &[u8]) -> Option<String> {
        if let Some(inner) = declarator.child_by_field_name("declarator") {
            if inner.kind() == "identifier" {
                return Some(self.get_node_text(inner, source));
            }
            return self.extract_variable_name(inner, source);
        }

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
