use tree_sitter::Node;

use super::Rule;
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct UseGObjectNewWithProperties;

impl Rule for UseGObjectNewWithProperties {
    fn name(&self) -> &'static str {
        "use_g_object_new_with_properties"
    }

    fn description(&self) -> &'static str {
        "Suggest setting properties in g_object_new instead of separate g_object_set calls"
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

impl UseGObjectNewWithProperties {
    fn check_node(
        &self,
        ast_context: &AstContext,
        node: Node,
        source: &[u8],
        file_path: &std::path::Path,
        base_line: usize,
        violations: &mut Vec<Violation>,
    ) {
        // Look for compound statements with the pattern
        if node.kind() == "compound_statement" {
            for (_var_name, set_count, new_node) in
                self.check_new_then_set_pattern(ast_context, node, source)
            {
                let position = new_node.start_position();
                violations.push(self.violation(
                    file_path,
                    base_line + position.row,
                    position.column + 1,
                    format!(
                        "Set properties in g_object_new() instead of {} separate g_object_set() call{}",
                        set_count,
                        if set_count > 1 { "s" } else { "" }
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

    /// Check for pattern: var = g_object_new(TYPE, NULL); followed by
    /// g_object_set(var, ...)
    fn check_new_then_set_pattern<'a>(
        &self,
        ast_context: &AstContext,
        compound: Node<'a>,
        source: &[u8],
    ) -> Vec<(String, usize, Node<'a>)> {
        let mut cursor = compound.walk();
        let children: Vec<_> = compound.children(&mut cursor).collect();

        let mut results = Vec::new();

        for i in 0..children.len() {
            let current = children[i];

            // Check if this is a g_object_new call with no properties
            if let Some((var_name, new_node)) =
                self.extract_g_object_new_empty(ast_context, current, source)
            {
                // Count how many consecutive g_object_set calls follow on the same variable
                let mut set_count = 0;

                for next in children.iter().skip(i + 1) {
                    if let Some(set_var) = self.extract_g_object_set(ast_context, *next, source) {
                        if set_var.trim() == var_name.trim() {
                            set_count += 1;
                            continue;
                        }
                    }

                    // Stop if we hit something that's not a g_object_set on our variable
                    break;
                }

                // Only report if there's at least one g_object_set call
                if set_count > 0 {
                    results.push((var_name, set_count, new_node));
                }
            }
        }

        results
    }

    /// Extract g_object_new call with no properties (just NULL or empty)
    /// Returns (variable_name, assignment_node)
    fn extract_g_object_new_empty<'a>(
        &self,
        ast_context: &AstContext,
        node: Node<'a>,
        source: &[u8],
    ) -> Option<(String, Node<'a>)> {
        // Look for assignment or declaration
        match node.kind() {
            "expression_statement" => {
                if let Some(assignment) = self.find_assignment(node) {
                    let left = assignment.child_by_field_name("left")?;
                    let right = assignment.child_by_field_name("right")?;

                    if let Some(call) = self.find_call_in_node(right) {
                        if self.is_g_object_new_empty(ast_context, call, source) {
                            let var_name =
                                ast_context.get_node_text(left, source).trim().to_string();
                            return Some((var_name, node));
                        }
                    }
                }
            }
            "declaration" => {
                if let Some(init_declarator) = self.find_init_declarator(node) {
                    let declarator = init_declarator.child_by_field_name("declarator")?;
                    let value = init_declarator.child_by_field_name("value")?;

                    if let Some(call) = self.find_call_in_node(value) {
                        if self.is_g_object_new_empty(ast_context, call, source) {
                            let var_name =
                                self.extract_declarator_name(ast_context, declarator, source)?;
                            return Some((var_name, node));
                        }
                    }
                }
            }
            _ => {}
        }

        None
    }

    /// Check if a call is g_object_new with no properties (just NULL or type
    /// only)
    fn is_g_object_new_empty(
        &self,
        ast_context: &AstContext,
        call_node: Node,
        source: &[u8],
    ) -> bool {
        let function = match call_node.child_by_field_name("function") {
            Some(f) => f,
            None => return false,
        };

        let func_name = ast_context.get_node_text(function, source);
        if func_name != "g_object_new" {
            return false;
        }

        // Get arguments
        let args = match call_node.child_by_field_name("arguments") {
            Some(a) => a,
            None => return false,
        };

        // Collect non-syntax arguments
        let mut cursor = args.walk();
        let mut arg_count = 0;
        let mut last_arg_is_null = false;

        for child in args.children(&mut cursor) {
            if child.kind() != "(" && child.kind() != ")" && child.kind() != "," {
                arg_count += 1;
                let arg_text = ast_context.get_node_text(child, source);
                last_arg_is_null = arg_text.trim() == "NULL";
            }
        }

        // g_object_new with just type and NULL, or just type
        // g_object_new(TYPE, NULL) - 2 args
        // g_object_new(TYPE) - 1 arg (rare but valid)
        if arg_count == 1 {
            return true;
        }

        if arg_count == 2 && last_arg_is_null {
            return true;
        }

        false
    }

    /// Extract g_object_set call, return the object variable
    fn extract_g_object_set(
        &self,
        ast_context: &AstContext,
        node: Node,
        source: &[u8],
    ) -> Option<String> {
        if let Some(call) = self.find_call_in_node(node) {
            let function = call.child_by_field_name("function")?;
            let func_name = ast_context.get_node_text(function, source);

            if func_name != "g_object_set" {
                return None;
            }

            // Get the first argument (the object)
            let args = call.child_by_field_name("arguments")?;
            let mut cursor = args.walk();
            for child in args.children(&mut cursor) {
                if child.kind() != "(" && child.kind() != ")" && child.kind() != "," {
                    return Some(ast_context.get_node_text(child, source).trim().to_string());
                }
            }
        }

        None
    }

    fn extract_declarator_name(
        &self,
        ast_context: &AstContext,
        declarator: Node,
        source: &[u8],
    ) -> Option<String> {
        match declarator.kind() {
            "identifier" => Some(
                ast_context
                    .get_node_text(declarator, source)
                    .trim()
                    .to_string(),
            ),
            "pointer_declarator" => {
                let inner = declarator.child_by_field_name("declarator")?;
                self.extract_declarator_name(ast_context, inner, source)
            }
            _ => None,
        }
    }

    #[allow(clippy::manual_find)]
    fn find_init_declarator<'a>(&self, node: Node<'a>) -> Option<Node<'a>> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "init_declarator" {
                return Some(child);
            }
        }
        None
    }

    fn find_call_in_node<'a>(&self, node: Node<'a>) -> Option<Node<'a>> {
        if node.kind() == "call_expression" {
            return Some(node);
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(call) = self.find_call_in_node(child) {
                return Some(call);
            }
        }

        None
    }

    fn find_assignment<'a>(&self, node: Node<'a>) -> Option<Node<'a>> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "assignment_expression" {
                return Some(child);
            }
            if let Some(assignment) = self.find_assignment(child) {
                return Some(assignment);
            }
        }
        None
    }
}
