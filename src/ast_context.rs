use anyhow::Result;
use globset::GlobSet;
use gobject_ast::{FunctionInfo, Parser, Project};
use indicatif::ProgressBar;
use std::path::Path;
use walkdir::WalkDir;

/// AST-based project context that replaces the old tree-sitter based ProjectContext
pub struct AstContext {
    pub project: Project,
}

impl AstContext {
    /// Build with ignore patterns
    pub fn build_with_ignore(
        directory: &Path,
        ignore_matcher: &GlobSet,
        spinner: Option<&ProgressBar>,
    ) -> Result<Self> {
        let mut parser = Parser::new()?;
        let mut project = Project::new();

        // Collect all files first to get count
        let files: Vec<_> = WalkDir::new(directory)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .is_some_and(|ext| ext == "h" || ext == "c")
            })
            .filter(|e| {
                let path = e.path();
                let relative_path = path.strip_prefix(directory).unwrap_or(path);
                !ignore_matcher.is_match(relative_path)
            })
            .collect();

        let total_files = files.len();

        // Parse each file
        for (i, entry) in files.iter().enumerate() {
            let path = entry.path();

            if let Some(sp) = spinner {
                sp.set_message(format!("Parsing files... {}/{}", i + 1, total_files));
            }

            // Parse this file
            if let Ok(file_project) = parser.parse_file(path) {
                // Merge into main project
                for (file_path, file_model) in file_project.files {
                    project.files.insert(file_path, file_model);
                }
            }
        }

        Ok(Self { project })
    }

    /// Find functions declared in headers that have no implementation
    /// Returns (file_path, function_info) tuples
    pub fn find_declared_but_not_defined(&self) -> Vec<(&Path, &FunctionInfo)> {
        self.project
            .files
            .iter()
            .filter(|(path, _)| path.extension().is_some_and(|ext| ext == "h"))
            .flat_map(|(path, file)| {
                file.functions
                    .iter()
                    .filter(|f| !f.is_definition)
                    .filter(|f| {
                        // Check if there's a matching definition in any C file
                        !self
                            .project
                            .files
                            .iter()
                            .filter(|(p, _)| p.extension().is_some_and(|ext| ext == "c"))
                            .flat_map(|(_, file)| &file.functions)
                            .any(|def| def.name == f.name && def.is_definition)
                    })
                    .map(move |f| (path.as_path(), f))
            })
            .collect()
    }

    /// Get the source text for an entire function
    pub fn get_function_source<'a>(
        &'a self,
        file_path: &Path,
        func: &FunctionInfo,
    ) -> Option<&'a [u8]> {
        let file = self.project.files.get(file_path)?;

        if let (Some(start), Some(end)) = (func.start_byte, func.end_byte) {
            Some(&file.source[start..end])
        } else {
            None
        }
    }
}
