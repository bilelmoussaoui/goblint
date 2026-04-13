use tree_sitter::Node;

use super::{CheckContext, Fix, Rule};
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct UseGStringFreeAndSteal;

impl Rule for UseGStringFreeAndSteal {
    fn name(&self) -> &'static str {
        "use_g_string_free_and_steal"
    }

    fn description(&self) -> &'static str {
        "Suggest g_string_free_and_steal instead of g_string_free (..., FALSE) for better readability"
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

impl UseGStringFreeAndSteal {
    fn check_node(
        &self,
        ast_context: &AstContext,
        node: Node,
        ctx: &CheckContext,
        violations: &mut Vec<Violation>,
    ) {
        if let Some(call) =
            ast_context.find_function_call_by_name(node, &["g_string_free"], ctx.source)
            && let Some(args) = call.child_by_field_name("arguments")
        {
            let mut cursor = args.walk();
            let mut children = args
                .children(&mut cursor)
                .filter(|c| !matches!(c.kind(), "(" | ")" | ","));
            if let (Some(first), Some(second)) = (children.next(), children.next()) {
                let second = ast_context.get_node_text(second, ctx.source);

                if matches!(second, "FALSE" | "false" | "0") {
                    let first_text = ast_context.get_node_text(first, ctx.source);

                    // Build replacement with proper spacing
                    let replacement = format!("g_string_free_and_steal ({})", first_text);

                    let fix = Fix::from_node(call, ctx, &replacement);

                    let position = call.start_position();
                    violations.push(self.violation_with_fix(
                        ctx.file_path,
                        ctx.base_line + position.row,
                        position.column + 1,
                        format!(
                            "Use {} instead of g_string_free({}, {}) for readability",
                            replacement, first_text, second
                        ),
                        fix,
                    ));
                }
            }
        }
    }
}
