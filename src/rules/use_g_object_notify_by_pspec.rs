use tree_sitter::Node;

use super::Rule;
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct UseGObjectNotifyByPspec;

impl Rule for UseGObjectNotifyByPspec {
    fn name(&self) -> &'static str {
        "use_g_object_notify_by_pspec"
    }

    fn description(&self) -> &'static str {
        "Suggest g_object_notify_by_pspec instead of g_object_notify for better performance"
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

impl UseGObjectNotifyByPspec {
    fn check_node(
        &self,
        ast_context: &AstContext,
        node: Node,
        source: &[u8],
        file_path: &std::path::Path,
        base_line: usize,
        violations: &mut Vec<Violation>,
    ) {
        // Look for g_object_notify calls
        if node.kind() == "call_expression" {
            if let Some(property_name) =
                self.extract_g_object_notify_with_string(ast_context, node, source)
            {
                let position = node.start_position();

                // Convert property-name to PROP_NAME for the suggestion
                let property_constant = self.property_name_to_constant(&property_name);

                violations.push(self.violation(
                    file_path,
                    base_line + position.row,
                    position.column + 1,
                    format!(
                        "Use g_object_notify_by_pspec(obj, properties[{}]) instead of g_object_notify(obj, \"{}\") for better performance",
                        property_constant, property_name
                    ),
                ));
            }
        }

        // Recurse
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.check_node(ast_context, child, source, file_path, base_line, violations);
        }
    }

    /// Extract g_object_notify call with string literal property name
    /// Returns the property name if it's a string literal
    fn extract_g_object_notify_with_string(
        &self,
        ast_context: &AstContext,
        call_node: Node,
        source: &[u8],
    ) -> Option<String> {
        let function = call_node.child_by_field_name("function")?;
        let func_name = ast_context.get_node_text(function, source);

        if func_name != "g_object_notify" {
            return None;
        }

        // Get the arguments
        let args = call_node.child_by_field_name("arguments")?;

        // Collect all arguments (skip parentheses and commas)
        let mut cursor = args.walk();
        let mut arguments = Vec::new();
        for child in args.children(&mut cursor) {
            if child.kind() != "(" && child.kind() != ")" && child.kind() != "," {
                arguments.push(child);
            }
        }

        // We need exactly 2 arguments: object and property name
        if arguments.len() != 2 {
            return None;
        }

        let property_arg = arguments[1];

        // Check if it's a string literal
        if property_arg.kind() == "string_literal" {
            let property_text = ast_context.get_node_text(property_arg, source);
            // Remove quotes
            let property_name = property_text.trim_matches('"').to_string();
            return Some(property_name);
        }

        None
    }

    /// Convert property-name to PROP_NAME constant style
    fn property_name_to_constant(&self, property_name: &str) -> String {
        // Convert kebab-case or camelCase to UPPER_SNAKE_CASE
        let mut result = String::with_capacity(property_name.len() + 5);
        result.push_str("PROP_");

        for c in property_name.chars() {
            if c == '-' {
                result.push('_');
            } else {
                result.push(c.to_ascii_uppercase());
            }
        }

        result
    }
}
