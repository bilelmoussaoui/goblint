use tree_sitter::Node;

use super::{CheckContext, Fix, Rule};
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct UseGSetStr;

impl Rule for UseGSetStr {
    fn name(&self) -> &'static str {
        "use_g_set_str"
    }

    fn description(&self) -> &'static str {
        "Suggest g_set_str() instead of manual g_free and g_strdup"
    }

    fn category(&self) -> super::Category {
        super::Category::Complexity
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

                if let Some(func_source) = ast_context.get_function_source(path, func)
                    && let Some(tree) = ast_context.parse_c_source(func_source)
                {
                    let ctx = CheckContext {
                        source: func_source,
                        file_path: path,
                        base_line: func.line,
                        base_byte: func.start_byte.unwrap_or(0),
                    };
                    self.check_node(ast_context, tree.root_node(), &ctx, violations);
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
        ctx: &CheckContext,
        violations: &mut Vec<Violation>,
    ) {
        // Look for g_free(var) followed by var = g_strdup(...) in any compound
        // statement or if body
        if node.kind() == "compound_statement" || node.kind() == "if_statement" {
            let body = if node.kind() == "if_statement" {
                node.child_by_field_name("consequence")
            } else {
                Some(node)
            };

            if let Some(body_node) = body
                && let Some(((var_name, args_text), first_stmt, second_stmt)) =
                    self.check_free_then_strdup(ast_context, body_node, ctx.source)
            {
                let position = first_stmt.start_position();
                // Strip parentheses from args if present
                let args_clean = args_text.trim_start_matches('(').trim_end_matches(')');

                let replacement = format!("g_set_str (&{}, {});", var_name, args_clean);

                let fix = Fix {
                    start_byte: ctx.base_byte + first_stmt.start_byte(),
                    end_byte: ctx.base_byte + second_stmt.end_byte(),
                    replacement: replacement.clone(),
                };

                violations.push(self.violation_with_fix(
                    ctx.file_path,
                    ctx.base_line + position.row,
                    position.column + 1,
                    format!("Use {} instead of g_free and g_strdup", replacement),
                    fix,
                ));
            }
        }

        // Recurse
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.check_node(ast_context, child, ctx, violations);
        }
    }

    /// Check for consecutive g_free + g_strdup
    /// Returns ((var_name, args_text), first_stmt, second_stmt)
    fn check_free_then_strdup<'a>(
        &self,
        ast_context: &AstContext,
        compound: Node<'a>,
        source: &[u8],
    ) -> Option<((String, String), Node<'a>, Node<'a>)> {
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
                    && assign_var.trim() == var_name.trim()
                {
                    return Some(((var_name, new_val), first, second));
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
        if let Some(call) = ast_context.find_call_expression(node)
            && let Some(function) = call.child_by_field_name("function")
        {
            let func_name = ast_context.get_node_text(function, source);

            // Match g_free(var)
            if func_name == "g_free" {
                if let Some(args) = call.child_by_field_name("arguments") {
                    // Get the first argument (skip the parentheses)
                    let mut cursor = args.walk();
                    for child in args.children(&mut cursor) {
                        if child.kind() != "(" && child.kind() != ")" && child.kind() != "," {
                            return Some(
                                ast_context.get_node_text(child, source).trim().to_string(),
                            );
                        }
                    }
                }
            }
            // Match g_clear_pointer(&var, g_free)
            else if func_name == "g_clear_pointer"
                && let Some(args) = call.child_by_field_name("arguments")
            {
                let mut cursor = args.walk();
                let mut args_list = Vec::new();
                for child in args.children(&mut cursor) {
                    if child.kind() != "(" && child.kind() != ")" && child.kind() != "," {
                        args_list.push(child);
                    }
                }
                // Check if second arg is g_free
                if args_list.len() == 2 {
                    let second_arg = ast_context.get_node_text(args_list[1], source);
                    if second_arg.trim() == "g_free" {
                        // First arg should be &var, strip the &
                        let first_arg = ast_context.get_node_text(args_list[0], source);
                        let var_name = first_arg.trim().trim_start_matches('&');
                        return Some(var_name.to_string());
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
        if let Some(assignment) = self.find_assignment(node)
            && let Some(left) = assignment.child_by_field_name("left")
        {
            let left_text = ast_context.get_node_text(left, source);
            if let Some(right) = assignment.child_by_field_name("right") {
                // Direct g_strdup call
                if right.kind() == "call_expression" {
                    if let Some(func) = right.child_by_field_name("function") {
                        let func_name = ast_context.get_node_text(func, source);
                        if (func_name == "g_strdup" || func_name == "g_strndup")
                            && let Some(args) = right.child_by_field_name("arguments")
                        {
                            let args_text = ast_context.get_node_text(args, source);
                            return Some((
                                left_text.trim().to_string(),
                                args_text.trim().to_string(),
                            ));
                        }
                    }
                }
                // Ternary: var ? g_strdup(var) : NULL
                else if right.kind() == "conditional_expression"
                    && let Some(consequence) = right.child_by_field_name("consequence")
                    && consequence.kind() == "call_expression"
                    && let Some(func) = consequence.child_by_field_name("function")
                {
                    let func_name = ast_context.get_node_text(func, source);
                    if func_name == "g_strdup" || func_name == "g_strndup" {
                        // For ternary, suggest the condition variable
                        if let Some(condition) = right.child_by_field_name("condition") {
                            let cond_text = ast_context.get_node_text(condition, source);
                            return Some((
                                left_text.trim().to_string(),
                                cond_text.trim().to_string(),
                            ));
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
