use std::collections::HashMap;

use tree_sitter::Node;

use super::Rule;
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct SuggestGAutofree;

impl Rule for SuggestGAutofree {
    fn name(&self) -> &'static str {
        "suggest_g_autofree"
    }

    fn description(&self) -> &'static str {
        "Suggest g_autofree for string/buffer types instead of manual g_free"
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

impl SuggestGAutofree {
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

            // For each variable, check if it's a candidate for g_autofree
            for (var_name, (var_type, decl_node)) in &local_vars {
                // Only suggest g_autofree for simple types (char*, guint8*, void*, etc.)
                // Not for GObject* types (those should use g_autoptr)
                if !self.is_autofree_candidate(var_type) {
                    continue;
                }

                // Check if variable is allocated
                let is_allocated = self.is_var_allocated(ast_context, body, var_name, source);

                // Check if variable is manually freed
                let is_manually_freed =
                    self.is_var_manually_freed(ast_context, body, var_name, source);

                // Check if variable is returned
                let is_returned = self.is_var_returned(ast_context, body, var_name, source);

                // Suggest g_autofree if:
                // 1. Variable is allocated
                // 2. Variable is manually freed
                // 3. Variable is not returned (would need g_steal_pointer)
                if is_allocated && is_manually_freed && !is_returned {
                    let position = decl_node.start_position();
                    violations.push(self.violation(
                        file_path,
                        base_line + position.row,
                        position.column + 1,
                        format!(
                            "Consider using g_autofree {} to avoid manual g_free",
                            var_name
                        ),
                    ));
                }
            }
        }
    }

    fn is_autofree_candidate(&self, var_type: &str) -> bool {
        let type_lower = var_type.to_lowercase();

        // g_autofree is for simple pointer types: char*, guint8*, void*, etc.
        // Not for GObject-derived types (those use g_autoptr)

        // Simple types that should use g_autofree
        if type_lower.contains("char")
            || type_lower.contains("guint8")
            || type_lower.contains("gint8")
            || type_lower.contains("guchar")
            || type_lower.contains("gchar")
            || type_lower.contains("uint8_t")
            || type_lower.contains("int8_t")
            || type_lower.contains("void")
        {
            return true;
        }

        // Skip GObject types - these should use g_autoptr instead
        // Common GObject patterns: GType*, GObject*, anything with G[A-Z][a-z]*
        if var_type.contains("GError")
            || var_type.contains("GObject")
            || var_type.contains("GList")
            || var_type.contains("GSList")
            || var_type.contains("GHashTable")
            || var_type.contains("GBytes")
            || var_type.contains("GVariant")
            || var_type.contains("GArray")
        {
            return false;
        }

        // Check for custom types (like CoglTexture, MetaWindow, etc.)
        // These should use g_autoptr
        if var_type.chars().next().is_some_and(|c| c.is_uppercase()) {
            // If starts with uppercase and contains mixed case, likely an object type
            if var_type.chars().any(|c| c.is_lowercase()) {
                return false;
            }
        }

        false
    }

    fn find_local_pointer_vars<'a>(
        &self,
        ast_context: &AstContext,
        body: Node<'a>,
        source: &[u8],
    ) -> HashMap<String, (String, Node<'a>)> {
        let mut result = HashMap::new();
        self.collect_local_vars(ast_context, body, source, &mut result);
        result
    }

    fn collect_local_vars<'a>(
        &self,
        ast_context: &AstContext,
        node: Node<'a>,
        source: &[u8],
        result: &mut HashMap<String, (String, Node<'a>)>,
    ) {
        if node.kind() == "compound_statement" {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "declaration"
                    && let Some(type_node) = child.child_by_field_name("type")
                {
                    let type_text = ast_context.get_node_text(type_node, source);

                    let mut decl_cursor = child.walk();
                    for decl_child in child.children(&mut decl_cursor) {
                        if (decl_child.kind() == "init_declarator"
                            || decl_child.kind() == "pointer_declarator")
                            && let Some(var_name) =
                                self.extract_var_name(ast_context, decl_child, source)
                            && !var_name.contains("->")
                            && !var_name.contains(".")
                        {
                            result.insert(var_name, (type_text.clone(), child));
                        }
                    }
                }
            }
        }
    }

    fn extract_var_name(
        &self,
        ast_context: &AstContext,
        node: Node,
        source: &[u8],
    ) -> Option<String> {
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
        // Check assignment: var = g_strdup(...)
        if node.kind() == "assignment_expression"
            && let Some(left) = node.child_by_field_name("left")
        {
            let left_text = ast_context.get_node_text(left, source);
            if left_text == var_name
                && let Some(right) = node.child_by_field_name("right")
                && self.is_autofree_allocation(ast_context, right, source)
            {
                return true;
            }
        }

        // Check init: char *var = g_strdup(...)
        if node.kind() == "init_declarator"
            && let Some(declarator) = node.child_by_field_name("declarator")
            && let Some(found_var) = self.extract_var_name(ast_context, declarator, source)
            && found_var == var_name
            && let Some(value) = node.child_by_field_name("value")
            && self.is_autofree_allocation(ast_context, value, source)
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

    fn is_autofree_allocation(&self, ast_context: &AstContext, node: Node, source: &[u8]) -> bool {
        if node.kind() == "call_expression"
            && let Some(function) = node.child_by_field_name("function")
        {
            let func_name = ast_context.get_node_text(function, source);

            // Functions that allocate memory suitable for g_autofree
            if func_name == "g_strdup"
                || func_name == "g_strndup"
                || func_name == "g_strdup_printf"
                || func_name == "g_strdup_vprintf"
                || func_name == "g_malloc"
                || func_name == "g_malloc0"
                || func_name == "g_realloc"
                || func_name == "g_try_malloc"
                || func_name == "g_try_malloc0"
                || func_name == "g_memdup"
            {
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
        let (is_cleanup, func_name) = ast_context.is_cleanup_call(node, source);
        if is_cleanup
            && func_name == "g_free"
            && let Some(arguments) = node.child_by_field_name("arguments")
        {
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
