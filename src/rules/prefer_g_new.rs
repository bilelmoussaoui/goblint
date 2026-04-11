use tree_sitter::Node;

use super::{CheckContext, Fix, Rule};
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct PreferGNew;

impl Rule for PreferGNew {
    fn name(&self) -> &'static str {
        "prefer_g_new"
    }

    fn description(&self) -> &'static str {
        "Suggest g_new/g_new0 instead of g_malloc/g_malloc0 with sizeof for type safety"
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

impl PreferGNew {
    fn check_node(
        &self,
        ast_context: &AstContext,
        node: Node,
        ctx: &CheckContext,
        violations: &mut Vec<Violation>,
    ) {
        // Look for g_malloc/g_malloc0 calls
        if node.kind() == "call_expression"
            && let Some((malloc_func, type_name, call_node)) =
                self.extract_malloc_with_sizeof(ast_context, node, ctx.source)
        {
            let position = call_node.start_position();
            let suggested_func = if malloc_func == "g_malloc0" {
                "g_new0"
            } else {
                "g_new"
            };

            // Remove any parentheses from type name that might come from sizeof parsing
            let clean_type = type_name.trim_matches(|c| c == '(' || c == ')');
            let replacement = format!("{} ({}, 1)", suggested_func, clean_type);

            let fix = Fix::from_node(call_node, ctx, &replacement);

            violations.push(self.violation_with_fix(
                ctx.file_path,
                ctx.base_line + position.row,
                position.column + 1,
                format!(
                    "Use {} instead of {}(sizeof({})) for type safety",
                    replacement, malloc_func, type_name
                ),
                fix,
            ));
        }

        // Recurse
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.check_node(ast_context, child, ctx, violations);
        }
    }

    /// Extract g_malloc/g_malloc0 call with sizeof argument
    /// Returns (malloc_function, type_name, call_node)
    fn extract_malloc_with_sizeof<'a>(
        &self,
        ast_context: &AstContext,
        call_node: Node<'a>,
        source: &[u8],
    ) -> Option<(&'static str, String, Node<'a>)> {
        let function = call_node.child_by_field_name("function")?;
        let func_name = ast_context.get_node_text(function, source);

        let malloc_func = match func_name.as_str() {
            "g_malloc" => "g_malloc",
            "g_malloc0" => "g_malloc0",
            _ => return None,
        };

        // Get the arguments
        let args = call_node.child_by_field_name("arguments")?;

        // Look for sizeof(...) as the argument
        let mut cursor = args.walk();
        for child in args.children(&mut cursor) {
            if child.kind() == "sizeof_expression"
                && let Some(type_name) = self.extract_sizeof_type(ast_context, child, source)
            {
                return Some((malloc_func, type_name, call_node));
            }
        }

        None
    }

    /// Extract the type from sizeof(Type) or sizeof (Type)
    fn extract_sizeof_type(
        &self,
        ast_context: &AstContext,
        sizeof_node: Node,
        source: &[u8],
    ) -> Option<String> {
        // sizeof can have different children depending on whether it's sizeof(type) or
        // sizeof(expr) We want to extract the type
        let mut cursor = sizeof_node.walk();
        for child in sizeof_node.children(&mut cursor) {
            // Skip the 'sizeof' keyword and parentheses
            if child.kind() == "sizeof" || child.kind() == "(" || child.kind() == ")" {
                continue;
            }

            // Get the type or expression
            let text = ast_context.get_node_text(child, source);

            // Clean up the type name (remove spaces, handle pointer types)
            let cleaned = text.trim().to_string();

            // Don't suggest for complex expressions, only simple types
            if !cleaned.contains('+')
                && !cleaned.contains('-')
                && !cleaned.contains('*')
                && !cleaned.contains('/')
                && !cleaned.contains('[')
            {
                return Some(cleaned);
            }
        }

        None
    }
}
