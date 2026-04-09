use std::{collections::HashMap, fs, path::Path};

use anyhow::{Context, Result};

use crate::rules::Violation;

/// Apply fixes to files
pub fn apply_fixes(violations: &[Violation]) -> Result<usize> {
    // Group violations by file
    let mut by_file: HashMap<&Path, Vec<&Violation>> = HashMap::new();
    for violation in violations {
        if violation.fix.is_some() {
            by_file
                .entry(violation.file.as_path())
                .or_default()
                .push(violation);
        }
    }

    let mut total_fixed = 0;

    for (file_path, mut file_violations) in by_file {
        // Sort by start_byte descending - apply fixes from bottom to top
        // This way earlier fixes don't invalidate byte positions of later fixes
        file_violations.sort_by(|a, b| {
            let a_start = a.fix.as_ref().unwrap().start_byte;
            let b_start = b.fix.as_ref().unwrap().start_byte;
            b_start.cmp(&a_start)
        });

        // Read file content as bytes
        let content = fs::read(file_path)
            .with_context(|| format!("Failed to read file: {}", file_path.display()))?;

        let mut modified_content = content;

        // Apply each fix
        for violation in file_violations {
            let fix = violation.fix.as_ref().unwrap();

            // Replace the range [start_byte, end_byte) with replacement
            let mut new_content = Vec::new();
            new_content.extend_from_slice(&modified_content[..fix.start_byte]);
            new_content.extend_from_slice(fix.replacement.as_bytes());
            new_content.extend_from_slice(&modified_content[fix.end_byte..]);

            modified_content = new_content;
            total_fixed += 1;
        }

        // Write back to file
        fs::write(file_path, modified_content)
            .with_context(|| format!("Failed to write file: {}", file_path.display()))?;
    }

    Ok(total_fixed)
}
