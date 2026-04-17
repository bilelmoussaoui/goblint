use std::{
    path::Path,
    sync::atomic::{AtomicUsize, Ordering},
};

use anyhow::Result;
use globset::GlobSet;
use gobject_ast::{Parser, Project};
use ignore::WalkBuilder;
use indicatif::ProgressBar;
use rayon::prelude::*;

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
        // WalkBuilder respects .gitignore, .ignore, and other ignore files
        // automatically
        let files: Vec<_> = WalkBuilder::new(directory)
            .hidden(false) // Include hidden files/dirs
            .git_ignore(true) // Respect .gitignore
            .git_global(true) // Respect global gitignore
            .git_exclude(true) // Respect .git/info/exclude
            .require_git(false) // Work in non-git directories too
            .build()
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
