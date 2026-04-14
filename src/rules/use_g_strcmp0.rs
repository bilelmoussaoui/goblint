use tree_sitter::Node;

use super::{CheckContext, Fix, Rule};
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct UseGStrcmp0;

impl Rule for UseGStrcmp0 {
    fn name(&self) -> &'static str {
        "use_g_strcmp0"
    }

    fn description(&self) -> &'static str {
        "Use g_strcmp0 instead of strcmp (NULL-safe)"
    }

    fn category(&self) -> super::Category {
        super::Category::Style
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
                    self.check_node(ast_context, tree.root_node(), None, &ctx, violations);
                }
            }
        }
    }
}

impl UseGStrcmp0 {
    fn check_node(
        &self,
        ast_context: &AstContext,
        node: Node,
        parent: Option<Node>,
        ctx: &CheckContext,
        violations: &mut Vec<Violation>,
    ) {
        if node.kind() == "call_expression"
            && let Some(function) = node.child_by_field_name("function")
        {
            let func_name = ast_context.get_node_text(function, ctx.source);
            if func_name == "strcmp" || func_name == "g_strcmp0" {
                // Check if it's used in a proper comparison context
                let (in_comparison, operator) = if let Some(p) = parent {
                    if p.kind() == "binary_expression"
                        && let Some(op) = p.child_by_field_name("operator")
                    {
                        let op_text = ast_context.get_node_text(op, ctx.source);
                        (true, Some(op_text))
                    } else {
                        (false, None)
                    }
                } else {
                    (false, None)
                };

                // Detect misuse: if (strcmp(a, b)) or if (!strcmp(a, b))
                if !in_comparison {
                    // Check if we're in a conditional context by traversing up the tree
                    let mut current = parent;
                    let mut is_negated = false;
                    let mut found_condition = false;

                    while let Some(p) = current {
                        if p.kind() == "unary_expression" {
                            is_negated = true;
                        } else if p.kind() == "if_statement"
                            || p.kind() == "while_statement"
                            || p.kind() == "for_statement"
                        {
                            found_condition = true;
                            break;
                        } else if p.kind() == "binary_expression" {
                            // Stop if we hit a binary expression (we're part of a larger
                            // comparison)
                            break;
                        }
                        current = p.parent();
                    }

                    if found_condition || is_negated {
                        violations.push(self.violation(
                            ctx.file_path,
                            ctx.base_line + node.start_position().row,
                            node.start_position().column + 1,
                            format!(
                                "{}() returns 0 for equality — use '{}(...) == 0' or '{}(...) != 0' instead of bare boolean check",
                                func_name, func_name, func_name
                            ),
                        ));
                    }
                }

                // Only suggest g_strcmp0 for strcmp (not for g_strcmp0 itself)
                if func_name == "strcmp" && in_comparison {
                    let message = if let Some(op) = operator
                        && (op == "==" || op == "!=")
                    {
                        "Consider g_strcmp0 instead of strcmp if arguments can be NULL (g_strcmp0 is NULL-safe)"
                    } else {
                        "Consider g_strcmp0 instead of strcmp if arguments can be NULL (g_strcmp0 is NULL-safe)"
                    };

                    let fix = Fix::from_node(function, ctx, "g_strcmp0");
                    violations.push(self.violation_with_fix(
                        ctx.file_path,
                        ctx.base_line + node.start_position().row,
                        node.start_position().column + 1,
                        message.to_string(),
                        fix,
                    ));
                }
            } else if func_name == "strncmp" {
                // strncmp is trickier — don't auto-fix
                violations.push(
                    self.violation(
                        ctx.file_path,
                        ctx.base_line + node.start_position().row,
                        node.start_position().column + 1,
                        "Consider g_strcmp0 or check for NULL first instead of strncmp (if NULL-safety needed)"
                            .to_string(),
                    ),
                );
            }
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.check_node(ast_context, child, Some(node), ctx, violations);
        }
    }
}
