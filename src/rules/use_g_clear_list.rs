use tree_sitter::Node;

use super::{CheckContext, Fix, Rule};
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct UseGClearList;

impl Rule for UseGClearList {
    fn name(&self) -> &'static str {
        "use_g_clear_list"
    }

    fn description(&self) -> &'static str {
        "Suggest g_clear_list/g_clear_slist instead of manual g_list_free/g_slist_free and NULL assignment"
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

impl UseGClearList {
    fn check_node(
        &self,
        ast_context: &AstContext,
        node: Node,
        ctx: &CheckContext,
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
                    self.check_free_then_null(ast_context, body_node, ctx.source)
                {
                    let position = first_stmt.start_position();
                    let clear_fn = if list_type == "GList" {
                        "g_clear_list"
                    } else {
                        "g_clear_slist"
                    };

                    let replacement = format!("{} (&{}, NULL);", clear_fn, var_name);

                    let fix = Fix::from_range(
                        first_stmt.start_byte(),
                        second_stmt.end_byte(),
                        ctx,
                        &replacement,
                    );

                    violations.push(self.violation_with_fix(
                        ctx.file_path,
                        ctx.base_line + position.row,
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
            self.check_node(ast_context, child, ctx, violations);
        }
    }

    /// Check for consecutive g_list_free/g_slist_free + list = NULL
    /// Returns (var_name, list_type, first_statement, second_statement)
    fn check_free_then_null<'a>(
        &self,
        ast_context: &AstContext,
        compound: Node<'a>,
        source: &'a [u8],
    ) -> Vec<(&'a str, &'static str, Node<'a>, Node<'a>)> {
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
                    && assign_var.trim() == var_name.trim()
                {
                    results.push((var_name, list_type, first, second));
                }
            }
        }

        results
    }

    fn extract_list_free<'a>(
        &self,
        ast_context: &AstContext,
        node: Node,
        source: &'a [u8],
    ) -> Option<(&'a str, &'static str)> {
        if let Some(call) = ast_context.find_call_expression(node)
            && let Some(function) = call.child_by_field_name("function")
        {
            let func_name = ast_context.get_node_text(function, source);

            let list_type = match func_name {
                "g_list_free" => "GList",
                "g_slist_free" => "GSList",
                _ => return None,
            };

            // Get the first argument (the list variable)
            if let Some(args) = call.child_by_field_name("arguments") {
                let mut cursor = args.walk();
                for child in args.children(&mut cursor) {
                    if child.kind() != "(" && child.kind() != ")" && child.kind() != "," {
                        return Some((ast_context.get_node_text(child, source).trim(), list_type));
                    }
                }
            }
        }
        None
    }

    fn extract_null_assignment<'a>(
        &self,
        ast_context: &AstContext,
        node: Node,
        source: &'a [u8],
    ) -> Option<&'a str> {
        if let Some(assignment) = self.find_assignment(node)
            && let Some(left) = assignment.child_by_field_name("left")
            && let Some(right) = assignment.child_by_field_name("right")
        {
            let right_text = ast_context.get_node_text(right, source);
            if right_text.trim() == "NULL" {
                return Some(ast_context.get_node_text(left, source).trim());
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
