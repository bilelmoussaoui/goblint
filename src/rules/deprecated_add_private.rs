use tree_sitter::Node;

use super::Rule;
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct DeprecatedAddPrivate;

impl Rule for DeprecatedAddPrivate {
    fn name(&self) -> &'static str {
        "deprecated_add_private"
    }

    fn description(&self) -> &'static str {
        "Detect deprecated g_type_class_add_private (use G_DEFINE_TYPE_WITH_PRIVATE instead)"
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
                    self.check_node(tree.root_node(), func_source, path, func.line, violations);
                }
            }
        }
    }
}
impl DeprecatedAddPrivate {
    fn check_node(
        &self,
        node: Node,
        source: &[u8],
        file_path: &std::path::Path,
        base_line: usize,
        violations: &mut Vec<Violation>,
    ) {
        if node.kind() == "call_expression"
            && let Some(function) = node.child_by_field_name("function")
        {
            let func_text = &source[function.byte_range()];
            if let Ok(text) = std::str::from_utf8(func_text)
                && text == "g_type_class_add_private"
            {
                violations.push(self.violation(file_path, base_line + node.start_position().row, node.start_position().column + 1, "g_type_class_add_private is deprecated since GLib 2.58. Use G_DEFINE_TYPE_WITH_PRIVATE or G_ADD_PRIVATE instead".to_owned()));
            }
        }

        // Recurse into children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.check_node(child, source, file_path, base_line, violations);
        }
    }
}
