use tree_sitter::Node;

use super::{Fix, Rule};
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct SuggestGSourceOnce;

impl Rule for SuggestGSourceOnce {
    fn name(&self) -> &'static str {
        "suggest_g_source_once"
    }

    fn description(&self) -> &'static str {
        "Suggest using g_idle_add_once/g_timeout_add_once when callback always returns G_SOURCE_REMOVE"
    }

    fn category(&self) -> super::Category {
        super::Category::Style
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
                    let root = tree.root_node();

                    if let Some(body) = ast_context.find_body(root) {
                        let ctx = super::CheckContext {
                            source: func_source,
                            file_path: path,
                            base_line: func.line,
                            base_byte: func.start_byte.unwrap(),
                        };
                        self.check_source_add_calls(ast_context, body, &ctx, violations);
                    }
                }
            }
        }
    }
}

impl SuggestGSourceOnce {
    fn check_source_add_calls(
        &self,
        ast_context: &AstContext,
        node: Node,
        ctx: &super::CheckContext,
        violations: &mut Vec<Violation>,
    ) {
        // Look for g_idle_add or g_timeout_add calls
        if node.kind() == "call_expression"
            && let Some(function) = node.child_by_field_name("function")
        {
            let func_text = ast_context.get_node_text(function, ctx.source);

            if func_text == "g_idle_add" || func_text == "g_timeout_add" {
                // Get the first argument (the callback function)
                if let Some(arguments) = node.child_by_field_name("arguments")
                    && let Some(first_arg) = self.get_first_argument(arguments)
                {
                    let callback_name = ast_context.get_node_text(first_arg, ctx.source);

                    // Only proceed if callback is NOT used elsewhere
                    if !self.is_callback_used_elsewhere(ast_context, &callback_name, ctx.file_path)
                    {
                        // Find the callback function definition (only in the same file)
                        if let Some(mut callback_fixes) =
                            self.get_callback_fixes(ast_context, &callback_name, ctx.file_path)
                        {
                            let position = node.start_position();
                            let replacement = if func_text == "g_idle_add" {
                                "g_idle_add_once"
                            } else {
                                "g_timeout_add_once"
                            };

                            // Fix 1: Replace g_idle_add → g_idle_add_once
                            let mut fixes = vec![Fix::from_node(function, ctx, replacement)];

                            // Add callback fixes (return type + return statements)
                            fixes.append(&mut callback_fixes);

                            violations.push(self.violation_with_fixes(
                                ctx.file_path,
                                ctx.base_line + position.row,
                                position.column + 1,
                                format!(
                                    "Callback '{}' always returns G_SOURCE_REMOVE. Use {} instead of {}",
                                    callback_name, replacement, func_text
                                ),
                                fixes,
                            ));
                        }
                    }
                }
            }
        }

        // Recursively check children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.check_source_add_calls(ast_context, child, ctx, violations);
        }
    }

    fn get_first_argument<'a>(&self, arguments_node: Node<'a>) -> Option<Node<'a>> {
        let mut cursor = arguments_node.walk();
        arguments_node
            .children(&mut cursor)
            .find(|&child| child.kind() != "(" && child.kind() != ")" && child.kind() != ",")
    }

    fn get_callback_fixes(
        &self,
        ast_context: &AstContext,
        callback_name: &str,
        target_file: &std::path::Path,
    ) -> Option<Vec<Fix>> {
        let mut fixes = Vec::new();
        let mut found_definition = false;

        // Find the function definition and all declarations in the same file
        for (path, file) in ast_context.iter_all_files() {
            // Only process callbacks in the same file
            if path != target_file {
                continue;
            }

            for func in &file.functions {
                if func.name != callback_name {
                    continue;
                }

                if func.is_definition {
                    let Some(func_source) = ast_context.get_function_source(path, func) else {
                        continue;
                    };
                    let Some(tree) = ast_context.parse_c_source(func_source) else {
                        continue;
                    };
                    let root = tree.root_node();
                    let func_start_byte = func.start_byte.unwrap();
                    // Check if all returns are FALSE/G_SOURCE_REMOVE
                    if let Some(body) = ast_context.find_body(root) {
                        let returns = self.collect_all_returns(body, func_source, ast_context);

                        // Must have at least one return statement
                        if returns.is_empty() {
                            return None;
                        }

                        // All returns must be FALSE or G_SOURCE_REMOVE
                        if !returns
                            .iter()
                            .all(|r| r == "FALSE" || r == "G_SOURCE_REMOVE" || r == "0")
                        {
                            return None;
                        }

                        // Fix: Change return type from gboolean to void in definition
                        if let Some(return_type_node) =
                            self.find_return_type(root, func_source, ast_context)
                        {
                            fixes.push(Fix::new(
                                func_start_byte + return_type_node.start_byte(),
                                func_start_byte + return_type_node.end_byte(),
                                "void",
                            ));
                        }

                        // Fix: Remove all return statements (entire lines)
                        let return_statements = self.collect_all_return_statements(body);
                        for return_stmt in return_statements {
                            let (line_start, line_end) =
                                self.find_line_bounds(return_stmt, func_source);
                            fixes.push(Fix::new(
                                func_start_byte + line_start,
                                func_start_byte + line_end,
                                "",
                            ));
                        }

                        found_definition = true;
                    }
                } else {
                    // This is a declaration - fix by searching the line in the file
                    if let Some(fix) = self.fix_declaration_return_type(path, func, ast_context) {
                        fixes.push(fix);
                    }
                }
            }
        }

        if found_definition && !fixes.is_empty() {
            Some(fixes)
        } else {
            None
        }
    }

    fn collect_all_returns(
        &self,
        node: Node,
        source: &[u8],
        ast_context: &AstContext,
    ) -> Vec<String> {
        let mut returns = Vec::new();

        if node.kind() == "return_statement" {
            // Get the return value
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() != "return" && child.kind() != ";" {
                    let return_value = ast_context.get_node_text(child, source);
                    returns.push(return_value.trim().to_string());
                }
            }
        }

        // Recursively check children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            returns.extend(self.collect_all_returns(child, source, ast_context));
        }

        returns
    }

    fn collect_all_return_statements<'a>(&self, node: Node<'a>) -> Vec<Node<'a>> {
        let mut statements = Vec::new();

        if node.kind() == "return_statement" {
            statements.push(node);
        }

        // Recursively check children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            statements.extend(self.collect_all_return_statements(child));
        }

        statements
    }

    fn find_return_type<'a>(
        &self,
        node: Node<'a>,
        source: &[u8],
        ast_context: &AstContext,
    ) -> Option<Node<'a>> {
        // Root might be a translation_unit, find function_definition first
        let func_def = if node.kind() == "function_definition" {
            node
        } else {
            let mut cursor = node.walk();
            node.children(&mut cursor)
                .find(|c| c.kind() == "function_definition")?
        };

        // Now recursively search for primitive_type or type_identifier with "gboolean"
        self.find_gboolean_type(func_def, source, ast_context)
    }

    fn find_gboolean_type<'a>(
        &self,
        node: Node<'a>,
        source: &[u8],
        ast_context: &AstContext,
    ) -> Option<Node<'a>> {
        // Check if this node itself is a gboolean type
        if node.kind() == "primitive_type" || node.kind() == "type_identifier" {
            let text = ast_context.get_node_text(node, source);
            if text == "gboolean" {
                return Some(node);
            }
        }

        // Recursively search children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            // Don't search inside the function body
            if child.kind() == "compound_statement" {
                continue;
            }
            if let Some(result) = self.find_gboolean_type(child, source, ast_context) {
                return Some(result);
            }
        }

        None
    }

    fn fix_declaration_return_type(
        &self,
        file_path: &std::path::Path,
        func: &gobject_ast::FunctionInfo,
        ast_context: &AstContext,
    ) -> Option<Fix> {
        // Get the file source
        let file = ast_context.project.files.get(file_path)?;
        let source = &file.source;

        // Find the line where the declaration is
        let mut line_start = 0;
        let mut current_line = 1;

        for (i, &byte) in source.iter().enumerate() {
            if current_line == func.line {
                // Found the line, now find "gboolean" on this line
                let mut line_end = i;
                while line_end < source.len() && source[line_end] != b'\n' {
                    line_end += 1;
                }

                let line_bytes = &source[line_start..line_end];
                let line_str = String::from_utf8_lossy(line_bytes);

                // Search for "gboolean" in the line
                if let Some(offset) = line_str.find("gboolean") {
                    let gboolean_start = line_start + offset;
                    let gboolean_end = gboolean_start + "gboolean".len();

                    // Preserve alignment by padding "void" to match "gboolean" length
                    let replacement = format!("{:8}", "void"); // "gboolean" is 8 chars

                    return Some(Fix::new(gboolean_start, gboolean_end, replacement));
                }

                return None;
            }

            if byte == b'\n' {
                current_line += 1;
                line_start = i + 1;
            }
        }

        None
    }

    fn is_callback_used_elsewhere(
        &self,
        ast_context: &AstContext,
        callback_name: &str,
        file_path: &std::path::Path,
    ) -> bool {
        // Search the file for all uses of this callback name
        for (path, file) in ast_context.iter_c_files() {
            if path != file_path {
                continue;
            }

            for func in &file.functions {
                if !func.is_definition {
                    continue;
                }

                if let Some(func_source) = ast_context.get_function_source(path, func)
                    && let Some(tree) = ast_context.parse_c_source(func_source)
                {
                    let root = tree.root_node();
                    if let Some(body) = ast_context.find_body(root)
                        && self.has_non_source_add_usage(
                            ast_context,
                            body,
                            callback_name,
                            func_source,
                        )
                    {
                        return true;
                    }
                }
            }
        }

        false
    }

    fn has_non_source_add_usage(
        &self,
        ast_context: &AstContext,
        node: Node,
        callback_name: &str,
        source: &[u8],
    ) -> bool {
        // Check if this is an identifier matching the callback name
        if node.kind() == "identifier" {
            let text = ast_context.get_node_text(node, source);
            if text == callback_name {
                // Check if this usage is inside a g_idle_add or g_timeout_add call
                if !self.is_inside_source_add_call(node, source, ast_context) {
                    return true;
                }
            }
        }

        // Recursively check children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if self.has_non_source_add_usage(ast_context, child, callback_name, source) {
                return true;
            }
        }

        false
    }

    fn is_inside_source_add_call(
        &self,
        node: Node,
        source: &[u8],
        ast_context: &AstContext,
    ) -> bool {
        let mut current = node;
        while let Some(parent) = current.parent() {
            if parent.kind() == "call_expression"
                && let Some(function) = parent.child_by_field_name("function")
            {
                let func_text = ast_context.get_node_text(function, source);
                if func_text == "g_idle_add" || func_text == "g_timeout_add" {
                    return true;
                }
            }
            current = parent;
        }
        false
    }

    fn find_line_bounds(&self, node: Node, source: &[u8]) -> (usize, usize) {
        // Find the start of the line
        let mut line_start = node.start_byte();
        while line_start > 0 && source[line_start - 1] != b'\n' {
            line_start -= 1;
        }

        // Check if the previous line is empty (only whitespace)
        if line_start > 0 {
            let mut prev_line_start = line_start - 1; // Skip the '\n'
            while prev_line_start > 0 && source[prev_line_start - 1] != b'\n' {
                prev_line_start -= 1;
            }

            // Check if the line is only whitespace
            let prev_line = &source[prev_line_start..line_start - 1];
            if prev_line.iter().all(|&b| b == b' ' || b == b'\t') {
                line_start = prev_line_start;
            }
        }

        // Find the end of the line (including newline)
        let mut line_end = node.end_byte();
        while line_end < source.len() && source[line_end] != b'\n' {
            line_end += 1;
        }
        // Include the newline character
        if line_end < source.len() && source[line_end] == b'\n' {
            line_end += 1;
        }

        (line_start, line_end)
    }
}
