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
                && let Some(((var_name, new_val), first_stmt, second_stmt)) =
                    self.check_free_then_strdup(ast_context, body_node, ctx.source)
            {
                let position = first_stmt.start_position();

                // Bytes between the two statements may contain comments that
                // should be preserved in the fix.  Strip only the leading
                // newline+indentation (which belonged to the deleted free call's
                // line) and keep everything else — comments, trailing indentation
                // — as a prefix for the g_set_str replacement.
                let intermediate = std::str::from_utf8(
                    &ctx.source[first_stmt.end_byte()..second_stmt.start_byte()],
                )
                .unwrap_or("");
                let comment_prefix = intermediate.trim_start_matches(['\n', '\r', ' ', '\t']);

                let set_str_call = format!("g_set_str (&{}, {});", var_name, new_val);

                // If there are comments between the two statements, include them
                // in the fix so they are not deleted.  The message only shows
                // the g_set_str call itself, without the comment prefix.
                let fix_text = if comment_prefix.is_empty() {
                    set_str_call.clone()
                } else {
                    format!("{}{}", comment_prefix, set_str_call)
                };

                let fix = Fix::from_range(
                    first_stmt.start_byte(),
                    second_stmt.end_byte(),
                    ctx,
                    &fix_text,
                );

                violations.push(self.violation_with_fix(
                    ctx.file_path,
                    ctx.base_line + position.row,
                    position.column + 1,
                    format!("Use {} instead of g_free and g_strdup", set_str_call),
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

    /// Check for g_free + g_strdup where only comments (no other statements)
    /// appear between them.
    ///
    /// Returns ((var_name, new_val), first_stmt, second_stmt).
    fn check_free_then_strdup<'a>(
        &self,
        ast_context: &AstContext,
        compound: Node<'a>,
        source: &'a [u8],
    ) -> Option<((&'a str, &'a str), Node<'a>, Node<'a>)> {
        let mut cursor = compound.walk();
        let children: Vec<_> = compound.children(&mut cursor).collect();

        let mut i = 0;
        while i < children.len() {
            let child = children[i];

            // Look for a g_free / g_clear_pointer expression_statement.
            if child.kind() != "expression_statement" {
                i += 1;
                continue;
            }
            let Some(var_name) = self.extract_gfree_var(ast_context, child, source) else {
                i += 1;
                continue;
            };

            let first_stmt = child;

            // Advance past comment nodes only.  Any other intervening node
            // (for loop, if statement, another call, …) means the free and
            // the strdup are not truly adjacent and we must not merge them.
            let mut j = i + 1;
            while j < children.len() && children[j].kind() == "comment" {
                j += 1;
            }

            // The next non-comment must be the g_strdup assignment.
            if j < children.len() && children[j].kind() == "expression_statement" {
                let second_stmt = children[j];
                if let Some((assign_var, new_val)) =
                    self.extract_strdup_assignment(ast_context, second_stmt, source)
                    && assign_var.trim() == var_name.trim()
                {
                    return Some(((var_name, new_val), first_stmt, second_stmt));
                }
            }

            i += 1;
        }

        None
    }

    fn extract_gfree_var<'a>(
        &self,
        ast_context: &AstContext,
        node: Node,
        source: &'a [u8],
    ) -> Option<&'a str> {
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
                            return Some(ast_context.get_node_text(child, source).trim());
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
                        return Some(var_name);
                    }
                }
            }
        }
        None
    }

    fn extract_strdup_assignment<'a>(
        &self,
        ast_context: &AstContext,
        node: Node,
        source: &'a [u8],
    ) -> Option<(&'a str, &'a str)> {
        if let Some(assignment) = self.find_assignment(node)
            && let Some(left) = assignment.child_by_field_name("left")
        {
            let left_text = ast_context.get_node_text(left, source);
            if let Some(right) = assignment.child_by_field_name("right") {
                // Direct g_strdup call
                if right.kind() == "call_expression" {
                    if let Some(func) = right.child_by_field_name("function") {
                        let func_name = ast_context.get_node_text(func, source);
                        if func_name == "g_strdup"
                            && let Some(args) = right.child_by_field_name("arguments")
                        {
                            // Extract the argument node directly so we get clean text
                            // without the surrounding parens of the argument_list.
                            let mut cursor = args.walk();
                            if let Some(arg) = args
                                .children(&mut cursor)
                                .find(|n| n.kind() != "(" && n.kind() != ")" && n.kind() != ",")
                            {
                                let arg_text = ast_context.get_node_text(arg, source);
                                return Some((left_text.trim(), arg_text.trim()));
                            }
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
                            return Some((left_text.trim(), cond_text.trim()));
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
