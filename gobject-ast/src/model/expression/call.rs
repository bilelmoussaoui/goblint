use serde::{Deserialize, Serialize};

use crate::model::{Expression, SourceLocation};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallExpression {
    pub function: String,
    pub arguments: Vec<Argument>,
    pub location: SourceLocation,
}

impl CallExpression {
    /// Get argument as source text
    pub fn get_arg_text(&self, index: usize, source: &[u8]) -> Option<String> {
        self.arguments.get(index)?.to_source_string(source)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Argument {
    Expression(Box<Expression>),
    // Add more specific types as needed
}

impl Argument {
    /// Convert this argument back to source text
    pub fn to_source_string(&self, source: &[u8]) -> Option<String> {
        match self {
            Argument::Expression(expr) => expr.to_source_string(source),
        }
    }
}
