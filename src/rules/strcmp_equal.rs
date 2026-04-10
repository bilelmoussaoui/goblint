use tree_sitter::Node;

use super::{CheckContext, Fix, Rule};
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct StrcmpForStringEqual;

impl Rule for StrcmpForStringEqual {
    fn name(&self) -> &'static str {
        "strcmp_for_string_equal"
    }

    fn description(&self) -> &'static str {
        "Suggest g_str_equal() instead of strcmp() == 0 for better readability"
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
                    self.check_node(ast_context, tree.root_node(), &ctx, violations);
                }
            }
        }
    }
}

impl StrcmpForStringEqual {
    fn check_node(
        &self,
        ast_context: &AstContext,
        node: Node,
        ctx: &CheckContext,
        violations: &mut Vec<Violation>,
    ) {
        // Look for binary expressions like: strcmp(a, b) == 0
        if node.kind() == "binary_expression"
            && let Some(operator) = node.child_by_field_name("operator")
        {
            let op_text = ast_context.get_node_text(operator, ctx.source);

            // Only care about == and != comparisons
            if op_text == "==" || op_text == "!=" {
                // Check left side
                if let Some(left) = node.child_by_field_name("left")
                    && let Some(right) = node.child_by_field_name("right")
                {
                    self.check_strcmp_comparison(
                        ast_context,
                        left,
                        right,
                        &op_text,
                        ctx,
                        node,
                        violations,
                    );
                    // Also check reverse: 0 == strcmp(a, b)
                    self.check_strcmp_comparison(
                        ast_context,
                        right,
                        left,
                        &op_text,
                        ctx,
                        node,
                        violations,
                    );
                }
            }
        }

        // Recurse into children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.check_node(ast_context, child, ctx, violations);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn check_strcmp_comparison(
        &self,
        ast_context: &AstContext,
        strcmp_side: Node,
        value_side: Node,
        operator: &str,
        ctx: &CheckContext,
        parent_node: Node,
        violations: &mut Vec<Violation>,
    ) {
        // Check if strcmp_side is a call to strcmp
        if strcmp_side.kind() != "call_expression" {
            return;
        }

        let Some(function) = strcmp_side.child_by_field_name("function") else {
            return;
        };

        let func_name = ast_context.get_node_text(function, ctx.source);
        if func_name != "strcmp" {
            return;
        }

        // Check if value_side is 0
        let value_text = ast_context
            .get_node_text(value_side, ctx.source)
            .trim()
            .to_string();
        if value_text != "0" {
            return;
        }

        // Extract the arguments
        if let Some(args) = strcmp_side.child_by_field_name("arguments") {
            // Preserve spacing between function name and arguments
            let spacing_start = function.end_byte();
            let spacing_end = args.start_byte();
            let spacing =
                std::str::from_utf8(&ctx.source[spacing_start..spacing_end]).unwrap_or("");

            let args_text = ast_context.get_node_text(args, ctx.source);

            // Build the replacement preserving original spacing
            let replacement = if operator == "==" {
                format!("g_str_equal{}{}", spacing, args_text)
            } else {
                format!("!g_str_equal{}{}", spacing, args_text)
            };

            let fix = Fix {
                start_byte: ctx.base_byte + parent_node.start_byte(),
                end_byte: ctx.base_byte + parent_node.end_byte(),
                replacement: replacement.clone(),
            };

            violations.push(self.violation_with_fix(
                ctx.file_path,
                ctx.base_line + parent_node.start_position().row,
                parent_node.start_position().column + 1,
                format!(
                    "Use {} instead of strcmp() {} 0 for string equality",
                    replacement, operator
                ),
                fix,
            ));
        }
    }
}
