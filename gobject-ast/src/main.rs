use std::{env, path::PathBuf};

use anyhow::Result;
use gobject_ast::{Parser, Project};

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: gobject-ast <file.c|file.h|directory>");
        eprintln!("\nParse a C/header file or directory and print the AST model");
        std::process::exit(1);
    }

    let path = PathBuf::from(&args[1]);
    let mut parser = Parser::new()?;

    let project = if path.is_dir() {
        eprintln!("Parsing directory: {}", path.display());
        parser.parse_directory(&path)?
    } else {
        eprintln!("Parsing file: {}", path.display());
        parser.parse_file(&path)?
    };

    print_project(&project);

    Ok(())
}

fn print_project(project: &Project) {
    println!("\n=== FILES ({}) ===\n", project.files.len());

    let mut sorted_files: Vec<_> = project.files.iter().collect();
    sorted_files.sort_by_key(|(path, _)| *path);

    for (path, file) in sorted_files {
        println!("{}:", path.display());

        // Print top-level items
        println!("  Top-level items ({}):", file.top_level_items.len());
        println!("{:#?}", file.top_level_items);

        println!();
    }
}

// OLD PRINTING CODE - COMMENTED OUT AFTER REFACTORING TO TREE STRUCTURE
// if !file.includes.is_empty() {
// println!("  Includes ({}):", file.includes.len());
// for inc in &file.includes {
// let bracket = if inc.is_system { "<>" } else { "\"\"" };
// println!(
// "    {}{}{} (line {})",
// bracket.chars().next().unwrap(),
// inc.path,
// bracket.chars().nth(1).unwrap(),
// inc.location.line
// );
// }
// }
//
// if !file.typedefs.is_empty() {
// println!("  Typedefs ({}):", file.typedefs.len());
// for td in &file.typedefs {
// println!(
// "    typedef {} {} (line {})",
// td.target_type, td.name, td.location.line
// );
// }
// }
//
// if !file.structs.is_empty() {
// println!("  Structs ({}):", file.structs.len());
// for s in &file.structs {
// println!(
// "    struct {} {} (line {})",
// s.name,
// if s.is_opaque { "[opaque]" } else { "" },
// s.location.line
// );
// }
// }
//
// if !file.enums.is_empty() {
// println!("  Enums ({}):", file.enums.len());
// for e in &file.enums {
// println!("    enum {} (line {})", e.name, e.location.line);
// }
// }
//
// if !file.gobject_types.is_empty() {
// println!("  GObject Types ({}):", file.gobject_types.len());
// for gt in &file.gobject_types {
// let kind_str = match &gt.kind {
// gobject_ast::GObjectTypeKind::DeclareFinal { parent_type, .. } => {
// format!("G_DECLARE_FINAL_TYPE (parent: {})", parent_type)
// }
// gobject_ast::GObjectTypeKind::DeclareDerivable { parent_type, .. } => {
// format!("G_DECLARE_DERIVABLE_TYPE (parent: {})", parent_type)
// }
// gobject_ast::GObjectTypeKind::DeclareInterface {
// prerequisite_type, ..
// } => format!("G_DECLARE_INTERFACE (prereq: {})", prerequisite_type),
// gobject_ast::GObjectTypeKind::DefineType { parent_type, .. } => {
// format!("G_DEFINE_TYPE (parent: {})", parent_type)
// }
// gobject_ast::GObjectTypeKind::DefineTypeWithPrivate { parent_type, .. } => {
// format!("G_DEFINE_TYPE_WITH_PRIVATE (parent: {})", parent_type)
// }
// gobject_ast::GObjectTypeKind::DefineAbstractType { parent_type, .. } => {
// format!("G_DEFINE_ABSTRACT_TYPE (parent: {})", parent_type)
// }
// gobject_ast::GObjectTypeKind::DefineTypeWithCode { parent_type, .. } => {
// format!("G_DEFINE_TYPE_WITH_CODE (parent: {})", parent_type)
// }
// gobject_ast::GObjectTypeKind::DefineFinalType { parent_type, .. } => {
// format!("G_DEFINE_FINAL_TYPE (parent: {})", parent_type)
// }
// gobject_ast::GObjectTypeKind::DefineFinalTypeWithCode {
// parent_type, ..
// } => {
// format!("G_DEFINE_FINAL_TYPE_WITH_CODE (parent: {})", parent_type)
// }
// gobject_ast::GObjectTypeKind::DefineFinalTypeWithPrivate {
// parent_type,
// ..
// } => {
// format!("G_DEFINE_FINAL_TYPE_WITH_PRIVATE (parent: {})", parent_type)
// }
// gobject_ast::GObjectTypeKind::DefineAbstractTypeWithCode {
// parent_type,
// ..
// } => {
// format!("G_DEFINE_ABSTRACT_TYPE_WITH_CODE (parent: {})", parent_type)
// }
// gobject_ast::GObjectTypeKind::DefineAbstractTypeWithPrivate {
// parent_type,
// ..
// } => {
// format!(
// "G_DEFINE_ABSTRACT_TYPE_WITH_PRIVATE (parent: {})",
// parent_type
// )
// }
// gobject_ast::GObjectTypeKind::DefineInterface {
// prerequisite_type, ..
// } => format!("G_DEFINE_INTERFACE (prereq: {})", prerequisite_type),
// gobject_ast::GObjectTypeKind::DefineInterfaceWithCode {
// prerequisite_type,
// ..
// } => format!(
// "G_DEFINE_INTERFACE_WITH_CODE (prereq: {})",
// prerequisite_type
// ),
// gobject_ast::GObjectTypeKind::DefineBoxedType { .. } => {
// "G_DEFINE_BOXED_TYPE".to_string()
// }
// gobject_ast::GObjectTypeKind::DefinePointerType { .. } => {
// "G_DEFINE_POINTER_TYPE".to_string()
// }
// };
// println!(
// "    {} - {} (line {})",
// gt.type_name, kind_str, gt.location.line
// );
//
// if !gt.interfaces.is_empty() {
// println!("      Interfaces: {}", gt.interfaces.len());
// for iface in &gt.interfaces {
// println!(
// "        {} -> {}",
// iface.interface_type, iface.init_function
// );
// }
// }
//
// if gt.has_private {
// println!("      Has private data: yes");
// }
//
// if let Some(ref class_struct) = gt.class_struct {
// println!(
// "      Class Struct: {} ({} vfuncs)",
// class_struct.name,
// class_struct.vfuncs.len()
// );
// for vfunc in &class_struct.vfuncs {
// let ret_type = vfunc.return_type.as_deref().unwrap_or("void");
// let params: Vec<String> = vfunc
// .parameters
// .iter()
// .map(|p| {
// if let Some(ref name) = p.name {
// format!("{} {}", p.type_name, name)
// } else {
// p.type_name.clone()
// }
// })
// .collect();
// println!(
// "        {} (*{}) ({})",
// ret_type,
// vfunc.name,
// params.join(", ")
// );
// }
// }
// }
// }
//
// if !file.functions.is_empty() {
// println!("  Functions ({}):", file.functions.len());
// for func in &file.functions {
// let kind = if func.is_definition { "def" } else { "decl" };
// let static_marker = if func.is_static { "static " } else { "" };
// let export = if !func.export_macros.is_empty() {
// format!("[{}] ", func.export_macros.join(", "))
// } else {
// String::new()
// };
// println!(
// "    {}{}{} {} (line {})",
// export, static_marker, kind, func.name, func.location.line
// );
// }
// }
//
// println!();
// }
//
// Summary
// let mut total_includes = 0;
// let mut total_typedefs = 0;
// let mut total_structs = 0;
// let mut total_enums = 0;
// let mut total_gobject_types = 0;
// let mut total_functions = 0;
//
// for file in project.files.values() {
// total_includes += file.includes.len();
// total_typedefs += file.typedefs.len();
// total_structs += file.structs.len();
// total_enums += file.enums.len();
// total_gobject_types += file.gobject_types.len();
// total_functions += file.functions.len();
// }
//
// println!("\n=== SUMMARY ===");
// println!("Files: {}", project.files.len());
// println!("Includes: {}", total_includes);
// println!("Typedefs: {}", total_typedefs);
// println!("Structs: {}", total_structs);
// println!("Enums: {}", total_enums);
// println!("GObject Types: {}", total_gobject_types);
// println!("Functions: {}", total_functions);
// }
