use tree_sitter::Node;

use super::{CheckContext, Fix, Rule};
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct UseGClearWeakPointer;

impl Rule for UseGClearWeakPointer {
    fn name(&self) -> &'static str {
        "use_g_clear_weak_pointer"
    }

    fn description(&self) -> &'static str {
        "Suggest g_clear_weak_pointer instead of manual g_object_remove_weak_pointer and NULL assignment"
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

impl UseGClearWeakPointer {
    fn check_node(
        &self,
        ast_context: &AstContext,
        node: Node,
        ctx: &CheckContext,
        violations: &mut Vec<Violation>,
    ) {
        // Look for g_object_remove_weak_pointer followed by NULL assignment
        if node.kind() == "expression_statement"
            && let Some(call) = ast_context.find_call_expression(node)
            && let Some(function) = call.child_by_field_name("function")
        {
            let func_name = ast_context.get_node_text(function, ctx.source);

            if func_name == "g_object_remove_weak_pointer" {
                // Extract the variable being cleaned up
                if let Some(var_name) = self.extract_weak_pointer_var(ast_context, call, ctx.source)
                {
                    // Check if next statement is var = NULL
                    if let Some(parent) = node.parent()
                        && parent.kind() == "compound_statement"
                        && let Some(next_sibling) = self.find_next_statement(parent, node)
                        && self.is_null_assignment(ast_context, next_sibling, &var_name, ctx.source)
                    {
                        // Found the pattern! Create a fix
                        let replacement = format!("g_clear_weak_pointer (&{});", var_name);

                        let fix = Fix {
                            start_byte: ctx.base_byte + node.start_byte(),
                            end_byte: ctx.base_byte + next_sibling.end_byte(),
                            replacement: replacement.clone(),
                        };

                        violations.push(self.violation_with_fix(
                            ctx.file_path,
                            ctx.base_line + node.start_position().row,
                            node.start_position().column + 1,
                            format!(
                                "Use {} instead of g_object_remove_weak_pointer + NULL assignment",
                                replacement.trim_end_matches(';')
                            ),
                            fix,
                        ));
                    }
                }
            }
        }

        // Recurse
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.check_node(ast_context, child, ctx, violations);
        }
    }

    /// Extract variable name from g_object_remove_weak_pointer call
    /// Pattern: g_object_remove_weak_pointer(G_OBJECT(obj), (gpointer*)&obj)
    fn extract_weak_pointer_var(
        &self,
        ast_context: &AstContext,
        call: Node,
        source: &[u8],
    ) -> Option<String> {
        let args = call.child_by_field_name("arguments")?;

        // Collect arguments
        let mut cursor = args.walk();
        let mut arguments = Vec::new();
        for child in args.children(&mut cursor) {
            if child.kind() != "(" && child.kind() != ")" && child.kind() != "," {
                arguments.push(child);
            }
        }

        // Need at least 2 arguments
        if arguments.len() < 2 {
            return None;
        }

        // Second argument should be (gpointer*)&var or &var
        let second_arg_text = ast_context.get_node_text(arguments[1], source);

        // Remove casts and address-of operator
        let cleaned = second_arg_text
            .replace("(gpointer*)", "")
            .replace("(gpointer *)", "")
            .replace("&", "")
            .trim()
            .to_string();

        if cleaned.is_empty() {
            return None;
        }

        Some(cleaned)
    }

    /// Find the next statement sibling
    fn find_next_statement<'a>(&self, parent: Node<'a>, current: Node<'a>) -> Option<Node<'a>> {
        let mut cursor = parent.walk();
        let mut found_current = false;

        for child in parent.children(&mut cursor) {
            if found_current && child.kind().ends_with("_statement") {
                return Some(child);
            }
            if child == current {
                found_current = true;
            }
        }

        None
    }

    /// Check if a statement is var = NULL
    fn is_null_assignment(
        &self,
        ast_context: &AstContext,
        node: Node,
        var_name: &str,
        source: &[u8],
    ) -> bool {
        if node.kind() == "expression_statement" {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "assignment_expression"
                    && let Some(left) = child.child_by_field_name("left")
                {
                    let left_text = ast_context.get_node_text(left, source);
                    if left_text == var_name
                        && let Some(right) = child.child_by_field_name("right")
                    {
                        let right_text = ast_context.get_node_text(right, source);
                        if ast_context.is_null_literal(&right_text) {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }
}
