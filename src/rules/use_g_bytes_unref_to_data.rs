use tree_sitter::Node;

use super::{CheckContext, Fix, Rule};
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct UseGBytesUnrefToData;

impl Rule for UseGBytesUnrefToData {
    fn name(&self) -> &'static str {
        "use_g_bytes_unref_to_data"
    }

    fn description(&self) -> &'static str {
        "Use g_bytes_unref_to_data() instead of g_bytes_get_data() + g_bytes_unref()"
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

impl UseGBytesUnrefToData {
    fn check_node(
        &self,
        ast_context: &AstContext,
        node: Node,
        ctx: &CheckContext,
        violations: &mut Vec<Violation>,
    ) {
        if node.kind() == "compound_statement" {
            self.check_compound(ast_context, node, ctx, violations);
            return;
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.check_node(ast_context, child, ctx, violations);
        }
    }

    fn check_compound(
        &self,
        ast_context: &AstContext,
        node: Node,
        ctx: &CheckContext,
        violations: &mut Vec<Violation>,
    ) {
        if node.kind() == "compound_statement" {
            let mut cursor = node.walk();
            let stmts: Vec<Node> = node
                .children(&mut cursor)
                .filter(|n| n.kind() != "{" && n.kind() != "}" && n.kind() != "comment")
                .collect();

            // Look for consecutive pairs of statements
            for i in 0..stmts.len() {
                // Try to match pattern starting at position i
                if i + 1 < stmts.len() {
                    self.try_bytes_pattern(ast_context, stmts[i], stmts[i + 1], ctx, violations);
                }

                // Also recurse into this statement to find nested patterns
                self.check_node(ast_context, stmts[i], ctx, violations);
            }
        }
    }

    /// Try to match: dest = g_bytes_get_data(bytes, ...); g_bytes_unref(bytes);
    fn try_bytes_pattern(
        &self,
        ast_context: &AstContext,
        stmt1: Node,
        stmt2: Node,
        ctx: &CheckContext,
        violations: &mut Vec<Violation>,
    ) -> bool {
        // First statement should be: dest = g_bytes_get_data(bytes, ...)
        let Some((dest, bytes_var, size_arg)) =
            self.extract_bytes_get_data(ast_context, stmt1, ctx.source)
        else {
            return false;
        };

        // Second statement should be: g_bytes_unref(bytes);
        if !self.is_bytes_unref(ast_context, stmt2, bytes_var, ctx.source) {
            return false;
        }

        // Build the replacement
        let replacement = format!(
            "{} = g_bytes_unref_to_data ({}, {});",
            dest, bytes_var, size_arg
        );
        let fix = Fix::from_range(stmt1.start_byte(), stmt2.end_byte(), ctx, &replacement);

        violations.push(self.violation_with_fix(
            ctx.file_path,
            ctx.base_line + stmt1.start_position().row,
            stmt1.start_position().column + 1,
            format!(
                "Use g_bytes_unref_to_data({}, {}) instead of g_bytes_get_data() followed by g_bytes_unref()",
                bytes_var, size_arg
            ),
            fix,
        ));

        true
    }

    /// Extract components from: dest = g_bytes_get_data(bytes, size_arg)
    /// Returns (dest_text, bytes_var, size_arg)
    fn extract_bytes_get_data<'a>(
        &self,
        ast_context: &AstContext,
        stmt: Node,
        source: &'a [u8],
    ) -> Option<(&'a str, &'a str, &'a str)> {
        if stmt.kind() != "expression_statement" {
            return None;
        }

        let mut cursor = stmt.walk();
        let assign = stmt
            .children(&mut cursor)
            .find(|n| n.kind() == "assignment_expression")?;

        let op = assign.child_by_field_name("operator")?;
        if ast_context.get_node_text(op, source) != "=" {
            return None;
        }

        let left = assign.child_by_field_name("left")?;
        let right = assign.child_by_field_name("right")?;

        // Right side should be a call to g_bytes_get_data
        if right.kind() != "call_expression" {
            return None;
        }

        let function = right.child_by_field_name("function")?;
        if ast_context.get_node_text(function, source) != "g_bytes_get_data" {
            return None;
        }

        // Extract arguments: g_bytes_get_data(bytes, &size)
        let args = right.child_by_field_name("arguments")?;
        let mut cursor = args.walk();
        let arg_nodes: Vec<Node> = args
            .children(&mut cursor)
            .filter(|n| n.kind() != "(" && n.kind() != ")" && n.kind() != ",")
            .collect();

        if arg_nodes.len() != 2 {
            return None;
        }

        let bytes_var = ast_context.get_node_text(arg_nodes[0], source);
        let size_arg = ast_context.get_node_text(arg_nodes[1], source);
        let dest = ast_context.get_node_text(left, source);

        Some((dest, bytes_var, size_arg))
    }

    /// Check if statement is: g_bytes_unref(expected_var);
    fn is_bytes_unref(
        &self,
        ast_context: &AstContext,
        stmt: Node,
        expected_var: &str,
        source: &[u8],
    ) -> bool {
        if stmt.kind() != "expression_statement" {
            return false;
        }

        let mut cursor = stmt.walk();
        let call = stmt
            .children(&mut cursor)
            .find(|n| n.kind() == "call_expression");

        let Some(call) = call else {
            return false;
        };

        let Some(function) = call.child_by_field_name("function") else {
            return false;
        };

        if ast_context.get_node_text(function, source) != "g_bytes_unref" {
            return false;
        }

        // Check that the argument matches the bytes variable
        let Some(args) = call.child_by_field_name("arguments") else {
            return false;
        };

        let mut cursor = args.walk();
        let arg_nodes: Vec<Node> = args
            .children(&mut cursor)
            .filter(|n| n.kind() != "(" && n.kind() != ")" && n.kind() != ",")
            .collect();

        if arg_nodes.len() != 1 {
            return false;
        }

        let arg_text = ast_context.get_node_text(arg_nodes[0], source);
        arg_text == expected_var
    }
}
