use std::{env, path::PathBuf};

/// Example showing how to use gobject-ast for linting
/// Run with: cargo run --example ast_lint_example -- /path/to/code
use anyhow::Result;
use gobject_ast::Parser;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <directory>", args[0]);
        std::process::exit(1);
    }

    let directory = PathBuf::from(&args[1]);

    println!("Parsing directory: {}", directory.display());
    let mut parser = Parser::new()?;
    let project = parser.parse_directory(&directory)?;

    println!("\n=== PROJECT SUMMARY ===");
    println!("Files: {}", project.files.len());

    let total_functions: usize = project.files.values().map(|f| f.functions.len()).sum();
    println!("Functions: {}", total_functions);

    let total_gobject_types: usize = project.files.values().map(|f| f.gobject_types.len()).sum();
    println!("GObject Types: {}", total_gobject_types);

    println!("\n=== LINT CHECKS ===");

    // Example Check 1: Find functions that should be static
    println!("\n1. Functions that should be static:");
    let mut count = 0;
    for (path, file) in &project.files {
        // Only check .c files
        if path.extension().is_some_and(|ext| ext != "c") {
            continue;
        }

        for func in &file.functions {
            // Skip if already static
            if func.is_static {
                continue;
            }

            // Skip if has export macros
            if !func.export_macros.is_empty() {
                continue;
            }

            // Check if declared in any header
            let is_in_header = project
                .files
                .iter()
                .filter(|(p, _)| p.extension().is_some_and(|ext| ext == "h"))
                .any(|(_, h)| h.functions.iter().any(|f| f.name == func.name));

            if !is_in_header {
                count += 1;
                println!("   {}:{} - {}", path.display(), func.line, func.name);
            }
        }
    }
    println!("   Found: {} issues", count);

    // Example Check 2: Find functions declared but not defined
    println!("\n2. Functions declared in headers but not implemented:");
    count = 0;
    for (header_path, header) in &project.files {
        if header_path.extension().is_some_and(|ext| ext != "h") {
            continue;
        }

        for decl in &header.functions {
            if decl.is_definition {
                continue; // Skip if it's a definition (inline function)
            }

            // Look for implementation in .c files
            let has_impl = project
                .files
                .iter()
                .filter(|(p, _)| p.extension().is_some_and(|ext| ext == "c"))
                .any(|(_, file)| {
                    file.functions
                        .iter()
                        .any(|f| f.name == decl.name && f.is_definition)
                });

            if !has_impl {
                count += 1;
                println!("   {}:{} - {}", header_path.display(), decl.line, decl.name);
            }
        }
    }
    println!("   Found: {} issues", count);

    // Example Check 3: Find public functions without export macros
    println!("\n3. Public functions missing export macros:");
    count = 0;
    for (header_path, header) in &project.files {
        if header_path.extension().is_some_and(|ext| ext != "h") {
            continue;
        }

        for func in &header.functions {
            // Skip GObject-generated functions
            if func.name.ends_with("_get_type") || func.name.ends_with("_error_quark") {
                continue;
            }

            if func.export_macros.is_empty() {
                count += 1;
                println!(
                    "   {}:{} - {} (should have export macro)",
                    header_path.display(),
                    func.line,
                    func.name
                );
            }
        }
    }
    println!("   Found: {} issues", count);

    // Example Check 4: Show GObject type information
    println!("\n4. GObject types with class structs:");
    for (path, file) in &project.files {
        for gtype in &file.gobject_types {
            if let Some(ref class_struct) = gtype.class_struct {
                println!(
                    "   {}:{} - {} ({})",
                    path.display(),
                    gtype.line,
                    gtype.type_name,
                    class_struct.name
                );
                println!("      {} virtual functions", class_struct.vfuncs.len());
                for vfunc in &class_struct.vfuncs {
                    let ret = vfunc.return_type.as_deref().unwrap_or("void");
                    println!("        {} (*{})()", ret, vfunc.name);
                }
            }
        }
    }

    Ok(())
}
