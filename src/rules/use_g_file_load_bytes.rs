use std::collections::HashMap;

use tree_sitter::Node;

use super::{CheckContext, Rule};
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct UseGFileLoadBytes;

impl Rule for UseGFileLoadBytes {
    fn name(&self) -> &'static str {
        "use_g_file_load_bytes"
    }

    fn description(&self) -> &'static str {
        "Suggest g_file_load_bytes/g_file_load_bytes_async instead of g_file_load_contents + g_bytes_new_take"
    }

    fn category(&self) -> super::Category {
        super::Category::Complexity
    }

    fn fixable(&self) -> bool {
        false // Complex pattern, needs manual review
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
                    self.check_function(ast_context, tree.root_node(), &ctx, violations);
                }
            }
        }
    }
}

impl UseGFileLoadBytes {
    fn check_function(
        &self,
        ast_context: &AstContext,
        node: Node,
        ctx: &CheckContext,
        violations: &mut Vec<Violation>,
    ) {
        // Find all g_file_load_contents calls and track their output variables
        let load_contents_calls = self.find_load_contents_calls(ast_context, node, ctx.source);

        // Find all g_bytes_new_take calls
        self.find_bytes_new_take_violations(
            ast_context,
            node,
            ctx,
            &load_contents_calls,
            violations,
        );
    }

    /// Find all g_file_load_contents calls and return (contents_var,
    /// length_var, file_arg, cancellable_arg, error_arg)
    fn find_load_contents_calls<'a>(
        &self,
        ast_context: &AstContext,
        node: Node,
        source: &'a [u8],
    ) -> HashMap<&'a str, (&'a str, &'a str, &'a str, &'a str)> {
        let mut result = HashMap::new();
        self.collect_load_contents_calls(ast_context, node, source, &mut result);
        result
    }

    fn collect_load_contents_calls<'a>(
        &self,
        ast_context: &AstContext,
        node: Node,
        source: &'a [u8],
        result: &mut HashMap<&'a str, (&'a str, &'a str, &'a str, &'a str)>,
    ) {
        if node.kind() == "call_expression"
            && let Some(function) = node.child_by_field_name("function")
        {
            let func_name = ast_context.get_node_text(function, source);
            if (func_name == "g_file_load_contents" || func_name == "g_file_load_contents_finish")
                && let Some(args) = node.child_by_field_name("arguments")
            {
                let arguments = self.collect_arguments(ast_context, args, source);

                // g_file_load_contents(file, cancellable, &contents, &length, &etag, &error)
                //                      0     1            2          3         4       5
                // g_file_load_contents_finish(file, res, &contents, &length, &etag, &error)
                //                             0     1    2          3         4       5
                if arguments.len() >= 6 {
                    let contents_var = arguments[2].trim_start_matches('&');
                    let length_var = arguments[3].trim_start_matches('&');

                    let (file_or_res_arg, cancellable_or_res_arg) =
                        if func_name == "g_file_load_contents" {
                            (arguments[0], arguments[1])
                        } else {
                            // For _finish, use file (arg 0) and "NULL" for cancellable (not
                            // used in async version)
                            (arguments[0], "NULL")
                        };

                    result.insert(
                        contents_var,
                        (
                            length_var,
                            file_or_res_arg,
                            cancellable_or_res_arg,
                            arguments[5],
                        ),
                    );
                }
            }
        }

        // Recurse
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.collect_load_contents_calls(ast_context, child, source, result);
        }
    }

    fn find_bytes_new_take_violations<'a>(
        &self,
        ast_context: &AstContext,
        node: Node,
        ctx: &CheckContext,
        load_contents_calls: &HashMap<&'a str, (&'a str, &'a str, &'a str, &'a str)>,
        violations: &mut Vec<Violation>,
    ) {
        if node.kind() == "call_expression"
            && let Some(function) = node.child_by_field_name("function")
        {
            let func_name = ast_context.get_node_text(function, ctx.source);
            if func_name == "g_bytes_new_take"
                && let Some(args) = node.child_by_field_name("arguments")
            {
                let arguments = self.collect_arguments(ast_context, args, ctx.source);

                // g_bytes_new_take(contents, length) or
                // g_bytes_new_take(g_steal_pointer(&contents), length)
                if arguments.len() >= 2 {
                    let first_arg = arguments[0];

                    // Extract variable name from first arg (handle g_steal_pointer)
                    let contents_var = if first_arg.contains("g_steal_pointer") {
                        first_arg
                            .trim_start_matches("g_steal_pointer")
                            .trim()
                            .trim_start_matches('(')
                            .trim_end_matches(')')
                            .trim()
                            .trim_start_matches('&')
                    } else {
                        first_arg
                    };

                    // Check if this contents variable came from g_file_load_contents
                    if let Some((_length_var, _file_arg, _cancellable_arg, _error_arg)) =
                        load_contents_calls.get(&contents_var)
                    {
                        violations.push(self.violation(
                                ctx.file_path,
                                ctx.base_line + node.start_position().row,
                                node.start_position().column + 1,
                                "Consider using g_file_load_bytes/g_file_load_bytes_async instead of g_file_load_contents + g_bytes_new_take for simplicity".to_string(),
                            ));
                    }
                }
            }
        }

        // Recurse
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.find_bytes_new_take_violations(
                ast_context,
                child,
                ctx,
                load_contents_calls,
                violations,
            );
        }
    }

    fn collect_arguments<'a>(
        &self,
        ast_context: &AstContext,
        args_node: Node,
        source: &'a [u8],
    ) -> Vec<&'a str> {
        let mut cursor = args_node.walk();
        let mut arguments = Vec::new();
        for child in args_node.children(&mut cursor) {
            if child.kind() != "(" && child.kind() != ")" && child.kind() != "," {
                arguments.push(ast_context.get_node_text(child, source));
            }
        }
        arguments
    }
}
