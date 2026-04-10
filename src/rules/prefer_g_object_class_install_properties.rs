use tree_sitter::Node;

use super::{CheckContext, Rule};
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct PreferGObjectClassInstallProperties;

impl Rule for PreferGObjectClassInstallProperties {
    fn name(&self) -> &'static str {
        "prefer_g_object_class_install_properties"
    }

    fn description(&self) -> &'static str {
        "Suggest g_object_class_install_properties for multiple g_object_class_install_property calls"
    }

    fn fixable(&self) -> bool {
        false // Complex refactoring, needs manual intervention
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

                // Only check functions ending with _class_init
                if !func.name.ends_with("_class_init") {
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
                    self.check_class_init_function(ast_context, tree.root_node(), &ctx, violations);
                }
            }
        }
    }
}

impl PreferGObjectClassInstallProperties {
    /// Check a _class_init function and count install_property calls
    fn check_class_init_function(
        &self,
        ast_context: &AstContext,
        node: Node,
        ctx: &CheckContext,
        violations: &mut Vec<Violation>,
    ) {
        let mut install_property_calls = Vec::new();
        self.collect_install_property_calls(ast_context, node, ctx, &mut install_property_calls);

        if install_property_calls.len() >= 2 {
            let first_call = install_property_calls[0];
            violations.push(self.violation(
                ctx.file_path,
                ctx.base_line + first_call.start_position().row,
                first_call.start_position().column + 1,
                format!(
                    "Consider using g_object_class_install_properties() instead of {} g_object_class_install_property() calls",
                    install_property_calls.len()
                ),
            ));
        }
    }

    /// Recursively collect all g_object_class_install_property calls
    fn collect_install_property_calls<'a>(
        &self,
        ast_context: &AstContext,
        node: Node<'a>,
        ctx: &CheckContext,
        calls: &mut Vec<Node<'a>>,
    ) {
        if node.kind() == "call_expression"
            && let Some(function) = node.child_by_field_name("function")
        {
            let func_name = ast_context.get_node_text(function, ctx.source);
            if func_name == "g_object_class_install_property" {
                calls.push(node);
            }
        }

        // Recurse
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.collect_install_property_calls(ast_context, child, ctx, calls);
        }
    }
}
