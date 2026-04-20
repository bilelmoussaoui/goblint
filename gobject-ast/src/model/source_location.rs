use serde::{Deserialize, Serialize};

/// Source location information for AST nodes
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
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

    /// Find the start and end byte positions of the line containing this
    /// location Returns (line_start_byte, line_end_byte) including the
    /// newline If the previous line is empty (only whitespace), includes it
    /// too
    pub fn find_line_bounds(&self, source: &[u8]) -> (usize, usize) {
        // Find the start of the line
        let mut line_start = self.start_byte;
        while line_start > 0 && source[line_start - 1] != b'\n' {
            line_start -= 1;
        }

        // Check if the previous line is empty (only whitespace)
        if line_start > 0 {
            let mut prev_line_start = line_start - 1; // Skip the '\n'
            while prev_line_start > 0 && source[prev_line_start - 1] != b'\n' {
                prev_line_start -= 1;
            }

            // Check if the line is only whitespace
            let prev_line = &source[prev_line_start..line_start - 1];
            if prev_line.iter().all(|&b| b == b' ' || b == b'\t') {
                line_start = prev_line_start;
            }
        }

        // Find the end of the line (including newline)
        let mut line_end = self.start_byte;
        while line_end < source.len() && source[line_end] != b'\n' {
            line_end += 1;
        }
        // Include the newline character
        if line_end < source.len() && source[line_end] == b'\n' {
            line_end += 1;
        }

        (line_start, line_end)
    }

    /// Find the start and end byte positions of the line containing this
    /// location, including any following blank line
    /// Returns (line_start_byte, line_end_byte) including newlines
    pub fn find_line_bounds_with_following_blank(&self, source: &[u8]) -> (usize, usize) {
        // Find the start of the line
        let mut line_start = self.start_byte;
        while line_start > 0 && source[line_start - 1] != b'\n' {
            line_start -= 1;
        }

        // Find the end of the line (including newline)
        let mut line_end = self.end_byte;
        while line_end < source.len() && source[line_end] != b'\n' {
            line_end += 1;
        }
        if line_end < source.len() {
            line_end += 1; // Include the newline
        }

        // Check if the next line is blank (only whitespace)
        if line_end < source.len() {
            let mut pos = line_end;
            let mut is_blank = true;

            while pos < source.len() && source[pos] != b'\n' {
                if !source[pos].is_ascii_whitespace() {
                    is_blank = false;
                    break;
                }
                pos += 1;
            }

            // If next line is blank, include it in the removal
            if is_blank && pos < source.len() {
                line_end = pos + 1; // Include the newline of the blank line
            }
        }

        (line_start, line_end)
    }
}
