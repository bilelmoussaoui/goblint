mod assignment;
mod binary;
mod call;
mod cast;
mod conditional;
mod field_access;
mod identifier;
mod initializer_list;
mod literal;
mod macro_call;
mod sizeof;
mod subscript;
mod unary;
mod update;

pub use assignment::Assignment;
pub use binary::BinaryExpression;
pub use call::{Argument, CallExpression};
pub use cast::CastExpression;
pub use conditional::ConditionalExpression;
pub use field_access::FieldAccessExpression;
pub use identifier::IdentifierExpression;
pub use initializer_list::{Designator, InitializerItem, InitializerListExpression};
pub use literal::{
    BooleanExpression, CharLiteralExpression, CommentExpression, GenericExpression, NullExpression,
    NumberLiteralExpression, StringLiteralExpression,
};
pub use macro_call::MacroCallExpression;
use serde::{Deserialize, Serialize};
pub use sizeof::{SizeofExpression, SizeofOperand};
pub use subscript::SubscriptExpression;
pub use unary::UnaryExpression;
pub use update::UpdateExpression;

use crate::model::SourceLocation;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Expression {
    Call(CallExpression),
    MacroCall(MacroCallExpression),
    Assignment(Assignment),
    Binary(BinaryExpression),
    Unary(UnaryExpression),
    Identifier(IdentifierExpression),
    FieldAccess(FieldAccessExpression),
    StringLiteral(StringLiteralExpression),
    NumberLiteral(NumberLiteralExpression),
    Null(NullExpression),
    Boolean(BooleanExpression),
    Cast(CastExpression),
    Conditional(ConditionalExpression),
    Sizeof(SizeofExpression),
    Subscript(SubscriptExpression),
    InitializerList(InitializerListExpression),
    CharLiteral(CharLiteralExpression),
    Update(UpdateExpression),
    Comment(CommentExpression),
    Generic(GenericExpression),
}

impl Expression {
    /// Get the byte range of this expression
    pub fn byte_range(&self) -> (usize, usize) {
        match self {
            Expression::Call(c) => (c.location.start_byte, c.location.end_byte),
            Expression::MacroCall(m) => (m.location.start_byte, m.location.end_byte),
            Expression::Assignment(a) => (a.location.start_byte, a.location.end_byte),
            Expression::Binary(b) => (b.location.start_byte, b.location.end_byte),
            Expression::Unary(u) => (u.location.start_byte, u.location.end_byte),
            Expression::Identifier(i) => (i.location.start_byte, i.location.end_byte),
            Expression::FieldAccess(f) => (f.location.start_byte, f.location.end_byte),
            Expression::StringLiteral(s) => (s.location.start_byte, s.location.end_byte),
            Expression::NumberLiteral(n) => (n.location.start_byte, n.location.end_byte),
            Expression::Null(n) => (n.location.start_byte, n.location.end_byte),
            Expression::Boolean(b) => (b.location.start_byte, b.location.end_byte),
            Expression::Cast(c) => (c.location.start_byte, c.location.end_byte),
            Expression::Conditional(c) => (c.location.start_byte, c.location.end_byte),
            Expression::Sizeof(s) => (s.location.start_byte, s.location.end_byte),
            Expression::Subscript(s) => (s.location.start_byte, s.location.end_byte),
            Expression::InitializerList(i) => (i.location.start_byte, i.location.end_byte),
            Expression::CharLiteral(c) => (c.location.start_byte, c.location.end_byte),
            Expression::Update(u) => (u.location.start_byte, u.location.end_byte),
            Expression::Comment(c) => (c.location.start_byte, c.location.end_byte),
            Expression::Generic(g) => (g.location.start_byte, g.location.end_byte),
        }
    }

    pub fn location(&self) -> &SourceLocation {
        match self {
            Expression::Call(c) => &c.location,
            Expression::MacroCall(m) => &m.location,
            Expression::Assignment(a) => &a.location,
            Expression::Binary(b) => &b.location,
            Expression::Unary(u) => &u.location,
            Expression::Identifier(i) => &i.location,
            Expression::FieldAccess(f) => &f.location,
            Expression::StringLiteral(s) => &s.location,
            Expression::NumberLiteral(n) => &n.location,
            Expression::Null(n) => &n.location,
            Expression::Boolean(b) => &b.location,
            Expression::Cast(c) => &c.location,
            Expression::Conditional(c) => &c.location,
            Expression::Sizeof(s) => &s.location,
            Expression::Subscript(s) => &s.location,
            Expression::InitializerList(i) => &i.location,
            Expression::CharLiteral(c) => &c.location,
            Expression::Update(u) => &u.location,
            Expression::Comment(c) => &c.location,
            Expression::Generic(g) => &g.location,
        }
    }

    /// Convert this expression back to source text
    pub fn to_source_string(&self, source: &[u8]) -> Option<String> {
        let (start, end) = self.byte_range();
        std::str::from_utf8(&source[start..end])
            .ok()
            .map(ToOwned::to_owned)
    }

    /// Recursively walk all nested expressions. The closure receives a
    /// `&'s Expression` tied to `self`'s lifetime, so references extracted
    /// inside the closure can be stored in an outer `Vec<&'s T>`.
    pub fn walk<'s, F>(&'s self, f: &mut F)
    where
        F: FnMut(&'s Expression),
    {
        f(self);
        match self {
            Expression::Call(call) => {
                call.function.walk(f);
                for arg in &call.arguments {
                    let Argument::Expression(e) = arg;
                    e.walk(f);
                }
            }
            Expression::MacroCall(macro_call) => {
                for arg in &macro_call.arguments {
                    let Argument::Expression(e) = arg;
                    e.walk(f);
                }
            }
            Expression::Assignment(assign) => {
                assign.lhs.walk(f);
                assign.rhs.walk(f);
            }
            Expression::Unary(unary) => {
                unary.operand.walk(f);
            }
            Expression::Binary(binary) => {
                binary.left.walk(f);
                binary.right.walk(f);
            }
            Expression::Cast(cast) => {
                cast.operand.walk(f);
            }
            Expression::Conditional(cond) => {
                cond.condition.walk(f);
                cond.then_expr.walk(f);
                cond.else_expr.walk(f);
            }
            Expression::Subscript(subscript) => {
                subscript.array.walk(f);
                subscript.index.walk(f);
            }
            Expression::Update(update) => {
                update.operand.walk(f);
            }
            Expression::FieldAccess(field) => {
                field.base.walk(f);
            }
            Expression::InitializerList(init) => {
                for item in &init.items {
                    item.value.walk(f);
                }
            }
            Expression::Identifier(_)
            | Expression::StringLiteral(_)
            | Expression::NumberLiteral(_)
            | Expression::Null(_)
            | Expression::Boolean(_)
            | Expression::Sizeof(_)
            | Expression::CharLiteral(_)
            | Expression::Comment(_)
            | Expression::Generic(_) => {}
        }
    }

    /// Extract variable name from simple expressions (Identifier or
    /// FieldAccess)
    pub fn extract_variable_name(&self) -> Option<String> {
        match self {
            Expression::Identifier(id) => Some(id.name.clone()),
            Expression::FieldAccess(f) => Some(f.text()),
            _ => None,
        }
    }

    /// Check if this expression is NULL
    /// Handles both Expression::Null and the identifier "NULL" (common in C
    /// code)
    pub fn is_null(&self) -> bool {
        matches!(self, Expression::Null(_))
            || matches!(self, Expression::Identifier(id) if id.name == "NULL")
    }

    /// Check if this expression is the number 0
    pub fn is_zero(&self) -> bool {
        matches!(self, Expression::NumberLiteral(n) if n.value.trim() == "0")
    }

    /// Convert simple expressions (identifier, number literal, boolean) to
    /// strings Useful for comparing return values or simple constants
    pub fn to_simple_string(&self) -> Option<String> {
        match self {
            Expression::Identifier(id) => Some(id.name.clone()),
            Expression::NumberLiteral(n) => Some(n.value.clone()),
            Expression::Boolean(b) => Some(if b.value {
                "true".to_string()
            } else {
                "false".to_string()
            }),
            _ => None,
        }
    }

    /// Generate a text representation of an expression
    /// Used for field access text generation where we need to reconstruct
    /// expressions like "function()->field"
    pub fn to_text(&self) -> String {
        match self {
            Expression::Identifier(id) => id.name.clone(),
            Expression::NumberLiteral(n) => n.value.clone(),
            Expression::StringLiteral(s) => s.value.clone(),
            Expression::CharLiteral(c) => c.value.clone(),
            Expression::Boolean(b) => if b.value { "true" } else { "false" }.to_string(),
            Expression::Call(call) => {
                let func = call.function.to_text();
                let args = call
                    .arguments
                    .iter()
                    .map(|arg| match arg {
                        crate::model::expression::Argument::Expression(e) => e.to_text(),
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{}({})", func, args)
            }
            Expression::FieldAccess(f) => f.text(),
            Expression::Unary(u) => {
                format!("{}{}", u.operator.as_str(), u.operand.to_text())
            }
            Expression::Binary(b) => {
                format!(
                    "{} {} {}",
                    b.left.to_text(),
                    b.operator.as_str(),
                    b.right.to_text()
                )
            }
            Expression::Cast(c) => {
                format!("({}){}", c.type_info.full_text, c.operand.to_text())
            }
            Expression::Subscript(s) => {
                format!("{}[{}]", s.array.to_text(), s.index.to_text())
            }
            // For other complex expressions, just return a placeholder
            _ => "<expr>".to_string(),
        }
    }

    /// Check if this expression is a string literal
    pub fn is_string_literal(&self) -> bool {
        matches!(self, Expression::StringLiteral(_))
    }

    /// Extract string literal value, unwrapping macro calls like I_("string")
    /// Returns the string without quotes
    pub fn extract_string_value(&self) -> Option<String> {
        match self {
            Expression::StringLiteral(lit) => Some(lit.value.trim_matches('"').to_string()),
            Expression::MacroCall(macro_call) => {
                macro_call.extract_string_literal().map(|s| s.to_string())
            }
            _ => None,
        }
    }

    /// Check if this is a string literal or a macro wrapping a string literal
    pub fn is_string_or_macro_string(&self) -> bool {
        self.extract_string_value().is_some()
    }

    /// Check if this expression contains an identifier with the given name
    /// Recursively searches through the entire expression tree
    pub fn contains_identifier(&self, name: &str) -> bool {
        let mut found = false;
        self.walk(&mut |e| {
            if let Expression::Identifier(id) = e {
                if id.name == name {
                    found = true;
                }
            }
        });
        found
    }

    /// Collect all identifiers in this expression
    /// Returns a list of all identifier names found in the expression tree
    pub fn collect_identifiers(&self) -> Vec<String> {
        let mut identifiers = Vec::new();
        self.walk(&mut |e| {
            if let Expression::Identifier(id) = e {
                identifiers.push(id.name.clone());
            }
        });
        identifiers
    }

    /// Check if this expression is a call to the specified function
    pub fn is_call_to(&self, function_name: &str) -> bool {
        matches!(self, Expression::Call(call) if call.is_function(function_name))
    }

    /// Check if this expression is a call to any of the specified functions
    pub fn is_call_to_any(&self, function_names: &[&str]) -> bool {
        matches!(self, Expression::Call(call) if call.function_name_str().is_some_and(|name| function_names.contains(&name)))
    }
}
