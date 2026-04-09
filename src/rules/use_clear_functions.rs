use tree_sitter::Node;

use super::{Fix, Rule};
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct UseClearFunctions;

impl Rule for UseClearFunctions {
    fn name(&self) -> &'static str {
        "use_clear_functions"
    }

    fn description(&self) -> &'static str {
        "Suggest g_clear_object/g_clear_pointer instead of manual unref and NULL assignment"
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

                if let Some(func_source) = ast_context.get_function_source(path, func) {
                    if let Some(tree) = ast_context.parse_c_source(func_source) {
                        let base_byte = func.start_byte.unwrap_or(0);
                        self.check_node(
                            ast_context,
                            tree.root_node(),
                            func_source,
                            path,
                            func.line,
                            base_byte,
                            violations,
                        );
                    }
                }
            }
        }
    }
}

impl UseClearFunctions {
    fn is_manual_clear_pattern<'a>(
        &self,
        ast_context: &AstContext,
        node: Node<'a>,
        source: &[u8],
    ) -> Option<(String, String, String, Node<'a>)> {
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

        // Check if condition has && or || operators - if so, skip
        // g_clear_pointer only checks NULL, not other conditions
        if self.has_logical_operators(ast_context, condition, source) {
            return None;
        }

        let checked_var = self.find_variable_in_condition(ast_context, condition, source)?;

        // Get the consequence (the if body)
        let consequence = node.child_by_field_name("consequence")?;

        // Count ALL statements in the body (including declarations, loops, etc.)
        let statement_count = self.count_all_statements(consequence);

        // Look for unref/free call and NULL assignment as DIRECT children only
        let (has_unref_call, unref_function) =
            self.has_unref_call_direct(ast_context, consequence, &checked_var, source);
        let has_null_assignment =
            self.has_null_assignment_direct(ast_context, consequence, &checked_var, source);

        // Only suggest if there are EXACTLY 2 statements: the free and the NULL
        // assignment
        if statement_count == 2 && has_unref_call && has_null_assignment {
            let suggested_function = self.suggest_clear_function(&unref_function);
            return Some((
                checked_var,
                suggested_function.to_string(),
                unref_function,
                node,
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

    fn has_logical_operators(&self, ast_context: &AstContext, node: Node, source: &[u8]) -> bool {
        // Check if the condition contains && or || operators
        if node.kind() == "binary_expression" {
            if let Some(operator) = node.child_by_field_name("operator") {
                let op_text = ast_context.get_node_text(operator, source);
                if op_text == "&&" || op_text == "||" {
                    return true;
                }
            }
        }

        // Recursively check children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if self.has_logical_operators(ast_context, child, source) {
                return true;
            }
        }

        false
    }

    fn count_all_statements(&self, body: Node) -> usize {
        if body.kind() == "compound_statement" {
            let mut count = 0;
            let mut cursor = body.walk();
            for child in body.children(&mut cursor) {
                // Count all statement types: expression_statement, declaration,
                // for_statement, while_statement, if_statement, etc.
                if child.kind().ends_with("_statement") || child.kind() == "declaration" {
                    count += 1;
                }
            }
            count
        } else {
            // Single statement without braces
            1
        }
    }

    fn has_unref_call_direct(
        &self,
        ast_context: &AstContext,
        body: Node,
        var_name: &str,
        source: &[u8],
    ) -> (bool, String) {
        // Look for g_object_unref, g_free, etc. in DIRECT children only, not
        // recursively
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

        // Only check direct children
        if body.kind() == "compound_statement" {
            let mut cursor = body.walk();
            for child in body.children(&mut cursor) {
                if child.kind() == "expression_statement" {
                    if let Some(call) = ast_context.find_call_expression(child) {
                        if let Some(function) = call.child_by_field_name("function") {
                            let func_name = ast_context.get_node_text(function, source);

                            for &expected_func in &unref_functions {
                                if func_name == expected_func {
                                    if let Some(arguments) = call.child_by_field_name("arguments") {
                                        let args_text =
                                            ast_context.get_node_text(arguments, source);
                                        if args_text.contains(var_name) {
                                            return (true, func_name);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        (false, String::new())
    }

    fn has_null_assignment_direct(
        &self,
        ast_context: &AstContext,
        body: Node,
        var_name: &str,
        source: &[u8],
    ) -> bool {
        // Look for: var_name = NULL; in DIRECT children only
        if body.kind() == "compound_statement" {
            let mut cursor = body.walk();
            for child in body.children(&mut cursor) {
                if child.kind() == "expression_statement" {
                    // Look for assignment_expression directly in this child
                    let mut child_cursor = child.walk();
                    for grandchild in child.children(&mut child_cursor) {
                        if grandchild.kind() == "assignment_expression" {
                            if let Some(left) = grandchild.child_by_field_name("left") {
                                let left_text = ast_context.get_node_text(left, source);
                                if left_text == var_name {
                                    if let Some(right) = grandchild.child_by_field_name("right") {
                                        let right_full = ast_context.get_node_text(right, source);
                                        let right_text = right_full.trim();
                                        if right_text == "NULL"
                                            || right_text == "0"
                                            || right_text == "((void*)0)"
                                        {
                                            return true;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
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
        base_byte: usize,
        violations: &mut Vec<Violation>,
    ) {
        if let Some((var_name, suggested_function, unref_function, if_node)) =
            self.is_manual_clear_pattern(ast_context, node, source)
        {
            let position = if_node.start_position();

            // Build the correct replacement based on the function type
            let replacement = if suggested_function == "g_clear_object" {
                format!("g_clear_object (&{});", var_name)
            } else {
                // g_clear_pointer needs the free function as second arg
                format!("g_clear_pointer (&{}, {});", var_name, unref_function)
            };

            let fix = Fix {
                start_byte: base_byte + if_node.start_byte(),
                end_byte: base_byte + if_node.end_byte(),
                replacement: replacement.clone(),
            };

            violations.push(self.violation_with_fix(
                file_path,
                base_line + position.row,
                position.column + 1,
                format!(
                    "Use {} instead of manual NULL check, unref, and assignment",
                    replacement
                ),
                fix,
            ));
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.check_node(
                ast_context,
                child,
                source,
                file_path,
                base_line,
                base_byte,
                violations,
            );
        }
    }
}
