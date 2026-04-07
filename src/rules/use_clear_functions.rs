use super::Violation;
use crate::ast_context::AstContext;
use crate::config::Config;
use tree_sitter::{Node, Parser};

pub struct UseClearFunctions;

impl UseClearFunctions {
    fn is_manual_clear_pattern(&self, node: Node, source: &[u8]) -> Option<Violation> {
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
        let checked_var = self.extract_checked_variable(condition, source)?;

        // Get the consequence (the if body)
        let consequence = node.child_by_field_name("consequence")?;

        // Look for unref/free call and NULL assignment in the body
        let (has_unref_call, unref_function) =
            self.has_unref_call(consequence, &checked_var, source);
        let has_null_assignment = self.has_null_assignment(consequence, &checked_var, source);

        if has_unref_call && has_null_assignment {
            let position = node.start_position();
            let suggested_function = self.suggest_clear_function(&unref_function);

            return Some(Violation {
                file: Default::default(), // Will be filled by caller
                line: position.row + 1,
                column: position.column + 1,
                message: format!(
                    "Use {} (&{}) instead of manual NULL check, unref, and assignment",
                    suggested_function, checked_var
                ),
                rule: "use_clear_functions",
                snippet: None,
            });
        }

        None
    }

    fn extract_checked_variable(&self, condition: Node, source: &[u8]) -> Option<String> {
        // Handle patterns:
        // - obj->field
        // - obj->field != NULL
        // - NULL != obj->field
        // - obj->field != 0

        // Look for field_expression or identifier
        self.find_variable_in_condition(condition, source)
    }

    fn find_variable_in_condition(&self, node: Node, source: &[u8]) -> Option<String> {
        // For field_expression (obj->field), return the full expression
        if node.kind() == "field_expression" {
            return Some(self.get_node_text(node, source));
        }

        // For identifier
        if node.kind() == "identifier" {
            return Some(self.get_node_text(node, source));
        }

        // For binary expressions (field != NULL), find the field
        if node.kind() == "binary_expression" {
            // Try both left and right sides
            if let Some(left) = node.child_by_field_name("left") {
                if let Some(var) = self.find_variable_in_condition(left, source) {
                    return Some(var);
                }
            }
            if let Some(right) = node.child_by_field_name("right") {
                if let Some(var) = self.find_variable_in_condition(right, source) {
                    return Some(var);
                }
            }
        }

        // For parenthesized expression, check inside
        if node.kind() == "parenthesized_expression" {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if let Some(var) = self.find_variable_in_condition(child, source) {
                    return Some(var);
                }
            }
        }

        None
    }

    fn has_unref_call(&self, body: Node, var_name: &str, source: &[u8]) -> (bool, String) {
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
            self.find_function_call_with_arg(body, &unref_functions, var_name, source)
        {
            return (true, function_name);
        }

        (false, String::new())
    }

    fn has_null_assignment(&self, body: Node, var_name: &str, source: &[u8]) -> bool {
        // Look for: var_name = NULL;
        self.find_null_assignment(body, var_name, source)
    }

    fn find_function_call_with_arg<'a>(
        &self,
        node: Node<'a>,
        function_names: &[&str],
        arg_name: &str,
        source: &[u8],
    ) -> Option<(String, Node<'a>)> {
        if node.kind() == "call_expression" {
            if let Some(function) = node.child_by_field_name("function") {
                let func_name = self.get_node_text(function, source);

                for &expected_func in function_names {
                    if func_name == expected_func {
                        // Check if argument matches
                        if let Some(arguments) = node.child_by_field_name("arguments") {
                            let args_text = self.get_node_text(arguments, source);
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
            if let Some(result) =
                self.find_function_call_with_arg(child, function_names, arg_name, source)
            {
                return Some(result);
            }
        }

        None
    }

    fn find_null_assignment(&self, node: Node, var_name: &str, source: &[u8]) -> bool {
        // Look for assignment: var_name = NULL;
        if node.kind() == "assignment_expression" {
            if let Some(left) = node.child_by_field_name("left") {
                let left_text = self.get_node_text(left, source);
                if left_text == var_name {
                    if let Some(right) = node.child_by_field_name("right") {
                        let right_full = self.get_node_text(right, source);
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
            if self.find_null_assignment(child, var_name, source) {
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

    fn get_node_text(&self, node: Node, source: &[u8]) -> String {
        let text = &source[node.byte_range()];
        std::str::from_utf8(text).unwrap_or("").to_string()
    }

    pub fn check_all(
        &self,
        ast_context: &AstContext,
        _config: &Config,
        violations: &mut Vec<Violation>,
    ) {
        let mut parser = Parser::new();
        parser.set_language(&tree_sitter_c::LANGUAGE.into()).ok();

        for (path, file) in ast_context.project.files.iter() {
            if path.extension().is_none_or(|ext| ext != "c") {
                continue;
            }

            for func in &file.functions {
                if !func.is_definition {
                    continue;
                }

                if let Some(func_source) = ast_context.get_function_source(path, func) {
                    if let Some(tree) = parser.parse(func_source, None) {
                        self.check_node(tree.root_node(), func_source, path, func.line, violations);
                    }
                }
            }
        }
    }

    fn check_node(
        &self,
        node: Node,
        source: &[u8],
        file_path: &std::path::Path,
        base_line: usize,
        violations: &mut Vec<Violation>,
    ) {
        if let Some(mut violation) = self.is_manual_clear_pattern(node, source) {
            violation.file = file_path.to_owned();
            violation.line = base_line + violation.line - 1;
            violations.push(violation);
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.check_node(child, source, file_path, base_line, violations);
        }
    }
}
