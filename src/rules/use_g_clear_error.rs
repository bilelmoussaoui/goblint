use tree_sitter::Node;

use super::Rule;
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct SuggestGAutoptrError;

impl Rule for SuggestGAutoptrError {
    fn name(&self) -> &'static str {
        "suggest_g_autoptr_error"
    }

    fn description(&self) -> &'static str {
        "Suggest g_autoptr(GError) instead of manual g_error_free"
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

impl SuggestGAutoptrError {
    fn check_node(
        &self,
        ast_context: &AstContext,
        node: Node,
        source: &[u8],
        file_path: &std::path::Path,
        base_line: usize,
        violations: &mut Vec<Violation>,
    ) {
        // Look for GError* variable declarations
        if node.kind() == "declaration" {
            if let Some((var_name, decl_node)) =
                self.find_gerror_declaration(ast_context, node, source)
            {
                // Check if this variable is manually freed with g_error_free in the function
                // We need to search the parent scope (function body)
                if let Some(function_body) = self.find_parent_function_body(node) {
                    if self.has_error_free_call(ast_context, function_body, &var_name, source) {
                        let position = decl_node.start_position();
                        violations.push(self.violation(
                            file_path,
                            base_line + position.row,
                            position.column + 1,
                            format!("Consider using g_autoptr(GError) {} instead of manual g_error_free", var_name),
                        ));
                    }
                }
            }
        }

        // Recurse
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.check_node(ast_context, child, source, file_path, base_line, violations);
        }
    }

    fn find_gerror_declaration<'a>(
        &self,
        ast_context: &AstContext,
        node: Node<'a>,
        source: &[u8],
    ) -> Option<(String, Node<'a>)> {
        // Look for: GError *var_name = NULL;
        // Tree structure: declaration -> type: pointer_declarator -> declarator:
        // identifier

        // Get the type specifier
        if let Some(type_node) = node.child_by_field_name("type") {
            let type_text = ast_context.get_node_text(type_node, source);
            if type_text.contains("GError") {
                // Find the declarator
                if let Some(declarator) = node.child_by_field_name("declarator") {
                    if let Some(var_name) =
                        self.extract_pointer_var_name(ast_context, declarator, source)
                    {
                        return Some((var_name, node));
                    }
                }
            }
        }

        None
    }

    fn extract_pointer_var_name(
        &self,
        ast_context: &AstContext,
        node: Node,
        source: &[u8],
    ) -> Option<String> {
        // Handle pointer_declarator and init_declarator
        match node.kind() {
            "pointer_declarator" => {
                if let Some(declarator) = node.child_by_field_name("declarator") {
                    return Some(ast_context.get_node_text(declarator, source));
                }
            }
            "init_declarator" => {
                if let Some(declarator) = node.child_by_field_name("declarator") {
                    return self.extract_pointer_var_name(ast_context, declarator, source);
                }
            }
            "identifier" => {
                return Some(ast_context.get_node_text(node, source));
            }
            _ => {}
        }
        None
    }

    fn find_parent_function_body<'a>(&self, mut node: Node<'a>) -> Option<Node<'a>> {
        // Walk up the tree to find the function_definition
        loop {
            if let Some(parent) = node.parent() {
                if parent.kind() == "function_definition" {
                    return parent.child_by_field_name("body");
                }
                node = parent;
            } else {
                return None;
            }
        }
    }

    fn has_error_free_call(
        &self,
        ast_context: &AstContext,
        body: Node,
        var_name: &str,
        source: &[u8],
    ) -> bool {
        self.find_error_free_call(ast_context, body, var_name, source)
    }

    fn find_error_free_call(
        &self,
        ast_context: &AstContext,
        node: Node,
        var_name: &str,
        source: &[u8],
    ) -> bool {
        let (is_cleanup, func_name) = ast_context.is_cleanup_call(node, source);
        if is_cleanup && func_name == "g_error_free" {
            if let Some(arguments) = node.child_by_field_name("arguments") {
                let args_text = ast_context.get_node_text(arguments, source);
                if args_text.contains(var_name) {
                    return true;
                }
            }
        }

        // Recursively check children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if self.find_error_free_call(ast_context, child, var_name, source) {
                return true;
            }
        }

        false
    }
}
