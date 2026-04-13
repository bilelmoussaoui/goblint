use tree_sitter::Node;

use super::{CheckContext, Fix, Rule};
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct UseGStealPointer;

impl Rule for UseGStealPointer {
    fn name(&self) -> &'static str {
        "use_g_steal_pointer"
    }

    fn description(&self) -> &'static str {
        "Use g_steal_pointer() instead of manually copying a pointer and setting it to NULL"
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
                    let ctx = CheckContext {
                        source: func_source,
                        file_path: path,
                        base_line: func.line,
                        base_byte: func.start_byte.unwrap_or(0),
                    };
                    self.check_node(ast_context, tree.root_node(), &ctx, violations);
                }
            }
        }
    }
}

impl UseGStealPointer {
    fn check_node(
        &self,
        ast_context: &AstContext,
        node: Node,
        ctx: &CheckContext,
        violations: &mut Vec<Violation>,
    ) {
        if node.kind() == "compound_statement" {
            self.check_compound(ast_context, node, ctx, violations);
            return;
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.check_node(ast_context, child, ctx, violations);
        }
    }

    fn check_compound(
        &self,
        ast_context: &AstContext,
        compound: Node,
        ctx: &CheckContext,
        violations: &mut Vec<Violation>,
    ) {
        let mut cursor = compound.walk();
        let stmts: Vec<Node> = compound
            .children(&mut cursor)
            .filter(|n| n.kind() != "{" && n.kind() != "}" && n.kind() != "comment")
            .collect();

        let mut i = 0;
        while i < stmts.len() {
            // Try if/else steal: if (expr) { dest = expr; expr = NULL; } else { dest =
            // NULL; }
            if self.try_if_else_steal(ast_context, stmts[i], ctx, violations) {
                i += 1;
                continue;
            }

            // Try if-without-else steal with brace removal:
            //   if (c) { dest = ptr; ptr = NULL; }
            //   if (c) { T *tmp = ptr; ptr = NULL; return tmp; }
            if self.try_if_no_else_steal(ast_context, stmts[i], ctx, violations) {
                i += 1;
                continue;
            }

            // Try 3-statement pattern: T *tmp = ptr; ptr = NULL; return tmp;
            if i + 2 < stmts.len()
                && self.try_declare_null_return(
                    ast_context,
                    stmts[i],
                    stmts[i + 1],
                    stmts[i + 2],
                    ctx,
                    violations,
                )
            {
                i += 3;
                continue;
            }

            // Try 2-statement pattern: other = ptr; ptr = NULL;
            if i + 1 < stmts.len()
                && self.try_assign_null(ast_context, stmts[i], stmts[i + 1], ctx, violations)
            {
                i += 2;
                continue;
            }

            // Recurse into nested blocks
            self.check_node(ast_context, stmts[i], ctx, violations);
            i += 1;
        }
    }

    /// Matches: `T *tmp = ptr_expr; ptr_expr = NULL; return tmp;`
    fn try_declare_null_return(
        &self,
        ast_context: &AstContext,
        s1: Node,
        s2: Node,
        s3: Node,
        ctx: &CheckContext,
        violations: &mut Vec<Violation>,
    ) -> bool {
        if s1.kind() != "declaration" {
            return false;
        }

        let Some((tmp_name, ptr_expr)) = self.extract_decl_init(ast_context, s1, ctx.source) else {
            return false;
        };

        if !self.is_null_assign(ast_context, s2, ptr_expr, ctx.source) {
            return false;
        }

        if s3.kind() != "return_statement" {
            return false;
        }

        let mut cursor = s3.walk();
        let ret_val = s3
            .children(&mut cursor)
            .find(|n| n.kind() != "return" && n.kind() != ";");
        let Some(ret_val) = ret_val else {
            return false;
        };
        if ast_context.get_node_text(ret_val, ctx.source) != tmp_name {
            return false;
        }

        let replacement = format!("return g_steal_pointer (&{ptr_expr});");
        let fix = Fix::from_range(s1.start_byte(), s3.end_byte(), ctx, &replacement);
        violations.push(self.violation_with_fix(
            ctx.file_path,
            ctx.base_line + s1.start_position().row,
            s1.start_position().column + 1,
            format!("Use {replacement} instead of copying {ptr_expr} and setting it to NULL"),
            fix,
        ));
        true
    }

    /// Matches: `other_expr = ptr_expr; ptr_expr = NULL;`
    fn try_assign_null(
        &self,
        ast_context: &AstContext,
        s1: Node,
        s2: Node,
        ctx: &CheckContext,
        violations: &mut Vec<Violation>,
    ) -> bool {
        let Some((other_expr, ptr_expr)) = self.extract_assignment(ast_context, s1, ctx.source)
        else {
            return false;
        };

        if self.is_null_text(ptr_expr) {
            return false;
        }

        // Skip dereference expressions — g_steal_pointer (&*expr) is confusing
        if ptr_expr.starts_with('*') {
            return false;
        }

        if !self.is_null_assign(ast_context, s2, ptr_expr, ctx.source) {
            return false;
        }

        let replacement = format!("{other_expr} = g_steal_pointer (&{ptr_expr});");
        let fix = Fix::from_range(s1.start_byte(), s2.end_byte(), ctx, &replacement);
        violations.push(self.violation_with_fix(
            ctx.file_path,
            ctx.base_line + s1.start_position().row,
            s1.start_position().column + 1,
            format!("Use g_steal_pointer (&{ptr_expr}) instead of copying and setting to NULL"),
            fix,
        ));
        true
    }

    /// Matches:
    /// ```c
    /// if (expr) { dest = expr; expr = NULL; } else { dest = NULL; }
    /// ```
    /// Condition may be bare `expr` or `expr != NULL` / `NULL != expr`.
    fn try_if_else_steal(
        &self,
        ast_context: &AstContext,
        if_node: Node,
        ctx: &CheckContext,
        violations: &mut Vec<Violation>,
    ) -> bool {
        if if_node.kind() != "if_statement" {
            return false;
        }

        // Extract the tested expression from the condition
        let Some(condition) = if_node.child_by_field_name("condition") else {
            return false;
        };
        let Some(expr_text) = self.extract_condition_expr(ast_context, condition, ctx.source)
        else {
            return false;
        };

        // Skip dereference expressions
        if expr_text.starts_with('*') {
            return false;
        }

        // Then-block: must be compound_statement with exactly 2 stmts
        let Some(consequence) = if_node.child_by_field_name("consequence") else {
            return false;
        };
        if consequence.kind() != "compound_statement" {
            return false;
        }
        let mut cursor = consequence.walk();
        let then_stmts: Vec<Node> = consequence
            .children(&mut cursor)
            .filter(|n| n.kind() != "{" && n.kind() != "}" && n.kind() != "comment")
            .collect();
        if then_stmts.len() != 2 {
            return false;
        }

        // then_stmts[0]: dest = expr
        let Some((dest_expr, rhs)) =
            self.extract_assignment(ast_context, then_stmts[0], ctx.source)
        else {
            return false;
        };
        if rhs != expr_text {
            return false;
        }

        // then_stmts[1]: expr = NULL
        if !self.is_null_assign(ast_context, then_stmts[1], expr_text, ctx.source) {
            return false;
        }

        // Else-block: must exist and contain exactly 1 stmt: dest = NULL
        let Some(alternative) = if_node.child_by_field_name("alternative") else {
            return false;
        };
        let mut cursor = alternative.walk();
        let else_body = alternative
            .children(&mut cursor)
            .find(|n| n.kind() == "compound_statement");
        let Some(else_body) = else_body else {
            return false;
        };
        let mut cursor = else_body.walk();
        let else_stmts: Vec<Node> = else_body
            .children(&mut cursor)
            .filter(|n| n.kind() != "{" && n.kind() != "}" && n.kind() != "comment")
            .collect();
        if else_stmts.len() != 1 {
            return false;
        }
        if !self.is_null_assign(ast_context, else_stmts[0], dest_expr, ctx.source) {
            return false;
        }

        let replacement = format!("{dest_expr} = g_steal_pointer (&{expr_text});");
        let fix = Fix::from_range(if_node.start_byte(), if_node.end_byte(), ctx, &replacement);
        violations.push(self.violation_with_fix(
            ctx.file_path,
            ctx.base_line + if_node.start_position().row,
            if_node.start_position().column + 1,
            format!("Use g_steal_pointer (&{expr_text}) instead of if/else copy-and-NULL pattern"),
            fix,
        ));
        true
    }

    /// Matches an if-without-else whose braced body contains a steal pattern,
    /// and removes the braces in the fix:
    ///   `if (c) { dest = ptr; ptr = NULL; }`      → `if (c)\n  dest =
    /// g_steal_pointer(&ptr);`   `if (c) { T *t = ptr; ptr = NULL; return
    /// t; }` → `if (c)\n  return g_steal_pointer(&ptr);`
    fn try_if_no_else_steal(
        &self,
        ast_context: &AstContext,
        if_node: Node,
        ctx: &CheckContext,
        violations: &mut Vec<Violation>,
    ) -> bool {
        if if_node.kind() != "if_statement" {
            return false;
        }
        // Must have no else
        if if_node.child_by_field_name("alternative").is_some() {
            return false;
        }
        let Some(consequence) = if_node.child_by_field_name("consequence") else {
            return false;
        };
        if consequence.kind() != "compound_statement" {
            return false;
        }

        let mut cursor = consequence.walk();
        let stmts: Vec<Node> = consequence
            .children(&mut cursor)
            .filter(|n| n.kind() != "{" && n.kind() != "}" && n.kind() != "comment")
            .collect();

        // 2-stmt: dest = ptr; ptr = NULL;
        if stmts.len() == 2 {
            let Some((other_expr, ptr_expr)) =
                self.extract_assignment(ast_context, stmts[0], ctx.source)
            else {
                return false;
            };
            if self.is_null_text(ptr_expr) || ptr_expr.starts_with('*') {
                return false;
            }
            if !self.is_null_assign(ast_context, stmts[1], ptr_expr, ctx.source) {
                return false;
            }
            let replacement = format!("{other_expr} = g_steal_pointer (&{ptr_expr});");
            let fix = Fix::from_range(
                consequence.start_byte(),
                consequence.end_byte(),
                ctx,
                &replacement,
            );
            violations.push(self.violation_with_fix(
                ctx.file_path,
                ctx.base_line + stmts[0].start_position().row,
                stmts[0].start_position().column + 1,
                format!("Use g_steal_pointer (&{ptr_expr}) instead of copying and setting to NULL"),
                fix,
            ));
            return true;
        }

        // 3-stmt: T *tmp = ptr; ptr = NULL; return tmp;
        if stmts.len() == 3 {
            let Some((tmp_name, ptr_expr)) =
                self.extract_decl_init(ast_context, stmts[0], ctx.source)
            else {
                return false;
            };
            if !self.is_null_assign(ast_context, stmts[1], ptr_expr, ctx.source) {
                return false;
            }
            if stmts[2].kind() != "return_statement" {
                return false;
            }
            let mut cursor = stmts[2].walk();
            let ret_val = stmts[2]
                .children(&mut cursor)
                .find(|n| n.kind() != "return" && n.kind() != ";");
            let Some(ret_val) = ret_val else {
                return false;
            };
            if ast_context.get_node_text(ret_val, ctx.source) != tmp_name {
                return false;
            }
            let replacement = format!("return g_steal_pointer (&{ptr_expr});");
            let fix = Fix::from_range(
                consequence.start_byte(),
                consequence.end_byte(),
                ctx,
                &replacement,
            );
            violations.push(self.violation_with_fix(
                ctx.file_path,
                ctx.base_line + stmts[0].start_position().row,
                stmts[0].start_position().column + 1,
                format!("Use {replacement} instead of copying {ptr_expr} and setting it to NULL"),
                fix,
            ));
            return true;
        }

        false
    }

    /// Extract the tested pointer expression from an if-condition.
    /// Handles `(expr)`, `(expr != NULL)`, and `(NULL != expr)`.
    fn extract_condition_expr<'a>(
        &self,
        ast_context: &AstContext,
        condition: Node,
        source: &'a [u8],
    ) -> Option<&'a str> {
        if condition.kind() != "parenthesized_expression" {
            return None;
        }
        let mut cursor = condition.walk();
        let inner = condition
            .children(&mut cursor)
            .find(|n| n.kind() != "(" && n.kind() != ")")?;

        if inner.kind() == "binary_expression" {
            let op = inner.child_by_field_name("operator")?;
            if ast_context.get_node_text(op, source) != "!=" {
                return None;
            }
            let left = inner.child_by_field_name("left")?;
            let right = inner.child_by_field_name("right")?;
            let left_text = ast_context.get_node_text(left, source);
            let right_text = ast_context.get_node_text(right, source);
            if self.is_null_text(right_text) {
                Some(left_text)
            } else if self.is_null_text(left_text) {
                Some(right_text)
            } else {
                None
            }
        } else {
            Some(ast_context.get_node_text(inner, source))
        }
    }

    /// Extract `(var_name, source_expr_text)` from `T *var = src;`
    fn extract_decl_init<'a>(
        &self,
        ast_context: &AstContext,
        decl: Node,
        source: &'a [u8],
    ) -> Option<(&'a str, &'a str)> {
        let init_decl = decl.child_by_field_name("declarator")?;
        if init_decl.kind() != "init_declarator" {
            return None;
        }

        let value_node = init_decl.child_by_field_name("value")?;
        let src_text = ast_context.get_node_text(value_node, source);

        // Stealing NULL is pointless
        if self.is_null_text(src_text) {
            return None;
        }

        // Skip dereference expressions — g_steal_pointer (&*expr) is confusing;
        // the caller already holds the pointer and should pass it directly.
        if src_text.starts_with('*') {
            return None;
        }

        let decl_node = init_decl.child_by_field_name("declarator")?;
        let name_node = self.innermost_declarator(decl_node)?;
        let var_name = ast_context.get_node_text(name_node, source);

        Some((var_name, src_text))
    }

    /// Recursively unwrap pointer/parenthesized declarators to get the
    /// identifier.
    fn innermost_declarator<'a>(&self, node: Node<'a>) -> Option<Node<'a>> {
        match node.kind() {
            "identifier" => Some(node),
            "pointer_declarator" | "parenthesized_declarator" => {
                let inner = node.child_by_field_name("declarator")?;
                self.innermost_declarator(inner)
            }
            _ => None,
        }
    }

    /// Extract `(lhs_text, rhs_text)` from `expression_statement →
    /// assignment_expression`
    fn extract_assignment<'a>(
        &self,
        ast_context: &AstContext,
        stmt: Node,
        source: &'a [u8],
    ) -> Option<(&'a str, &'a str)> {
        if stmt.kind() != "expression_statement" {
            return None;
        }
        let mut cursor = stmt.walk();
        let expr = stmt
            .children(&mut cursor)
            .find(|n| n.kind() == "assignment_expression")?;

        let op = expr.child_by_field_name("operator")?;
        if ast_context.get_node_text(op, source) != "=" {
            return None;
        }

        let lhs = expr.child_by_field_name("left")?;
        let rhs = expr.child_by_field_name("right")?;
        Some((
            ast_context.get_node_text(lhs, source),
            ast_context.get_node_text(rhs, source),
        ))
    }

    /// Returns true if `stmt` is `expected_expr = NULL;`
    fn is_null_assign(
        &self,
        ast_context: &AstContext,
        stmt: Node,
        expected_expr: &str,
        source: &[u8],
    ) -> bool {
        let Some((lhs, rhs)) = self.extract_assignment(ast_context, stmt, source) else {
            return false;
        };
        lhs == expected_expr && self.is_null_text(rhs)
    }

    fn is_null_text(&self, text: &str) -> bool {
        text == "NULL"
    }
}
