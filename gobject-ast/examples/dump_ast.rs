use std::{env, fs};

/// Dump the tree-sitter AST as JSON for debugging
/// Usage: cargo run --example dump_ast <file.c>
use anyhow::Result;
use serde_json::json;
use tree_sitter::{Node, Parser};

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <file>", args[0]);
        std::process::exit(1);
    }

    let file_path = &args[1];
    let source = fs::read(file_path)?;

    let mut parser = Parser::new();
    parser.set_language(&tree_sitter_c_gobject::LANGUAGE.into())?;

    let tree = parser.parse(&source, None).unwrap();
    let root = tree.root_node();

    // Print as JSON
    let json = node_to_json(root, &source);
    println!("{}", serde_json::to_string_pretty(&json)?);

    Ok(())
}

fn node_to_json(node: Node, source: &[u8]) -> serde_json::Value {
    let text = std::str::from_utf8(&source[node.byte_range()]).unwrap_or("");

    // For leaf nodes or small nodes, show the text
    let text_preview = if text.len() < 100 && !text.contains('\n') {
        Some(text.to_string())
    } else {
        None
    };

    let mut children = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        children.push(node_to_json(child, source));
    }

    json!({
        "kind": node.kind(),
        "start_byte": node.start_byte(),
        "end_byte": node.end_byte(),
        "start_line": node.start_position().row + 1,
        "start_column": node.start_position().column,
        "end_line": node.end_position().row + 1,
        "end_column": node.end_position().column,
        "text": text_preview,
        "child_count": node.child_count(),
        "children": if children.is_empty() { serde_json::Value::Null } else { json!(children) }
    })
}
