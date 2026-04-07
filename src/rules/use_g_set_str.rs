use super::Rule;
use crate::ast_context::AstContext;
use crate::config::Config;
use crate::rules::Violation;
use tree_sitter::Node;

pub struct UseGSetStr;

impl Rule for UseGSetStr {
    fn name(&self) -> &'static str {
        "use_g_set_str"
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

impl UseGSetStr {
    fn check_node(
        &self,
        ast_context: &AstContext,
        node: Node,
        source: &[u8],
        file_path: &std::path::Path,
        base_line: usize,
        violations: &mut Vec<Violation>,
    ) {
        // Look for g_free(var) followed by var = g_strdup(...) in any compound statement or if body
        if node.kind() == "compound_statement" || node.kind() == "if_statement" {
            let body = if node.kind() == "if_statement" {
                node.child_by_field_name("consequence")
            } else {
                Some(node)
            };

            if let Some(body_node) = body {
                if let Some(((var_name, args_text), gfree_node)) =
                    self.check_free_then_strdup(ast_context, body_node, source)
                {
                    let position = gfree_node.start_position();
                    // Strip parentheses from args if present
                    let args_clean = args_text.trim_start_matches('(').trim_end_matches(')');
                    violations.push(self.violation(
                        file_path,
                        base_line + position.row,
                        position.column + 1,
                        format!(
                            "Use g_set_str(&{}, {}) instead of g_free and g_strdup",
                            var_name, args_clean
                        ),
                    ));
                }
            }
        }

        // Recurse
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.check_node(ast_context, child, source, file_path, base_line, violations);
        }
    }

    /// Check for consecutive g_free + g_strdup
    fn check_free_then_strdup<'a>(
        &self,
        ast_context: &AstContext,
        compound: Node<'a>,
        source: &[u8],
    ) -> Option<((String, String), Node<'a>)> {
        let mut cursor = compound.walk();
        let statements: Vec<_> = compound
            .children(&mut cursor)
            .filter(|n| n.kind() == "expression_statement")
            .collect();

        // Look for consecutive pairs
        for i in 0..statements.len().saturating_sub(1) {
            let first = statements[i];
            let second = statements[i + 1];

            // Check if first is g_free
            if let Some(var_name) = self.extract_gfree_var(ast_context, first, source) {
                // Check if second is assignment with g_strdup
                if let Some((assign_var, new_val)) =
                    self.extract_strdup_assignment(ast_context, second, source)
                {
                    if assign_var.trim() == var_name.trim() {
                        return Some(((var_name, new_val), first));
                    }
                }
            }
        }

        None
    }

    fn extract_gfree_var(
        &self,
        ast_context: &AstContext,
        node: Node,
        source: &[u8],
    ) -> Option<String> {
        if let Some(call) = ast_context.find_call_expression(node) {
            if let Some(function) = call.child_by_field_name("function") {
                let func_name = ast_context.get_node_text(function, source);
                if func_name == "g_free" {
                    if let Some(args) = call.child_by_field_name("arguments") {
                        // Get the first argument (skip the parentheses)
                        let mut cursor = args.walk();
                        for child in args.children(&mut cursor) {
                            if child.kind() != "(" && child.kind() != ")" {
                                return Some(
                                    ast_context.get_node_text(child, source).trim().to_string(),
                                );
                            }
                        }
                    }
                }
            }
        }
        None
    }

    fn extract_strdup_assignment(
        &self,
        ast_context: &AstContext,
        node: Node,
        source: &[u8],
    ) -> Option<(String, String)> {
        if let Some(assignment) = self.find_assignment(node) {
            if let Some(left) = assignment.child_by_field_name("left") {
                let left_text = ast_context.get_node_text(left, source);
                if let Some(right) = assignment.child_by_field_name("right") {
                    // Direct g_strdup call
                    if right.kind() == "call_expression" {
                        if let Some(func) = right.child_by_field_name("function") {
                            let func_name = ast_context.get_node_text(func, source);
                            if func_name == "g_strdup" || func_name == "g_strndup" {
                                if let Some(args) = right.child_by_field_name("arguments") {
                                    let args_text = ast_context.get_node_text(args, source);
                                    return Some((
                                        left_text.trim().to_string(),
                                        args_text.trim().to_string(),
                                    ));
                                }
                            }
                        }
                    }
                    // Ternary: var ? g_strdup(var) : NULL
                    else if right.kind() == "conditional_expression" {
                        if let Some(consequence) = right.child_by_field_name("consequence") {
                            if consequence.kind() == "call_expression" {
                                if let Some(func) = consequence.child_by_field_name("function") {
                                    let func_name = ast_context.get_node_text(func, source);
                                    if func_name == "g_strdup" || func_name == "g_strndup" {
                                        // For ternary, suggest the condition variable
                                        if let Some(condition) =
                                            right.child_by_field_name("condition")
                                        {
                                            let cond_text =
                                                ast_context.get_node_text(condition, source);
                                            return Some((
                                                left_text.trim().to_string(),
                                                cond_text.trim().to_string(),
                                            ));
                                        }
                                    }
                                }
                            }
                        }
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
