use tree_sitter::Node;

use super::{Fix, Rule};
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct UseGClearList;

impl Rule for UseGClearList {
    fn name(&self) -> &'static str {
        "use_g_clear_list"
    }

    fn description(&self) -> &'static str {
        "Suggest g_clear_list/g_clear_slist instead of manual g_list_free/g_slist_free and NULL assignment"
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

impl UseGClearList {
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
        // Look for compound statements that might have g_list_free/g_slist_free
        // followed by assignment
        if node.kind() == "compound_statement" || node.kind() == "if_statement" {
            let body = if node.kind() == "if_statement" {
                node.child_by_field_name("consequence")
            } else {
                Some(node)
            };

            if let Some(body_node) = body {
                for (var_name, list_type, first_stmt, second_stmt) in
                    self.check_free_then_null(ast_context, body_node, source)
                {
                    let position = first_stmt.start_position();
                    let clear_fn = if list_type == "GList" {
                        "g_clear_list"
                    } else {
                        "g_clear_slist"
                    };

                    let replacement = format!("{} (&{}, NULL);", clear_fn, var_name);

                    let fix = Fix {
                        start_byte: base_byte + first_stmt.start_byte(),
                        end_byte: base_byte + second_stmt.end_byte(),
                        replacement: replacement.clone(),
                    };

                    violations.push(self.violation_with_fix(
                        file_path,
                        base_line + position.row,
                        position.column + 1,
                        format!(
                            "Use {} instead of {}_free and NULL assignment",
                            replacement,
                            list_type.to_lowercase()
                        ),
                        fix,
                    ));
                }
            }
        }

        // Recurse
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

    /// Check for consecutive g_list_free/g_slist_free + list = NULL
    /// Returns (var_name, list_type, first_statement, second_statement)
    fn check_free_then_null<'a>(
        &self,
        ast_context: &AstContext,
        compound: Node<'a>,
        source: &[u8],
    ) -> Vec<(String, &'static str, Node<'a>, Node<'a>)> {
        let mut cursor = compound.walk();
        let statements: Vec<_> = compound
            .children(&mut cursor)
            .filter(|n| n.kind() == "expression_statement")
            .collect();

        let mut results = Vec::new();

        // Look for consecutive pairs
        for i in 0..statements.len().saturating_sub(1) {
            let first = statements[i];
            let second = statements[i + 1];

            // Check if first is g_list_free or g_slist_free
            if let Some((var_name, list_type)) = self.extract_list_free(ast_context, first, source)
            {
                // Check if second is assignment to NULL
                if let Some(assign_var) = self.extract_null_assignment(ast_context, second, source)
                {
                    if assign_var.trim() == var_name.trim() {
                        results.push((var_name, list_type, first, second));
                    }
                }
            }
        }

        results
    }

    fn extract_list_free(
        &self,
        ast_context: &AstContext,
        node: Node,
        source: &[u8],
    ) -> Option<(String, &'static str)> {
        if let Some(call) = ast_context.find_call_expression(node) {
            if let Some(function) = call.child_by_field_name("function") {
                let func_name = ast_context.get_node_text(function, source);

                let list_type = match func_name.as_str() {
                    "g_list_free" => "GList",
                    "g_slist_free" => "GSList",
                    _ => return None,
                };

                // Get the first argument (the list variable)
                if let Some(args) = call.child_by_field_name("arguments") {
                    let mut cursor = args.walk();
                    for child in args.children(&mut cursor) {
                        if child.kind() != "(" && child.kind() != ")" && child.kind() != "," {
                            return Some((
                                ast_context.get_node_text(child, source).trim().to_string(),
                                list_type,
                            ));
                        }
                    }
                }
            }
        }
        None
    }

    fn extract_null_assignment(
        &self,
        ast_context: &AstContext,
        node: Node,
        source: &[u8],
    ) -> Option<String> {
        if let Some(assignment) = self.find_assignment(node) {
            if let Some(left) = assignment.child_by_field_name("left") {
                if let Some(right) = assignment.child_by_field_name("right") {
                    let right_text = ast_context.get_node_text(right, source);
                    if right_text.trim() == "NULL" {
                        return Some(ast_context.get_node_text(left, source).trim().to_string());
                    }
                }
            }
        }
        None
    }

    fn find_assignment<'a>(&self, node: Node<'a>) -> Option<Node<'a>> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "assignment_expression" {
                return Some(child);
            }
            if let Some(assignment) = self.find_assignment(child) {
                return Some(assignment);
            }
        }
        None
    }
}
