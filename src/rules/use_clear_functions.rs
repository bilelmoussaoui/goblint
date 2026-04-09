use tree_sitter::Node;

use super::Rule;
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct UseClearFunctions;

impl Rule for UseClearFunctions {
    fn name(&self) -> &'static str {
        "use_clear_functions"
    }

    fn description(&self) -> &'static str {
        "Suggest g_clear_object/g_clear_pointer instead of manual unref and NULL assignment"
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

impl UseClearFunctions {
    fn is_manual_clear_pattern(
        &self,
        ast_context: &AstContext,
        node: Node,
        source: &[u8],
    ) -> Option<Violation> {
        // Look for pattern:
        // if (obj->field) {
        //   g_object_unref (obj->field);
        //   obj->field = NULL;
        // }

        if node.kind() != "if_statement" {
            return None;
        }

        // Get the condition
        let condition = node.child_by_field_name("condition")?;
        let checked_var = self.find_variable_in_condition(ast_context, condition, source)?;

        // Get the consequence (the if body)
        let consequence = node.child_by_field_name("consequence")?;

        // Look for unref/free call and NULL assignment in the body
        let (has_unref_call, unref_function) =
            self.has_unref_call(ast_context, consequence, &checked_var, source);
        let has_null_assignment =
            self.has_null_assignment(ast_context, consequence, &checked_var, source);

        if has_unref_call && has_null_assignment {
            let position = node.start_position();
            let suggested_function = self.suggest_clear_function(&unref_function);

            return Some(self.violation(
                &std::path::PathBuf::default(),
                position.row + 1,
                position.column + 1,
                format!(
                    "Use {} (&{}) instead of manual NULL check, unref, and assignment",
                    suggested_function, checked_var
                ),
            ));
        }

        None
    }

    fn find_variable_in_condition(
        &self,
        ast_context: &AstContext,
        node: Node,
        source: &[u8],
    ) -> Option<String> {
        // For field_expression (obj->field), return the full expression
        if node.kind() == "field_expression" {
            return Some(ast_context.get_node_text(node, source));
        }

        // For identifier
        if node.kind() == "identifier" {
            return Some(ast_context.get_node_text(node, source));
        }

        // For binary expressions (field != NULL), find the field
        if node.kind() == "binary_expression" {
            // Try both left and right sides
            if let Some(left) = node.child_by_field_name("left") {
                if let Some(var) = self.find_variable_in_condition(ast_context, left, source) {
                    return Some(var);
                }
            }
            if let Some(right) = node.child_by_field_name("right") {
                if let Some(var) = self.find_variable_in_condition(ast_context, right, source) {
                    return Some(var);
                }
            }
        }

        // For parenthesized expression, check inside
        if node.kind() == "parenthesized_expression" {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if let Some(var) = self.find_variable_in_condition(ast_context, child, source) {
                    return Some(var);
                }
            }
        }

        None
    }

    fn has_unref_call(
        &self,
        ast_context: &AstContext,
        body: Node,
        var_name: &str,
        source: &[u8],
    ) -> (bool, String) {
        // Look for g_object_unref, g_free, g_hash_table_unref, etc.
        let unref_functions = [
            "g_object_unref",
            "g_free",
            "g_hash_table_unref",
            "g_hash_table_destroy",
            "g_list_free",
            "g_slist_free",
            "g_array_unref",
            "g_bytes_unref",
            "g_variant_unref",
        ];

        if let Some((function_name, _)) =
            self.find_function_call_with_arg(ast_context, body, &unref_functions, var_name, source)
        {
            return (true, function_name);
        }

        (false, String::new())
    }

    fn has_null_assignment(
        &self,
        ast_context: &AstContext,
        body: Node,
        var_name: &str,
        source: &[u8],
    ) -> bool {
        // Look for: var_name = NULL;
        self.find_null_assignment(ast_context, body, var_name, source)
    }

    fn find_function_call_with_arg<'a>(
        &self,
        ast_context: &AstContext,
        node: Node<'a>,
        function_names: &[&str],
        arg_name: &str,
        source: &[u8],
    ) -> Option<(String, Node<'a>)> {
        if node.kind() == "call_expression" {
            if let Some(function) = node.child_by_field_name("function") {
                let func_name = ast_context.get_node_text(function, source);

                for &expected_func in function_names {
                    if func_name == expected_func {
                        // Check if argument matches
                        if let Some(arguments) = node.child_by_field_name("arguments") {
                            let args_text = ast_context.get_node_text(arguments, source);
                            if args_text.contains(arg_name) {
                                return Some((func_name, node));
                            }
                        }
                    }
                }
            }
        }

        // Recursively check children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(result) = self.find_function_call_with_arg(
                ast_context,
                child,
                function_names,
                arg_name,
                source,
            ) {
                return Some(result);
            }
        }

        None
    }

    fn find_null_assignment(
        &self,
        ast_context: &AstContext,
        node: Node,
        var_name: &str,
        source: &[u8],
    ) -> bool {
        // Look for assignment: var_name = NULL;
        if node.kind() == "assignment_expression" {
            if let Some(left) = node.child_by_field_name("left") {
                let left_text = ast_context.get_node_text(left, source);
                if left_text == var_name {
                    if let Some(right) = node.child_by_field_name("right") {
                        let right_full = ast_context.get_node_text(right, source);
                        let right_text = right_full.trim();
                        if right_text == "NULL" || right_text == "0" || right_text == "((void*)0)" {
                            return true;
                        }
                    }
                }
            }
        }

        // Recursively check children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if self.find_null_assignment(ast_context, child, var_name, source) {
                return true;
            }
        }

        false
    }

    fn suggest_clear_function(&self, unref_function: &str) -> &str {
        match unref_function {
            "g_object_unref" => "g_clear_object",
            "g_free" => "g_clear_pointer",
            "g_hash_table_unref" | "g_hash_table_destroy" => "g_clear_pointer",
            "g_list_free" | "g_slist_free" => "g_clear_pointer",
            "g_array_unref" => "g_clear_pointer",
            "g_bytes_unref" => "g_clear_pointer",
            "g_variant_unref" => "g_clear_pointer",
            _ => "g_clear_pointer",
        }
    }

    fn check_node(
        &self,
        ast_context: &AstContext,
        node: Node,
        source: &[u8],
        file_path: &std::path::Path,
        base_line: usize,
        violations: &mut Vec<Violation>,
    ) {
        if let Some(mut violation) = self.is_manual_clear_pattern(ast_context, node, source) {
            violation.file = file_path.to_owned();
            violation.line = base_line + violation.line - 1;
            violations.push(violation);
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.check_node(ast_context, child, source, file_path, base_line, violations);
        }
    }
}
