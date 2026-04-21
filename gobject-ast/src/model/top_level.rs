use serde::{Deserialize, Serialize};

use super::{SourceLocation, Statement};

/// Represents a top-level item in a C file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TopLevelItem {
    /// Preprocessor directive (#define, #include, etc.)
    Preprocessor(PreprocessorDirective),
    /// Type definition (typedef, enum, struct)
    TypeDefinition(TypeDefItem),
    /// Function declaration (forward declaration)
    FunctionDeclaration(FunctionDeclItem),
    /// Function definition (with body)
    FunctionDefinition(FunctionDefItem),
    /// Standalone declaration (variables, etc.)
    Declaration(Statement),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PragmaKind {
    /// #pragma once
    Once,
    /// #pragma GCC/clang diagnostic push
    DiagnosticPush,
    /// #pragma GCC/clang diagnostic pop
    DiagnosticPop,
    /// #pragma GCC/clang diagnostic ignored "-Wwarning-name"
    DiagnosticIgnored { warning: String },
    /// Other pragma directive
    Other {
        name: String,
        arguments: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PreprocessorDirective {
    Include {
        path: String,
        is_system: bool,
        location: SourceLocation,
    },
    Define {
        name: String,
        value: Option<String>,
        location: SourceLocation,
    },
    Call {
        directive: String,
        location: SourceLocation,
    },
    Pragma {
        kind: PragmaKind,
        location: SourceLocation,
    },
    /// GObject type declaration/definition (G_DECLARE_*, G_DEFINE_*)
    GObjectType {
        gobject_type: Box<super::types::GObjectType>,
        location: SourceLocation,
    },
    Conditional {
        kind: ConditionalKind,
        condition: Option<String>,
        body: Vec<TopLevelItem>,
        location: SourceLocation,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConditionalKind {
    Ifdef,
    Ifndef,
    If,
    Elif,
    Else,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TypeDefItem {
    Typedef {
        name: String,
        target_type: String,
        location: SourceLocation,
    },
    Struct {
        name: String,
        has_body: bool,
        location: SourceLocation,
    },
    Enum {
        enum_info: Box<super::types::EnumInfo>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionDeclItem {
    pub name: String,
    pub return_type: super::TypeInfo,
    pub is_static: bool,
    pub export_macros: Vec<String>,
    pub location: SourceLocation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionDefItem {
    pub name: String,
    pub return_type: super::TypeInfo,
    pub is_static: bool,
    pub parameters: Vec<super::types::Parameter>,
    pub body_statements: Vec<Statement>,
    pub location: SourceLocation,
    pub body_location: Option<SourceLocation>,
}

impl FunctionDefItem {
    /// Find all calls to specific functions in the body
    /// Returns references to all CallExpression nodes that match any of the
    /// given function names
    pub fn find_calls<'a>(
        &'a self,
        function_names: &[&str],
    ) -> Vec<&'a super::expression::CallExpression> {
        self.find_calls_matching(|name| function_names.contains(&name))
    }

    /// Find all calls matching a predicate in the body
    /// Returns references to all CallExpression nodes where the predicate
    /// returns true
    pub fn find_calls_matching<F>(&self, predicate: F) -> Vec<&super::expression::CallExpression>
    where
        F: Fn(&str) -> bool,
    {
        let mut calls = Vec::new();
        self.find_calls_recursive_matching(&self.body_statements, &predicate, &mut calls);
        calls
    }

    fn find_calls_recursive_matching<'a, F>(
        &'a self,
        statements: &'a [Statement],
        predicate: &F,
        calls: &mut Vec<&'a super::expression::CallExpression>,
    ) where
        F: Fn(&str) -> bool,
    {
        for stmt in statements {
            match stmt {
                Statement::Expression(expr_stmt) => {
                    self.find_calls_in_expr_matching(&expr_stmt.expr, predicate, calls);
                }
                Statement::Return(ret) => {
                    if let Some(expr) = &ret.value {
                        self.find_calls_in_expr_matching(expr, predicate, calls);
                    }
                }
                Statement::Declaration(decl) => {
                    if let Some(expr) = &decl.initializer {
                        self.find_calls_in_expr_matching(expr, predicate, calls);
                    }
                }
                Statement::If(if_stmt) => {
                    self.find_calls_in_expr_matching(&if_stmt.condition, predicate, calls);
                    self.find_calls_recursive_matching(&if_stmt.then_body, predicate, calls);
                    if let Some(else_body) = &if_stmt.else_body {
                        self.find_calls_recursive_matching(else_body, predicate, calls);
                    }
                }
                Statement::Compound(compound) => {
                    self.find_calls_recursive_matching(&compound.statements, predicate, calls);
                }
                Statement::Labeled(labeled) => {
                    self.find_calls_recursive_matching(
                        std::slice::from_ref(&labeled.statement),
                        predicate,
                        calls,
                    );
                }
                _ => {}
            }
        }
    }

    fn find_calls_in_expr_matching<'a, F>(
        &'a self,
        expr: &'a super::expression::Expression,
        predicate: &F,
        calls: &mut Vec<&'a super::expression::CallExpression>,
    ) where
        F: Fn(&str) -> bool,
    {
        use super::expression::{Argument, Expression};
        match expr {
            Expression::Call(call) => {
                if call.function_name_str().is_some_and(|name| predicate(name)) {
                    calls.push(call);
                }
                // Also check arguments
                for arg in &call.arguments {
                    let Argument::Expression(e) = arg;
                    self.find_calls_in_expr_matching(e, predicate, calls);
                }
            }
            Expression::Assignment(assign) => {
                self.find_calls_in_expr_matching(&assign.rhs, predicate, calls);
            }
            Expression::Binary(binary) => {
                self.find_calls_in_expr_matching(&binary.left, predicate, calls);
                self.find_calls_in_expr_matching(&binary.right, predicate, calls);
            }
            Expression::Unary(unary) => {
                self.find_calls_in_expr_matching(&unary.operand, predicate, calls);
            }
            _ => {}
        }
    }

    /// Collect all return values from the function body
    pub fn collect_return_values(&self) -> Vec<&super::expression::Expression> {
        let mut values = Vec::new();
        for stmt in &self.body_statements {
            collect_returns(stmt, &mut values);
        }
        values
    }

    /// Find all g_object_class_install_properties calls in the function body
    pub fn find_install_properties_calls(&self) -> Vec<&super::expression::CallExpression> {
        self.find_calls(&["g_object_class_install_properties"])
    }

    /// Get a parameter by name
    pub fn get_param_by_name(&self, name: &str) -> Option<&super::types::Parameter> {
        self.parameters
            .iter()
            .find(|p| p.name.as_ref().map(|n| n == name).unwrap_or(false))
    }

    /// Find all param_spec assignments in the function body
    /// Handles both array pattern (props[PROP_X] = ...) and variable pattern
    /// (param_spec = ...)
    pub fn find_param_spec_assignments(
        &self,
        source: &[u8],
    ) -> Vec<super::types::ParamSpecAssignment> {
        use super::{Statement, expression::Expression, types::ParamSpecAssignment};

        let mut assignments = Vec::new();

        for stmt in &self.body_statements {
            if let Statement::Expression(expr_stmt) = stmt
                && let Expression::Assignment(assignment) = &expr_stmt.expr
            {
                // Check if RHS is a g_param_spec_* call
                if let Expression::Call(param_call) = &*assignment.rhs {
                    let func_name = param_call.function_name();
                    if !func_name.contains("_param_spec_") {
                        continue;
                    }

                    // Extract property name from first argument
                    let property_name = if let Some(name_arg) = param_call.get_arg(0)
                        && let Expression::StringLiteral(name_lit) = name_arg
                    {
                        name_lit.value.trim_matches('"').to_string()
                    } else {
                        continue;
                    };

                    // Check LHS: array subscript or variable?
                    if let Expression::Subscript(subscript) = &*assignment.lhs {
                        // Array pattern: props[PROP_X] = g_param_spec_*()
                        if let Some(array_name) = subscript.array.to_source_string(source)
                            && let Some(enum_value) = subscript.index.to_source_string(source)
                        {
                            assignments.push(ParamSpecAssignment::ArraySubscript {
                                array_name,
                                enum_value,
                                property_name,
                                statement_location: stmt.location().clone(),
                                call: param_call.clone(),
                            });
                        }
                    } else if let Some(var_name) = assignment.lhs.to_source_string(source) {
                        // Variable pattern: param_spec = g_param_spec_*()
                        assignments.push(ParamSpecAssignment::Variable {
                            variable_name: var_name,
                            property_name,
                            statement_location: stmt.location().clone(),
                            call: param_call.clone(),
                        });
                    }
                }
            }
        }

        assignments
    }
}

fn collect_returns<'a>(stmt: &'a Statement, values: &mut Vec<&'a super::expression::Expression>) {
    match stmt {
        Statement::Return(ret_stmt) => {
            if let Some(value) = &ret_stmt.value {
                values.push(value);
            }
        }
        Statement::If(if_stmt) => {
            for s in &if_stmt.then_body {
                collect_returns(s, values);
            }
            if let Some(else_body) = &if_stmt.else_body {
                for s in else_body {
                    collect_returns(s, values);
                }
            }
        }
        Statement::Compound(compound) => {
            for s in &compound.statements {
                collect_returns(s, values);
            }
        }
        Statement::Labeled(labeled) => {
            collect_returns(&labeled.statement, values);
        }
        _ => {}
    }
}
