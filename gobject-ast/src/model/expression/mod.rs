mod identifier;
mod field_access;
mod literal;
mod call;
mod assignment;
mod binary;
mod unary;
mod cast;
mod conditional;
mod sizeof;
mod subscript;
mod initializer_list;
mod update;

pub use identifier::IdentifierExpression;
pub use field_access::FieldAccessExpression;
pub use literal::{StringLiteralExpression, NumberLiteralExpression, CharLiteralExpression, NullExpression, BooleanExpression};
pub use call::{CallExpression, Argument};
pub use assignment::Assignment;
pub use binary::BinaryExpression;
pub use unary::UnaryExpression;
pub use cast::CastExpression;
pub use conditional::ConditionalExpression;
pub use sizeof::SizeofExpression;
pub use subscript::SubscriptExpression;
pub use initializer_list::InitializerListExpression;
pub use update::UpdateExpression;

use serde::{Deserialize, Serialize};

use crate::model::SourceLocation;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Expression {
    Call(CallExpression),
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
}

impl Expression {
    /// Get the byte range of this expression
    pub fn byte_range(&self) -> (usize, usize) {
        match self {
            Expression::Call(c) => (c.location.start_byte, c.location.end_byte),
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
        }
    }

    pub fn location(&self) -> &SourceLocation {
        match self {
            Expression::Call(c) => &c.location,
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
        }
    }

    /// Convert this expression back to source text
    pub fn to_source_string(&self, source: &[u8]) -> Option<String> {
        let (start, end) = self.byte_range();
        std::str::from_utf8(&source[start..end])
            .ok()
            .map(ToOwned::to_owned)
    }

    /// Recursively walk all nested expressions
    pub fn walk<F>(&self, f: &mut F)
    where
        F: FnMut(&Expression),
    {
        f(self);
        match self {
            Expression::Call(call) => {
                for arg in &call.arguments {
                    let Argument::Expression(e) = arg;
                    e.walk(f);
                }
            }
            Expression::Assignment(assign) => {
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
            _ => {}
        }
    }

    /// Extract variable name from simple expressions (Identifier or
    /// FieldAccess)
    pub fn extract_variable_name(&self) -> Option<String> {
        match self {
            Expression::Identifier(id) => Some(id.name.clone()),
            Expression::FieldAccess(f) => Some(f.text.clone()),
            _ => None,
        }
    }

    /// Check if this expression is NULL
    pub fn is_null(&self) -> bool {
        matches!(self, Expression::Null(_))
    }

    /// Check if this expression is the number 0
    pub fn is_zero(&self) -> bool {
        matches!(self, Expression::NumberLiteral(n) if n.value.trim() == "0")
    }

    /// Check if this expression is a string literal
    pub fn is_string_literal(&self) -> bool {
        matches!(self, Expression::StringLiteral(_))
    }
}
