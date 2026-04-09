use std::collections::HashMap;

use tree_sitter::Node;

use super::Rule;
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct SuggestGAutoptrGoto;

impl Rule for SuggestGAutoptrGoto {
    fn name(&self) -> &'static str {
        "suggest_g_autoptr_goto_cleanup"
    }

    fn description(&self) -> &'static str {
        "Suggest g_autoptr instead of goto error cleanup pattern"
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

impl SuggestGAutoptrGoto {
    fn check_function(
        &self,
        ast_context: &AstContext,
        node: Node,
        source: &[u8],
        file_path: &std::path::Path,
        base_line: usize,
        violations: &mut Vec<Violation>,
    ) {
        // Find the function body
        if let Some(body) = ast_context.find_body(node) {
            // Find all allocated variables (g_object_new, g_new, etc.)
            let allocated_vars = self.find_allocated_variables(ast_context, body, source);

            // Find all goto statements and the labels they target
            let goto_labels = self.find_goto_labels(ast_context, body, source);

            // Find cleanup labels (labels that unref/free variables)
            let cleanup_labels = self.find_cleanup_labels(ast_context, body, source);

            // Match: if allocated var has goto to cleanup label that frees it
            for (var_name, (var_type, decl_node)) in &allocated_vars {
                for goto_label in &goto_labels {
                    if let Some(cleanup_vars) = cleanup_labels.get(goto_label)
                        && cleanup_vars.contains(var_name)
                    {
                        // Extract base type name (strip pointer and qualifiers)
                        let base_type = self.extract_base_type(var_type);
                        let position = decl_node.start_position();
                        violations.push(self.violation(
                                file_path,
                                base_line + position.row,
                                position.column + 1,
                                format!(
                                    "Consider using g_autoptr({}) {} and g_steal_pointer to avoid goto cleanup",
                                    base_type, var_name
                                ),
                            ));
                    }
                }
            }
        }
    }

    fn extract_base_type(&self, type_text: &str) -> String {
        // Remove const, pointer, etc. to get base type
        // "const CoglOffscreen *" -> "CoglOffscreen"
        // "CoglDisplay *" -> "CoglDisplay"
        type_text
            .trim()
            .replace("const ", "")
            .replace("*", "")
            .trim()
            .to_string()
    }

    /// Find variables allocated with g_object_new, g_new, etc.
    /// Returns map of var_name -> (type_name, decl_node)
    fn find_allocated_variables<'a>(
        &self,
        ast_context: &AstContext,
        body: Node<'a>,
        source: &[u8],
    ) -> HashMap<String, (String, Node<'a>)> {
        let mut result = HashMap::new();

        // First pass: find all local pointer declarations
        let mut local_vars = HashMap::new();
        self.collect_local_pointer_declarations(ast_context, body, source, &mut local_vars);

        // Second pass: find assignments to those variables from allocation functions
        self.collect_allocated_vars(ast_context, body, source, &local_vars, &mut result);

        result
    }

    fn collect_local_pointer_declarations<'a>(
        &self,
        ast_context: &AstContext,
        node: Node<'a>,
        source: &[u8],
        result: &mut HashMap<String, (String, Node<'a>)>,
    ) {
        if node.kind() == "declaration" {
            // Look for: Type *var = NULL; or Type *var = some_function();
            // We collect all pointer declarations, will filter by allocation later
            if let Some(type_node) = node.child_by_field_name("type") {
                let type_text = ast_context.get_node_text(type_node, source);

                // Find all declarators in this declaration (could be multiple: Type *a = NULL,
                // *b = NULL;)
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if (child.kind() == "init_declarator" || child.kind() == "pointer_declarator")
                        && let Some(var_name) =
                            self.extract_var_name_from_declarator(ast_context, child, source)
                    {
                        // Only track simple identifiers, not field expressions
                        if !var_name.contains("->") && !var_name.contains(".") {
                            result.insert(var_name, (type_text.clone(), node));
                        }
                    }
                }
            }
        }

        // Recurse only one level (don't go into nested blocks)
        if node.kind() == "compound_statement" || node.kind() == "function_definition" {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "declaration" {
                    self.collect_local_pointer_declarations(ast_context, child, source, result);
                }
            }
        }
    }

    fn extract_var_name_from_declarator(
        &self,
        ast_context: &AstContext,
        node: Node,
        source: &[u8],
    ) -> Option<String> {
        match node.kind() {
            "init_declarator" => {
                if let Some(declarator) = node.child_by_field_name("declarator") {
                    return self.extract_var_name_from_declarator(ast_context, declarator, source);
                }
            }
            "pointer_declarator" => {
                if let Some(declarator) = node.child_by_field_name("declarator") {
                    return self.extract_var_name_from_declarator(ast_context, declarator, source);
                }
            }
            "identifier" => {
                return Some(ast_context.get_node_text(node, source));
            }
            _ => {}
        }
        None
    }

    fn collect_allocated_vars<'a>(
        &self,
        ast_context: &AstContext,
        node: Node<'a>,
        source: &[u8],
        local_vars: &HashMap<String, (String, Node<'a>)>,
        result: &mut HashMap<String, (String, Node<'a>)>,
    ) {
        // Look for assignments or initializations of local vars with allocation calls

        // Pattern 1: Type *var = allocation_call();
        if node.kind() == "declaration"
            && let Some(init_declarator) = self.find_init_declarator(node)
            && let Some(value) = init_declarator.child_by_field_name("value")
            && ast_context.is_allocation_call(value, source)
            && let Some(declarator) = init_declarator.child_by_field_name("declarator")
            && let Some(var_name) = self.extract_var_name(ast_context, declarator, source)
            && let Some((type_text, decl_node)) = local_vars.get(&var_name)
        {
            result.insert(var_name.clone(), (type_text.clone(), *decl_node));
        }

        // Pattern 2: var = allocation_call();
        if node.kind() == "assignment_expression"
            && let Some(left) = node.child_by_field_name("left")
        {
            let var_name = ast_context.get_node_text(left, source);
            // Only simple identifiers, not field expressions
            if !var_name.contains("->")
                && !var_name.contains(".")
                && let Some(right) = node.child_by_field_name("right")
                && ast_context.is_allocation_call(right, source)
                && let Some((type_text, decl_node)) = local_vars.get(&var_name)
            {
                result.insert(var_name.clone(), (type_text.clone(), *decl_node));
            }
        }

        // Recurse
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.collect_allocated_vars(ast_context, child, source, local_vars, result);
        }
    }

    fn find_init_declarator<'a>(&self, node: Node<'a>) -> Option<Node<'a>> {
        let mut cursor = node.walk();
        #[allow(clippy::manual_find)]
        for child in node.children(&mut cursor) {
            if child.kind() == "init_declarator" {
                return Some(child);
            }
        }
        None
    }

    fn extract_var_name(
        &self,
        ast_context: &AstContext,
        node: Node,
        source: &[u8],
    ) -> Option<String> {
        match node.kind() {
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

    /// Find all goto statements and collect the labels they target
    fn find_goto_labels(&self, ast_context: &AstContext, body: Node, source: &[u8]) -> Vec<String> {
        let mut labels = Vec::new();
        self.collect_goto_labels(ast_context, body, source, &mut labels);
        labels
    }

    fn collect_goto_labels(
        &self,
        ast_context: &AstContext,
        node: Node,
        source: &[u8],
        labels: &mut Vec<String>,
    ) {
        if node.kind() == "goto_statement" {
            // goto has a label child
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "statement_identifier" {
                    let label = ast_context.get_node_text(child, source);
                    labels.push(label);
                }
            }
        }

        // Recurse
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.collect_goto_labels(ast_context, child, source, labels);
        }
    }

    /// Find all labels and what variables they cleanup (unref/free)
    /// Returns map of label_name -> set of variable names
    fn find_cleanup_labels(
        &self,
        ast_context: &AstContext,
        body: Node,
        source: &[u8],
    ) -> HashMap<String, Vec<String>> {
        let mut result = HashMap::new();
        self.collect_cleanup_labels(ast_context, body, source, &mut result);
        result
    }

    fn collect_cleanup_labels(
        &self,
        ast_context: &AstContext,
        node: Node,
        source: &[u8],
        result: &mut HashMap<String, Vec<String>>,
    ) {
        if node.kind() == "labeled_statement"
            && let Some(label) = node.child_by_field_name("label")
        {
            let label_name = ast_context.get_node_text(label, source);

            // Find cleanup calls in the label body
            let mut cleanup_vars = Vec::new();
            self.find_cleanup_calls(ast_context, node, source, &mut cleanup_vars);

            if !cleanup_vars.is_empty() {
                result.insert(label_name, cleanup_vars);
            }
        }

        // Recurse
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.collect_cleanup_labels(ast_context, child, source, result);
        }
    }

    fn find_cleanup_calls(
        &self,
        ast_context: &AstContext,
        node: Node,
        source: &[u8],
        cleanup_vars: &mut Vec<String>,
    ) {
        let (is_cleanup, _) = ast_context.is_cleanup_call(node, source);
        if is_cleanup {
            // Get the argument
            if let Some(arguments) = node.child_by_field_name("arguments") {
                let mut cursor = arguments.walk();
                for child in arguments.children(&mut cursor) {
                    if child.kind() != "(" && child.kind() != ")" && child.kind() != "," {
                        let var_text = ast_context.get_node_text(child, source);
                        // For g_clear_object(&var), extract var from &var
                        let var_name = var_text.trim_start_matches('&');
                        cleanup_vars.push(var_name.to_string());
                    }
                }
            }
        }

        // Recurse
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.find_cleanup_calls(ast_context, child, source, cleanup_vars);
        }
    }
}
