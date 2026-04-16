use serde::{Deserialize, Serialize};

/// Source location information for AST nodes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceLocation {
    pub line: usize,
    pub column: usize,
    pub start_byte: usize,
    pub end_byte: usize,
}

impl SourceLocation {
    pub fn new(line: usize, column: usize, start_byte: usize, end_byte: usize) -> Self {
        Self {
            line,
            column,
            start_byte,
            end_byte,
        }
    }

    /// Extract the source text for this location
    pub fn as_str<'a>(&self, source: &'a [u8]) -> Option<&'a str> {
        std::str::from_utf8(&source[self.start_byte..self.end_byte]).ok()
    }
}
