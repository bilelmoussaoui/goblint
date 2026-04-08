use tree_sitter::Node;

use super::Rule;
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct UseGStringFreeAndSteal;

impl Rule for UseGStringFreeAndSteal {
    fn name(&self) -> &'static str {
        "use_g_string_free_and_steal"
    }

    fn description(&self) -> &'static str {
        "Suggests g_string_free_and_steal instead of g_string_free (..., FALSE) for better readability"
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

                if let Some(func_source) = ast_context.get_function_source(path, func) {
                    if let Some(tree) = ast_context.parse_c_source(func_source) {
                        self.check_node(
                            ast_context,
                            tree.root_node(),
                            func_source,
                            path,
                            func.line,
                            violations,
                        );
                    }
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
        source: &[u8],
        file_path: &std::path::Path,
        base_line: usize,
        violations: &mut Vec<Violation>,
    ) {
        if let Some(call) = ast_context.find_function_call_by_name(node, &["g_string_free"], source)
        {
            if let Some(args) = call.child_by_field_name("arguments") {
                let mut cursor = args.walk();
                let mut children = args
                    .children(&mut cursor)
                    .filter(|c| !matches!(c.kind(), "(" | ")" | ","));
                if let (Some(first), Some(second)) = (children.next(), children.next()) {
                    let second = ast_context.get_node_text(second, source);

                    if matches!(second.as_str(), "FALSE" | "false" | "0") {
                        let first = ast_context.get_node_text(first, source);

                        let position = call.start_position();
                        violations.push(self.violation(
                            file_path,
                            base_line + position.row,
                            position.column + 1,
                            format!(
                                "Consider using g_string_free_and_steal({first}) instead of g_string_free({first}, {second}) for readability",
                            ),
                        ));
                    }
                }
            }
        }
    }
}
