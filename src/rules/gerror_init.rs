use tree_sitter::Node;

use super::Rule;
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct GErrorInit;

impl Rule for GErrorInit {
    fn name(&self) -> &'static str {
        "gerror_init"
    }

    fn description(&self) -> &'static str {
        "Ensure GError* variables are initialized to NULL"
    }
    fn check_all(
        &self,
        ast_context: &AstContext,
        _config: &Config,
        violations: &mut Vec<Violation>,
    ) {
        for (path, file) in ast_context.iter_c_files() {
            for func in &file.functions {
                if !func.is_definition {
                    continue;
                }

                if let Some(func_source) = ast_context.get_function_source(path, func) {
                    if let Some(tree) = ast_context.parse_c_source(func_source) {
                        self.check_node(
                            ast_context,
                            tree.root_node(),
                            func_source,
                            path,
                            func.line,
                            violations,
                        );
                    }
                }
            }
        }
    }
}

impl GErrorInit {
    fn check_node(
        &self,
        ast_context: &AstContext,
        node: Node,
        source: &[u8],
        file_path: &std::path::Path,
        base_line: usize,
        violations: &mut Vec<Violation>,
    ) {
        if let Some((var_name, is_initialized_to_null)) =
            self.is_gerror_declaration(ast_context, node, source)
        {
            if !is_initialized_to_null {
                violations.push(self.violation(
                    file_path,
                    base_line + node.start_position().row,
                    node.start_position().column + 1,
                    format!(
                        "GError *{} must be initialized to NULL (GError *{} = NULL;)",
                        var_name, var_name
                    ),
                ));
            }
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.check_node(ast_context, child, source, file_path, base_line, violations);
        }
    }

    fn is_gerror_declaration(
        &self,
        ast_context: &AstContext,
        node: Node,
        source: &[u8],
    ) -> Option<(String, bool)> {
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
        let type_text = ast_context.get_node_text(type_node, source);

        if !type_text.contains("GError") {
            return None;
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "pointer_declarator" || child.kind() == "init_declarator" {
                let declarator_text = ast_context.get_node_text(child, source);

                if !declarator_text.contains('*') {
                    continue;
                }

                if child.kind() == "init_declarator" {
                    if let Some(value) = child.child_by_field_name("value") {
                        let value_full = ast_context.get_node_text(value, source);
                        let value_text = value_full.trim();
                        let is_null =
                            value_text == "NULL" || value_text == "0" || value_text == "((void*)0)";

                        if let Some(declarator) = child.child_by_field_name("declarator") {
                            let var_name = ast_context.extract_variable_name(declarator, source)?;
                            return Some((var_name, is_null));
                        }
                    }
                } else if child.kind() == "pointer_declarator" {
                    let var_name = ast_context.extract_variable_name(child, source)?;
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
}
