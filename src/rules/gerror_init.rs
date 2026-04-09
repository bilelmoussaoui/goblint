use tree_sitter::Node;

use super::{Fix, Rule};
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
                        let base_byte = func.start_byte.unwrap_or(0);
                        self.check_node(
                            ast_context,
                            tree.root_node(),
                            func_source,
                            path,
                            func.line,
                            base_byte,
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
        base_byte: usize,
        violations: &mut Vec<Violation>,
    ) {
        if let Some((var_name, is_initialized_to_null, declarator_node)) =
            self.is_gerror_declaration(ast_context, node, source)
        {
            if !is_initialized_to_null {
                // Add = NULL right after the declarator (before the semicolon)
                let fix = Fix {
                    start_byte: base_byte + declarator_node.end_byte(),
                    end_byte: base_byte + declarator_node.end_byte(),
                    replacement: " = NULL".to_string(),
                };

                violations.push(self.violation_with_fix(
                    file_path,
                    base_line + node.start_position().row,
                    node.start_position().column + 1,
                    format!("GError *{} must be initialized to NULL", var_name),
                    fix,
                ));
            }
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.check_node(
                ast_context,
                child,
                source,
                file_path,
                base_line,
                base_byte,
                violations,
            );
        }
    }

    fn is_gerror_declaration<'a>(
        &self,
        ast_context: &AstContext,
        node: Node<'a>,
        source: &[u8],
    ) -> Option<(String, bool, Node<'a>)> {
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
                            return Some((var_name, is_null, declarator));
                        }
                    }
                } else if child.kind() == "pointer_declarator" {
                    let var_name = ast_context.extract_variable_name(child, source)?;
                    return Some((var_name, false, child));
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
