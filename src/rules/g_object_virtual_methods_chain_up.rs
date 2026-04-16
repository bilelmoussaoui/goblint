use gobject_ast::Statement;

use super::Rule;
use crate::{ast_context::AstContext, config::Config, rules::Violation};

pub struct GObjectVirtualMethodsChainUp;

impl Rule for GObjectVirtualMethodsChainUp {
    fn name(&self) -> &'static str {
        "g_object_virtual_methods_chain_up"
    }

    fn description(&self) -> &'static str {
        "Ensure dispose/finalize/constructed methods chain up to parent class"
    }

    fn category(&self) -> super::Category {
        super::Category::Correctness
    }

    fn check_func_impl(
        &self,
        _ast_context: &AstContext,
        _config: &Config,
        func: &gobject_ast::FunctionInfo,
        path: &std::path::Path,
        violations: &mut Vec<Violation>,
    ) {
        if !func.is_definition {
            return;
        }

        // Check if function name ends with _dispose, _finalize, or _constructed
        let method_type = if func.name.ends_with("_dispose") {
            "dispose"
        } else if func.name.ends_with("_finalize") {
            "finalize"
        } else if func.name.ends_with("_constructed") {
            "constructed"
        } else {
            return;
        };

        // Verify it's a GObject virtual method by checking the first parameter
        // GObject virtual methods take GObject* or a derived type (XxxObject*) as first
        // param
        if func.parameters.is_empty() {
            return;
        }

        let first_param_type = &func.parameters[0].type_name;

        // Must be a pointer type
        if !first_param_type.contains("*") {
            return;
        }

        // Must be GObject or a type ending in "Object"
        // This matches: GObject*, MyObject*, FooBarObject*, etc.
        if !first_param_type.contains("GObject") && !first_param_type.contains("Object *") {
            return;
        }

        // Check if it chains up to parent class
        if !self.has_chainup_call(&func.body_statements, method_type) {
            violations.push(self.violation(
                path,
                func.line,
                1,
                format!(
                    "{} must chain up to parent class (e.g., G_OBJECT_CLASS (parent_class)->{} (object))",
                    func.name, method_type
                ),
            ));
        }
    }
}

impl GObjectVirtualMethodsChainUp {
    fn has_chainup_call(&self, statements: &[Statement], method_type: &str) -> bool {
        for stmt in statements {
            let mut found = false;
            stmt.walk(&mut |s| {
                // Check expressions for field access like parent_class->dispose
                for expr in s.expressions() {
                    if self.check_expression_for_chainup(expr, method_type) {
                        found = true;
                    }
                }
            });
            if found {
                return true;
            }
        }
        false
    }

    fn check_expression_for_chainup(
        &self,
        expr: &gobject_ast::Expression,
        method_type: &str,
    ) -> bool {
        use gobject_ast::Expression;

        match expr {
            // Field access: parent_class->dispose
            Expression::FieldAccess(field) => {
                // field.text contains the whole access like "parent_class->dispose"
                // Check if it ends with ->method_type
                let expected_suffix = format!("->{}", method_type);
                if field.text.ends_with(&expected_suffix) {
                    // Check if the base looks like a parent class
                    if self.looks_like_parent_class_variable(&field.text)
                        || self.looks_like_parent_class_cast(&field.text)
                    {
                        return true;
                    }
                }
            }
            // Call expression: might contain field access as part of it
            Expression::Call(call) => {
                // Check if the function itself is a field access
                if call.function.contains("->")
                    && call.function.ends_with(method_type)
                    && (self.looks_like_parent_class_variable(&call.function)
                        || self.looks_like_parent_class_cast(&call.function))
                {
                    return true;
                }
            }
            _ => {}
        }

        // Recursively check sub-expressions
        let mut found = false;
        expr.walk(&mut |e| {
            if !std::ptr::eq(e, expr) && self.check_expression_for_chainup(e, method_type) {
                found = true;
            }
        });
        found
    }

    fn looks_like_parent_class_cast(&self, text: &str) -> bool {
        // Common patterns for parent class access:
        // G_OBJECT_CLASS (parent_class)
        // G_OBJECT_CLASS (my_class_parent_class)
        // FOO_CLASS (parent_class)
        text.contains("_CLASS") && text.contains("parent")
    }

    fn looks_like_parent_class_variable(&self, text: &str) -> bool {
        // Common variable names that hold parent class:
        // parent_object_class, parent_class, object_class, klass, parent_klass

        // Extract just the variable name if this is a field access like
        // "object_class->dispose"
        let var_name = if let Some(arrow_pos) = text.find("->") {
            &text[..arrow_pos]
        } else {
            text
        };

        let var_lower = var_name.to_lowercase();

        // Check for explicit parent references
        if var_lower.contains("parent")
            && (var_lower.contains("class") || var_lower.contains("klass"))
        {
            return true;
        }

        // Check for *_class or *_klass variables (object_class, gobject_class, etc.)
        if var_lower.ends_with("_class") || var_lower.ends_with("_klass") {
            return true;
        }

        // Just "klass" or "class" by itself
        if var_lower == "klass" || var_lower == "class" {
            return true;
        }

        false
    }
}
