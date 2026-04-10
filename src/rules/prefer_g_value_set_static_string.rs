use tree_sitter::Node;

use super::{CheckContext, Fix, Rule};
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct PreferGValueSetStaticString;

impl Rule for PreferGValueSetStaticString {
    fn name(&self) -> &'static str {
        "prefer_g_value_set_static_string"
    }

    fn description(&self) -> &'static str {
        "Use g_value_set_static_string for string literals instead of g_value_set_string"
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

impl PreferGValueSetStaticString {
    fn check_node(
        &self,
        ast_context: &AstContext,
        node: Node,
        ctx: &CheckContext,
        violations: &mut Vec<Violation>,
    ) {
        // Look for g_value_set_string calls
        if node.kind() == "call_expression"
            && let Some(function) = node.child_by_field_name("function")
        {
            let func_name = ast_context.get_node_text(function, ctx.source);
            if func_name == "g_value_set_string" {
                // Check if the second argument is a string literal
                if let Some(args) = node.child_by_field_name("arguments") {
                    let arguments = self.collect_arguments(args);

                    if arguments.len() >= 2 {
                        let second_arg = arguments[1];

                        // Check if it's a string literal
                        if second_arg.kind() == "string_literal" {
                            let fix = Fix {
                                start_byte: ctx.base_byte + function.start_byte(),
                                end_byte: ctx.base_byte + function.end_byte(),
                                replacement: "g_value_set_static_string".to_string(),
                            };

                            let string_value = ast_context.get_node_text(second_arg, ctx.source);

                            violations.push(self.violation_with_fix(
                                ctx.file_path,
                                ctx.base_line + node.start_position().row,
                                node.start_position().column + 1,
                                format!(
                                    "Use g_value_set_static_string instead of g_value_set_string for string literal {}",
                                    string_value
                                ),
                                fix,
                            ));
                        }
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

    fn collect_arguments<'a>(&self, args_node: Node<'a>) -> Vec<Node<'a>> {
        let mut cursor = args_node.walk();
        let mut arguments = Vec::new();
        for child in args_node.children(&mut cursor) {
            if child.kind() != "(" && child.kind() != ")" && child.kind() != "," {
                arguments.push(child);
            }
        }
        arguments
    }
}
