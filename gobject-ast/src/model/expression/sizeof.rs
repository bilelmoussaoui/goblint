use serde::{Deserialize, Serialize};

use super::Expression;
use crate::model::SourceLocation;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SizeofExpression {
    pub operand: Option<SizeofOperand>,
    pub text: String, // Full text like "sizeof(int)" or "sizeof x"
    pub location: SourceLocation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SizeofOperand {
    Type(crate::TypeInfo),       // sizeof(MyType) or sizeof(struct MyType *)
    Expression(Box<Expression>), // sizeof(expr)
}

impl SizeofExpression {
    /// Get the type if this is sizeof(Type)
    /// Returns Some for both explicit types and simple identifiers (which are
    /// likely types)
    pub fn type_name(&self) -> Option<String> {
        match &self.operand {
            Some(SizeofOperand::Type(t)) => Some(t.base_type.clone()),
            // If it's a simple identifier, it's likely a type name
            Some(SizeofOperand::Expression(expr)) => expr.extract_variable_name(),
            None => None,
        }
    }

    /// Check if this is sizeof of a simple type (not a complex expression)
    pub fn is_sizeof_type(&self) -> bool {
        match &self.operand {
            Some(SizeofOperand::Type(_)) => true,
            // Simple identifier is likely a type
            Some(SizeofOperand::Expression(expr)) => {
                matches!(expr.as_ref(), Expression::Identifier(_))
            }
            None => false,
        }
    }
}
