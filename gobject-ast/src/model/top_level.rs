use super::{SourceLocation, Statement};

/// Represents a top-level item in a C file
#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
pub enum PreprocessorDirective {
    Include {
        path: String,
        is_system: bool,
        location: SourceLocation,
    },
    Define {
        name: String,
        location: SourceLocation,
    },
    Call {
        directive: String,
        location: SourceLocation,
    },
    Other {
        location: SourceLocation,
    },
}

#[derive(Debug, Clone)]
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
        name: String,
        location: SourceLocation,
    },
}

#[derive(Debug, Clone)]
pub struct FunctionDeclItem {
    pub name: String,
    pub is_static: bool,
    pub export_macros: Vec<String>,
    pub location: SourceLocation,
}

#[derive(Debug, Clone)]
pub struct FunctionDefItem {
    pub name: String,
    pub is_static: bool,
    pub body_statements: Vec<Statement>,
    pub location: SourceLocation,
    pub body_location: Option<SourceLocation>,
}
