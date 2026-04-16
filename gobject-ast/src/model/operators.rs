use serde::{Deserialize, Serialize};

/// Binary operators in C
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BinaryOp {
    // Arithmetic
    Add,      // +
    Subtract, // -
    Multiply, // *
    Divide,   // /
    Modulo,   // %

    // Comparison
    Equal,        // ==
    NotEqual,     // !=
    Less,         // <
    LessEqual,    // <=
    Greater,      // >
    GreaterEqual, // >=

    // Logical
    LogicalAnd, // &&
    LogicalOr,  // ||

    // Bitwise
    BitwiseAnd, // &
    BitwiseOr,  // |
    BitwiseXor, // ^
    LeftShift,  // <<
    RightShift, // >>
}

impl BinaryOp {
    /// Parse from operator string
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "+" => Some(Self::Add),
            "-" => Some(Self::Subtract),
            "*" => Some(Self::Multiply),
            "/" => Some(Self::Divide),
            "%" => Some(Self::Modulo),
            "==" => Some(Self::Equal),
            "!=" => Some(Self::NotEqual),
            "<" => Some(Self::Less),
            "<=" => Some(Self::LessEqual),
            ">" => Some(Self::Greater),
            ">=" => Some(Self::GreaterEqual),
            "&&" => Some(Self::LogicalAnd),
            "||" => Some(Self::LogicalOr),
            "&" => Some(Self::BitwiseAnd),
            "|" => Some(Self::BitwiseOr),
            "^" => Some(Self::BitwiseXor),
            "<<" => Some(Self::LeftShift),
            ">>" => Some(Self::RightShift),
            _ => None,
        }
    }

    /// Convert to operator string
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Add => "+",
            Self::Subtract => "-",
            Self::Multiply => "*",
            Self::Divide => "/",
            Self::Modulo => "%",
            Self::Equal => "==",
            Self::NotEqual => "!=",
            Self::Less => "<",
            Self::LessEqual => "<=",
            Self::Greater => ">",
            Self::GreaterEqual => ">=",
            Self::LogicalAnd => "&&",
            Self::LogicalOr => "||",
            Self::BitwiseAnd => "&",
            Self::BitwiseOr => "|",
            Self::BitwiseXor => "^",
            Self::LeftShift => "<<",
            Self::RightShift => ">>",
        }
    }

    /// Check if this is a comparison operator
    pub fn is_comparison(&self) -> bool {
        matches!(
            self,
            Self::Equal
                | Self::NotEqual
                | Self::Less
                | Self::LessEqual
                | Self::Greater
                | Self::GreaterEqual
        )
    }
}

/// Unary operators in C
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnaryOp {
    Not,            // !
    BitwiseNot,     // ~
    Negate,         // - (unary minus)
    Plus,           // + (unary plus)
    Dereference,    // *
    AddressOf,      // &
    PreIncrement,   // ++x (handled by UpdateExpression but can appear)
    PreDecrement,   // --x (handled by UpdateExpression but can appear)
    PostIncrement,  // x++ (handled by UpdateExpression but can appear)
    PostDecrement,  // x-- (handled by UpdateExpression but can appear)
}

impl UnaryOp {
    /// Parse from operator string
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "!" => Some(Self::Not),
            "~" => Some(Self::BitwiseNot),
            "-" => Some(Self::Negate),
            "+" => Some(Self::Plus),
            "*" => Some(Self::Dereference),
            "&" => Some(Self::AddressOf),
            "++" => Some(Self::PreIncrement),
            "--" => Some(Self::PreDecrement),
            _ => None,
        }
    }

    /// Convert to operator string
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Not => "!",
            Self::BitwiseNot => "~",
            Self::Negate => "-",
            Self::Plus => "+",
            Self::Dereference => "*",
            Self::AddressOf => "&",
            Self::PreIncrement | Self::PostIncrement => "++",
            Self::PreDecrement | Self::PostDecrement => "--",
        }
    }
}

/// Update operators (increment/decrement)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UpdateOp {
    Increment, // ++
    Decrement, // --
}

impl UpdateOp {
    /// Parse from operator string
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "++" => Some(Self::Increment),
            "--" => Some(Self::Decrement),
            _ => None,
        }
    }

    /// Convert to operator string
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Increment => "++",
            Self::Decrement => "--",
        }
    }
}

/// Assignment operators in C
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssignmentOp {
    Assign,            // =
    AddAssign,         // +=
    SubtractAssign,    // -=
    MultiplyAssign,    // *=
    DivideAssign,      // /=
    ModuloAssign,      // %=
    BitwiseAndAssign,  // &=
    BitwiseOrAssign,   // |=
    BitwiseXorAssign,  // ^=
    LeftShiftAssign,   // <<=
    RightShiftAssign,  // >>=
}

impl AssignmentOp {
    /// Parse from operator string
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "=" => Some(Self::Assign),
            "+=" => Some(Self::AddAssign),
            "-=" => Some(Self::SubtractAssign),
            "*=" => Some(Self::MultiplyAssign),
            "/=" => Some(Self::DivideAssign),
            "%=" => Some(Self::ModuloAssign),
            "&=" => Some(Self::BitwiseAndAssign),
            "|=" => Some(Self::BitwiseOrAssign),
            "^=" => Some(Self::BitwiseXorAssign),
            "<<=" => Some(Self::LeftShiftAssign),
            ">>=" => Some(Self::RightShiftAssign),
            _ => None,
        }
    }

    /// Convert to operator string
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Assign => "=",
            Self::AddAssign => "+=",
            Self::SubtractAssign => "-=",
            Self::MultiplyAssign => "*=",
            Self::DivideAssign => "/=",
            Self::ModuloAssign => "%=",
            Self::BitwiseAndAssign => "&=",
            Self::BitwiseOrAssign => "|=",
            Self::BitwiseXorAssign => "^=",
            Self::LeftShiftAssign => "<<=",
            Self::RightShiftAssign => ">>=",
        }
    }
}
