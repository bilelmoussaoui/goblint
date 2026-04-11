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
                    self.check_node(tree.root_node(), &ctx, violations);
                }
            }
        }
    }
}

impl UseGStrcmp0 {
    fn check_node(&self, node: Node, ctx: &CheckContext, violations: &mut Vec<Violation>) {
        if node.kind() == "call_expression"
            && let Some(function) = node.child_by_field_name("function")
        {
            let func_text = &ctx.source[function.byte_range()];
            if let Ok(func_name) = std::str::from_utf8(func_text) {
                if func_name == "strcmp" {
                    // Only auto-fix strcmp, not strncmp (strncmp needs manual review)
                    let fix = Fix::from_node(function, ctx, "g_strcmp0");

                    violations.push(self.violation_with_fix(
                        ctx.file_path,
                        ctx.base_line + node.start_position().row,
                        node.start_position().column + 1,
                        "Use g_strcmp0 instead of strcmp (NULL-safe)".to_string(),
                        fix,
                    ));
                } else if func_name == "strncmp" {
                    // strncmp is trickier - don't auto-fix
                    violations.push(
                        self.violation(
                            ctx.file_path,
                            ctx.base_line + node.start_position().row,
                            node.start_position().column + 1,
                            "Use g_strcmp0 or check for NULL first instead of strncmp (NULL-safe)"
                                .to_string(),
                        ),
                    );
                }
            }
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.check_node(child, ctx, violations);
        }
    }
}
