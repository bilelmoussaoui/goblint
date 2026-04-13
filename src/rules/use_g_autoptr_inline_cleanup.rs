use std::collections::HashMap;

use tree_sitter::Node;

use super::Rule;
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct UseGAutoptrInlineCleanup;

impl Rule for UseGAutoptrInlineCleanup {
    fn name(&self) -> &'static str {
        "use_g_autoptr_inline_cleanup"
    }

    fn description(&self) -> &'static str {
        "Suggest g_autoptr instead of inline manual cleanup (g_object_unref/g_free)"
    }

    fn category(&self) -> super::Category {
        super::Category::Complexity
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
                    self.check_function(
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

impl UseGAutoptrInlineCleanup {
    fn check_function(
        &self,
        ast_context: &AstContext,
        node: Node,
        source: &[u8],
        file_path: &std::path::Path,
        base_line: usize,
        violations: &mut Vec<Violation>,
    ) {
        if let Some(body) = ast_context.find_body(node) {
            // Find all local pointer declarations
            let local_vars = self.find_local_pointer_vars(ast_context, body, source);

            // For each variable, check if it's a candidate for g_autoptr
            for (var_name, (var_type, decl_node)) in &local_vars {
                // Check if variable is allocated
                let is_allocated = self.is_var_allocated(ast_context, body, var_name, source);

                // Check if variable is manually freed
                let is_manually_freed =
                    self.is_var_manually_freed(ast_context, body, var_name, source);

                // Check if variable is returned without being freed
                let is_returned = self.is_var_returned(ast_context, body, var_name, source);

                // Suggest g_autoptr if:
                // 1. Variable is allocated
                // 2. Variable is manually freed at least once
                // 3. Variable is not returned directly (would need g_steal_pointer)
                if is_allocated && is_manually_freed && !is_returned {
                    let base_type = AstContext::extract_base_type(var_type);
                    let position = decl_node.start_position();
                    violations.push(self.violation(
                        file_path,
                        base_line + position.row,
                        position.column + 1,
                        format!(
                            "Consider using g_autoptr({}) {} to avoid manual cleanup",
                            base_type, var_name
                        ),
                    ));
                }
            }
        }
    }

    fn find_local_pointer_vars<'a>(
        &self,
        ast_context: &AstContext,
        body: Node<'a>,
        source: &'a [u8],
    ) -> HashMap<&'a str, (&'a str, Node<'a>)> {
        let mut result = HashMap::new();
        self.collect_local_vars(ast_context, body, source, &mut result);
        result
    }

    fn collect_local_vars<'a>(
        &self,
        ast_context: &AstContext,
        node: Node<'a>,
        source: &'a [u8],
        result: &mut HashMap<&'a str, (&'a str, Node<'a>)>,
    ) {
        // Only look at top-level declarations in the function body
        if node.kind() == "compound_statement" {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "declaration"
                    && let Some(type_node) = child.child_by_field_name("type")
                {
                    let type_text = ast_context.get_node_text(type_node, source);

                    // Skip variables already using g_autoptr/g_autofree
                    if type_text.contains("g_autoptr") || type_text.contains("g_autofree") {
                        continue;
                    }

                    // Find declarators
                    let mut decl_cursor = child.walk();
                    for decl_child in child.children(&mut decl_cursor) {
                        if (decl_child.kind() == "init_declarator"
                            || decl_child.kind() == "pointer_declarator")
                            && let Some(var_name) =
                                self.extract_var_name(ast_context, decl_child, source)
                        {
                            // Only simple identifiers
                            if !var_name.contains("->") && !var_name.contains(".") {
                                result.insert(var_name, (type_text, child));
                            }
                        }
                    }
                }
            }
        }
    }

    fn extract_var_name<'a>(
        &self,
        ast_context: &AstContext,
        node: Node,
        source: &'a [u8],
    ) -> Option<&'a str> {
        match node.kind() {
            "init_declarator" => {
                if let Some(declarator) = node.child_by_field_name("declarator") {
                    return self.extract_var_name(ast_context, declarator, source);
                }
            }
            "pointer_declarator" => {
                if let Some(declarator) = node.child_by_field_name("declarator") {
                    return self.extract_var_name(ast_context, declarator, source);
                }
            }
            "identifier" => {
                return Some(ast_context.get_node_text(node, source));
            }
            _ => {}
        }
        None
    }

    fn is_var_allocated(
        &self,
        ast_context: &AstContext,
        body: Node,
        var_name: &str,
        source: &[u8],
    ) -> bool {
        self.find_var_allocation(ast_context, body, var_name, source)
    }

    fn find_var_allocation(
        &self,
        ast_context: &AstContext,
        node: Node,
        var_name: &str,
        source: &[u8],
    ) -> bool {
        // Look for: var_name = allocation_call()
        if node.kind() == "assignment_expression"
            && let Some(left) = node.child_by_field_name("left")
        {
            let left_text = ast_context.get_node_text(left, source);
            if left_text == var_name
                && let Some(right) = node.child_by_field_name("right")
                && ast_context.is_allocation_call(right, source)
            {
                return true;
            }
        }

        // Also check init declarator: Type *var = allocation_call()
        if node.kind() == "init_declarator"
            && let Some(declarator) = node.child_by_field_name("declarator")
            && let Some(found_var) = self.extract_var_name(ast_context, declarator, source)
            && found_var == var_name
            && let Some(value) = node.child_by_field_name("value")
            && ast_context.is_allocation_call(value, source)
        {
            return true;
        }

        // Recurse
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if self.find_var_allocation(ast_context, child, var_name, source) {
                return true;
            }
        }

        false
    }

    fn is_var_manually_freed(
        &self,
        ast_context: &AstContext,
        body: Node,
        var_name: &str,
        source: &[u8],
    ) -> bool {
        self.find_manual_free(ast_context, body, var_name, source)
    }

    fn find_manual_free(
        &self,
        ast_context: &AstContext,
        node: Node,
        var_name: &str,
        source: &[u8],
    ) -> bool {
        let (is_cleanup, _) = ast_context.is_cleanup_call(node, source);
        if is_cleanup && let Some(arguments) = node.child_by_field_name("arguments") {
            let args_text = ast_context.get_node_text(arguments, source);
            if args_text.contains(var_name) {
                return true;
            }
        }

        // Recurse
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if self.find_manual_free(ast_context, child, var_name, source) {
                return true;
            }
        }

        false
    }

    fn is_var_returned(
        &self,
        ast_context: &AstContext,
        body: Node,
        var_name: &str,
        source: &[u8],
    ) -> bool {
        self.find_return_of_var(ast_context, body, var_name, source)
    }

    fn find_return_of_var(
        &self,
        ast_context: &AstContext,
        node: Node,
        var_name: &str,
        source: &[u8],
    ) -> bool {
        if node.kind() == "return_statement" {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "identifier" {
                    let id = ast_context.get_node_text(child, source);
                    if id == var_name {
                        return true;
                    }
                }
            }
        }

        // Recurse
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if self.find_return_of_var(ast_context, child, var_name, source) {
                return true;
            }
        }

        false
    }
}
