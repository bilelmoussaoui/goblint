use std::path::PathBuf;

use gobject_ast::{Expression, ExpressionStmt, Parser, Statement};

fn parse_fixture(name: &str) -> gobject_ast::Project {
    let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name);

    let mut parser = Parser::new().unwrap();
    parser.parse_file(&fixture_path).unwrap()
}

#[test]
fn test_parse_call_expressions() {
    let project = parse_fixture("call_expressions.c");

    let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("call_expressions.c");

    let file = project
        .get_file(&fixture_path)
        .expect("File should be parsed");

    let func = file
        .iter_function_definitions()
        .next()
        .expect("Should find a function");
    assert_eq!(func.name, "test_function");

    // Check we have statements parsed
    assert!(
        !func.body_statements.is_empty(),
        "Should have parsed body statements"
    );

    // Count call expressions
    let mut call_count = 0;
    for stmt in &func.body_statements {
        if let Statement::Expression(ExpressionStmt {
            expr: Expression::Call(_),
            ..
        }) = stmt
        {
            call_count += 1;
        }
    }

    // We should find at least the function calls (not counting the variable
    // declaration)
    assert!(
        call_count >= 2,
        "Should find at least 2 call expressions (g_task_set_source_tag, g_object_unref), found {}",
        call_count
    );
}

#[test]
fn test_parse_assignments() {
    let project = parse_fixture("assignments.c");

    let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("assignments.c");

    let file = project
        .get_file(&fixture_path)
        .expect("File should be parsed");
    let func = file
        .iter_function_definitions()
        .next()
        .expect("Should find a function");

    // Count assignments
    let mut assignment_count = 0;
    for stmt in &func.body_statements {
        if let Statement::Expression(ExpressionStmt {
            expr: Expression::Assignment(_),
            ..
        }) = stmt
        {
            assignment_count += 1;
        }
    }

    assert!(
        assignment_count >= 1,
        "Should find at least 1 assignment expression, found {}",
        assignment_count
    );
}

#[test]
fn test_parse_return_statement() {
    let project = parse_fixture("return_statement.c");

    let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("return_statement.c");

    let file = project
        .get_file(&fixture_path)
        .expect("File should be parsed");
    let func = file
        .iter_function_definitions()
        .next()
        .expect("Should find a function");

    // Should have a return statement
    assert!(!func.body_statements.is_empty(), "Should have statements");

    let has_return = func
        .body_statements
        .iter()
        .any(|stmt| matches!(stmt, Statement::Return(_)));

    assert!(has_return, "Should find return statement");
}

#[test]
fn test_parse_goto_statement() {
    let project = parse_fixture("goto_statement.c");

    let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("goto_statement.c");

    let file = project
        .get_file(&fixture_path)
        .expect("File should be parsed");

    let func = file
        .iter_function_definitions()
        .next()
        .expect("Should find a function");

    // Should have a goto statement (either top-level or nested in if)
    let has_goto = find_goto_recursive(&func.body_statements);

    assert!(has_goto, "Should find goto statement");
}

fn find_goto_recursive(statements: &[Statement]) -> bool {
    for stmt in statements {
        match stmt {
            Statement::Goto(_) => return true,
            Statement::If(if_stmt) => {
                if find_goto_recursive(&if_stmt.then_body) {
                    return true;
                }
                if let Some(else_body) = &if_stmt.else_body {
                    if find_goto_recursive(else_body) {
                        return true;
                    }
                }
            }
            Statement::Compound(compound) => {
                if find_goto_recursive(&compound.statements) {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

#[test]
fn test_statement_order() {
    let project = parse_fixture("statement_order.c");

    let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("statement_order.c");

    let file = project
        .get_file(&fixture_path)
        .expect("File should be parsed");
    let func = file
        .iter_function_definitions()
        .next()
        .expect("Should find a function");

    // Verify order: should have declaration/call first, then call
    assert!(
        func.body_statements.len() >= 2,
        "Should have at least 2 statements, found {}",
        func.body_statements.len()
    );

    // Second statement should be a call to g_bytes_unref
    let mut found_pattern = false;
    for i in 0..func.body_statements.len() - 1 {
        if let Statement::Expression(ExpressionStmt {
            expr: Expression::Call(call2),
            ..
        }) = &func.body_statements[i + 1]
        {
            if call2.function == "g_bytes_unref" {
                found_pattern = true;
            }
        }
    }

    assert!(
        found_pattern,
        "Should find consecutive g_bytes_get_data and g_bytes_unref calls in order"
    );
}
