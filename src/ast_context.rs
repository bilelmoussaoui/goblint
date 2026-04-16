use std::{
    cell::RefCell,
    path::Path,
    sync::atomic::{AtomicUsize, Ordering},
};

thread_local! {
    static TS_PARSER: RefCell<tree_sitter::Parser> = RefCell::new({
        let mut p = tree_sitter::Parser::new();
        p.set_language(&tree_sitter_c::LANGUAGE.into())
            .expect("Failed to load C grammar");
        p
    });
}

use anyhow::Result;
use globset::GlobSet;
use gobject_ast::{FunctionInfo, Parser, Project};
use indicatif::ProgressBar;
use rayon::prelude::*;
use walkdir::WalkDir;

/// AST-based project context that replaces the old tree-sitter based
/// ProjectContext
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
        let counter = AtomicUsize::new(0);

        // Parse files in parallel — each rayon thread gets its own Parser
        let parsed: Vec<_> = files
            .par_iter()
            .filter_map(|entry| {
                let i = counter.fetch_add(1, Ordering::Relaxed);
                if let Some(sp) = spinner {
                    sp.set_message(format!("Parsing files... {}/{}", i + 1, total_files));
                }
                let mut parser = Parser::new().ok()?;
                parser.parse_file(entry.path()).ok()
            })
            .collect();

        let mut project = Project::new();
        for file_project in parsed {
            project.files.extend(file_project.files);
        }

        Ok(Self { project })
    }

    /// Update a single file in the project
    pub fn update_file(&mut self, file_path: &Path) -> Result<()> {
        let mut parser = Parser::new()?;

        // Parse the file
        if let Ok(file_project) = parser.parse_file(file_path) {
            // Update or insert the file in the project
            for (path, file_model) in file_project.files {
                self.project.files.insert(path, file_model);
            }
        } else {
            // If parsing failed, remove the file from the project
            self.project.files.remove(file_path);
        }

        Ok(())
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

    /// Iterate over all files in the project
    pub fn iter_all_files(&self) -> impl Iterator<Item = (&Path, &gobject_ast::FileModel)> {
        self.project
            .files
            .iter()
            .map(|(path, file)| (path.as_path(), file))
    }

    /// Iterate over all C files (extension .c) in the project
    pub fn iter_c_files(&self) -> impl Iterator<Item = (&Path, &gobject_ast::FileModel)> {
        self.project
            .files
            .iter()
            .filter(|(path, _)| path.extension().is_some_and(|ext| ext == "c"))
            .map(|(path, file)| (path.as_path(), file))
    }

    /// Iterate over all header files (extension .h) in the project
    pub fn iter_header_files(&self) -> impl Iterator<Item = (&Path, &gobject_ast::FileModel)> {
        self.project
            .files
            .iter()
            .filter(|(path, _)| path.extension().is_some_and(|ext| ext == "h"))
            .map(|(path, file)| (path.as_path(), file))
    }
}
