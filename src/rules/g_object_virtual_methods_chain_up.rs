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
        func: &gobject_ast::top_level::FunctionDefItem,
        path: &std::path::Path,
        violations: &mut Vec<Violation>,
    ) {
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

        let first_param = &func.parameters[0];

        // Must be a pointer type
        if !first_param.type_info.is_pointer() {
            return;
        }

        // Must be GObject or a type ending in "Object"
        // This matches: GObject*, MyObject*, FooBarObject*, etc.
        if !first_param.type_info.is_base_type("GObject") {
            return;
        }

        // Check if it chains up to parent class
        if !self.has_chainup_call(&func.body_statements, method_type) {
            violations.push(self.violation(
                path,
                func.location.line,
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

        // The chain-up lives inside a call: `<parent_base>->method(args)`
        // <parent_base> is either a direct identifier or a CLASS-macro call.
        let field_access = match expr {
            Expression::FieldAccess(f) => Some(f),
            Expression::Call(call) => match &*call.function {
                Expression::FieldAccess(f) => Some(f),
                _ => None,
            },
            _ => None,
        };

        if let Some(field) = field_access
            && field.field == method_type
            && self.looks_like_parent_class_base(&field.base)
        {
            return true;
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

    /// Returns true when `expr` looks like a parent-class base, i.e.:
    ///   - a plain identifier: `parent_class`, `object_class`, `klass`, …
    ///   - a CLASS-macro call: `G_OBJECT_CLASS(parent_class)`,
    ///     `FOO_CLASS(klass)`, …
    fn looks_like_parent_class_base(&self, expr: &gobject_ast::Expression) -> bool {
        use gobject_ast::{Expression, model::expression::Argument};

        match expr {
            Expression::Identifier(id) => self.is_parent_class_name(&id.name),
            Expression::Call(call) => {
                // Function must be an ALL_CAPS identifier ending in _CLASS
                let func_is_class_macro = matches!(&*call.function,
                    Expression::Identifier(id) if id.name.ends_with("_CLASS")
                );
                if !func_is_class_macro {
                    return false;
                }
                // At least one argument must look like a parent class identifier
                call.arguments.iter().any(|arg| {
                    let Argument::Expression(e) = arg;
                    matches!(&**e, Expression::Identifier(id) if self.is_parent_class_name(&id.name))
                })
            }
            _ => false,
        }
    }

    fn is_parent_class_name(&self, name: &str) -> bool {
        let lower = name.to_lowercase();
        // parent_class, parent_klass, foo_parent_class, object_parent_class, …
        if lower.contains("parent") && (lower.contains("class") || lower.contains("klass")) {
            return true;
        }
        // object_class, gobject_class, widget_class, …
        if lower.ends_with("_class") || lower.ends_with("_klass") {
            return true;
        }
        lower == "klass"
    }
}
