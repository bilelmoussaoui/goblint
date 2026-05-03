use std::collections::{HashMap, HashSet};

use gobject_ast::model::{
    TypeInfo,
    expression::Expression,
    top_level::{PreprocessorDirective, TopLevelItem, TypeDefItem},
};

use crate::{
    ast_context::AstContext,
    config::Config,
    rules::{Category, Rule, Violation},
};

pub struct DeadCode;

impl Rule for DeadCode {
    fn name(&self) -> &'static str {
        "dead_code"
    }

    fn description(&self) -> &'static str {
        "Detect unused internal functions and types"
    }

    fn long_description(&self) -> Option<&'static str> {
        Some(
            "Detects internal functions and types that are never used anywhere in the codebase. \
             For functions: tracks both direct calls and function pointer usage (e.g., callbacks). \
             For types: tracks usage in variable declarations, casts, sizeof, and GObject macros. \
             Only reports items in private headers (not installed by meson) and static functions/types \
             defined in .c files.",
        )
    }

    fn category(&self) -> Category {
        Category::Suspicious
    }

    fn requires_meson(&self) -> bool {
        true
    }

    fn check_all(
        &self,
        ast_context: &AstContext,
        _config: &Config,
        violations: &mut Vec<Violation>,
    ) {
        // Skip if we don't have public/private distinction
        if !ast_context.has_public_private_info() {
            return;
        }

        // ── Step 1: Collect all function declarations and definitions ───────────

        let mut function_definitions: HashMap<
            String,
            Vec<(&std::path::Path, bool, gobject_ast::SourceLocation)>,
        > = HashMap::new();
        let mut function_declarations: HashMap<
            String,
            Vec<(&std::path::Path, gobject_ast::SourceLocation)>,
        > = HashMap::new();

        for (path, file) in ast_context.iter_c_files() {
            for func in file.iter_function_definitions() {
                function_definitions
                    .entry(func.name.clone())
                    .or_default()
                    .push((path, func.is_static, func.location));
            }
        }

        for (path, file) in ast_context.iter_header_files() {
            for func in file.iter_function_declarations() {
                function_declarations
                    .entry(func.name.clone())
                    .or_default()
                    .push((path, func.location));
            }
        }

        // ── Step 1b: Collect type definitions from private contexts ─────────────

        // name → [(file_path, location)]
        let mut type_definitions: HashMap<
            String,
            Vec<(&std::path::Path, gobject_ast::SourceLocation)>,
        > = HashMap::new();

        // For forward typedef aliases (`typedef struct _Foo Foo;`), map the
        // typedef name to its underlying tag so we can consider the typedef
        // referenced whenever the tag is referenced (code often uses the tag
        // directly rather than the alias).
        let mut typedef_to_tag: HashMap<String, String> = HashMap::new();
        // Reverse map: bare tag name → typedef alias name.  Used to consider
        // `struct _Foo` (keyed as "_Foo") referenced when `Foo` appears in code.
        let mut tag_to_typedef: HashMap<String, String> = HashMap::new();

        for (path, file) in ast_context.iter_c_files() {
            collect_type_defs_from_items(
                &file.top_level_items,
                path,
                &mut type_definitions,
                &mut typedef_to_tag,
                &mut tag_to_typedef,
            );
        }

        for (path, file) in ast_context.iter_header_files() {
            // Skip public headers — types there are part of the public API
            if ast_context.is_public_header(path) == Some(true) {
                continue;
            }
            collect_type_defs_from_items(
                &file.top_level_items,
                path,
                &mut type_definitions,
                &mut typedef_to_tag,
                &mut tag_to_typedef,
            );
        }

        // ── Step 2: Collect all function and type references ───────────────────

        let mut function_references: HashSet<String> = HashSet::new();
        let mut type_references: HashSet<String> = HashSet::new();

        for (_path, file) in ast_context.iter_all_files() {
            // Scan function bodies
            for func in file.iter_function_definitions() {
                // Function parameters and return type
                collect_type_ref(&func.return_type, &mut type_references);
                for param in &func.parameters {
                    collect_type_ref(&param.type_info, &mut type_references);
                }

                for stmt in &func.body_statements {
                    collect_function_references(stmt, &mut function_references);
                    collect_type_refs_from_stmt(stmt, &mut type_references);
                }
            }

            // Function declarations: return type and parameters reference types
            for func in file.iter_function_declarations() {
                collect_type_ref(&func.return_type, &mut type_references);
                // Note: FunctionDeclItem doesn't store parameters in the AST,
                // so we only have the return type here.
            }

            // Top-level declarations and preprocessor directives
            for item in &file.top_level_items {
                collect_function_references_from_top_level_item(item, &mut function_references);
                collect_type_refs_from_top_level_item(item, &mut type_references);
            }

            // GObject type registration: mark implicitly referenced functions/types
            for gobject_type in file.iter_all_gobject_types() {
                use gobject_ast::model::types::GObjectTypeKind;

                if gobject_type.is_interface() {
                    function_references.insert(gobject_type.default_init_function_name());
                } else {
                    function_references.insert(gobject_type.class_init_function_name());
                    function_references.insert(gobject_type.init_function_name());
                }

                for interface_impl in &gobject_type.interfaces {
                    function_references.insert(interface_impl.init_function.clone());
                }

                match &gobject_type.kind {
                    GObjectTypeKind::DefineBoxedType {
                        copy_func,
                        free_func,
                        ..
                    }
                    | GObjectTypeKind::DefineBoxedTypeWithCode {
                        copy_func,
                        free_func,
                        ..
                    } => {
                        function_references.insert(copy_func.clone());
                        function_references.insert(free_func.clone());
                    }
                    _ => {}
                }

                // *_WITH_PRIVATE variants implicitly use {TypeName}Private
                if gobject_type.has_private
                    || matches!(
                        gobject_type.kind,
                        GObjectTypeKind::DefineTypeWithPrivate { .. }
                            | GObjectTypeKind::DefineFinalTypeWithPrivate { .. }
                            | GObjectTypeKind::DefineAbstractTypeWithPrivate { .. }
                    )
                {
                    let priv_name = format!("{}Private", gobject_type.type_name);
                    type_references.insert(priv_name.clone());
                    // Also mark the underscore-prefixed tag form (e.g.
                    // `struct _ShellGLSLEffectPrivate` forward-declared before
                    // G_DEFINE_TYPE_WITH_PRIVATE).
                    type_references.insert(format!("_{priv_name}"));
                }

                // GObject private structs are used implicitly by the type
                // machinery — never flag them as dead code.
                //   _TypeName          — instance struct (all types)
                //   _TypeNameClass     — class vtable (derivable/abstract)
                //   _TypeNameInterface — interface vtable (interfaces only)
                let tn = &gobject_type.type_name;
                type_references.insert(format!("_{tn}"));
                if gobject_type.is_interface() {
                    type_references.insert(format!("_{tn}Interface"));
                } else if !matches!(
                    gobject_type.kind,
                    GObjectTypeKind::DefineBoxedType { .. }
                        | GObjectTypeKind::DefineBoxedTypeWithCode { .. }
                ) {
                    type_references.insert(format!("_{tn}Class"));
                }

                for stmt in &gobject_type.code_block_statements {
                    collect_function_references(stmt, &mut function_references);
                    collect_type_refs_from_stmt(stmt, &mut type_references);
                }
            }

            // Preprocessor directives (autoptr cleanup, #define bodies).
            // scan_preprocessor_items recurses into #ifdef/#if blocks so that
            // #define macros inside conditional sections are also checked.
            scan_preprocessor_items(
                &file.top_level_items,
                &mut function_references,
                &mut type_references,
            );
        }

        // ── Step 3: Report function violations ─────────────────────────────────

        for (func_name, defs) in &function_definitions {
            if function_references.contains(func_name) {
                continue;
            }

            for (def_path, is_static, location) in defs {
                if *is_static {
                    violations.push(self.violation(
                        def_path,
                        location.line,
                        location.column,
                        format!("Static function '{}' is never used", func_name),
                    ));
                    continue;
                }

                if let Some(decls) = function_declarations.get(func_name) {
                    for (decl_path, decl_location) in decls {
                        if ast_context.is_public_header(decl_path) == Some(true) {
                            continue;
                        }

                        violations.push(self.violation(
                            decl_path,
                            decl_location.line,
                            decl_location.column,
                            format!(
                                "Internal function '{}' is never used (declared in private header)",
                                func_name
                            ),
                        ));
                    }
                }
            }
        }

        // Declared-but-not-defined functions in private headers
        for (func_name, decls) in &function_declarations {
            if function_references.contains(func_name) {
                continue;
            }
            if function_definitions.contains_key(func_name) {
                continue;
            }

            for (decl_path, decl_location) in decls {
                if ast_context.is_public_header(decl_path) == Some(true) {
                    continue;
                }

                violations.push(self.violation(
                    decl_path,
                    decl_location.line,
                    decl_location.column,
                    format!(
                        "Internal function '{}' is never used (declared but not defined)",
                        func_name
                    ),
                ));
            }
        }

        // ── Step 4: Report type violations ─────────────────────────────────────

        for (type_name, defs) in &type_definitions {
            if type_references.contains(type_name) {
                continue;
            }
            // For forward typedef aliases (`typedef struct _Foo Foo`), also
            // consider the typedef referenced if the underlying tag is used.
            if typedef_to_tag
                .get(type_name)
                .is_some_and(|tag| type_references.contains(tag))
            {
                continue;
            }
            // Reverse: if `_Foo` is a tag with typedef alias `Foo`, consider
            // the tag referenced whenever the alias appears in code.  This
            // covers the common pattern where a union/struct is forward-declared
            // as `typedef union _Foo Foo;` and then defined as `union _Foo {...}`
            // but code only ever spells `Foo`, not `_Foo`.
            if tag_to_typedef
                .get(type_name)
                .is_some_and(|alias| type_references.contains(alias))
            {
                continue;
            }

            for (def_path, location) in defs {
                violations.push(self.violation(
                    def_path,
                    location.line,
                    location.column,
                    format!("Type '{}' is defined but never used", type_name),
                ));
            }
        }
    }
}

// ── Type definition collection
// ─────────────────────────────────────────────────

fn collect_type_defs_from_items<'a>(
    items: &'a [TopLevelItem],
    path: &'a std::path::Path,
    defs: &mut HashMap<String, Vec<(&'a std::path::Path, gobject_ast::SourceLocation)>>,
    typedef_to_tag: &mut HashMap<String, String>,
    tag_to_typedef: &mut HashMap<String, String>,
) {
    for item in items {
        match item {
            TopLevelItem::TypeDefinition(type_def) => match type_def {
                TypeDefItem::Struct {
                    name,
                    has_body: true,
                    location,
                    ..
                } => {
                    defs.entry(name.clone())
                        .or_default()
                        .push((path, *location));
                }
                TypeDefItem::Typedef {
                    name,
                    target_type,
                    tag_name,
                    struct_fields,
                    location,
                } => {
                    defs.entry(name.clone())
                        .or_default()
                        .push((path, *location));
                    // Forward typedef aliases: `typedef struct _Foo Foo`.
                    // typedef_to_tag: Foo → "struct _Foo" (full text, used in
                    // the existing check against type_references).
                    // tag_to_typedef: _Foo → Foo (clean tag name from the AST,
                    // used when the struct definition is keyed as "_Foo").
                    if struct_fields.is_empty() && !target_type.base_type.is_empty() {
                        typedef_to_tag.insert(name.clone(), target_type.base_type.clone());
                    }
                    if let Some(tag) = tag_name {
                        tag_to_typedef.insert(tag.clone(), name.clone());
                    }
                }
                _ => {}
            },
            // Recurse into preprocessor conditional/decls-block bodies
            TopLevelItem::Preprocessor(
                PreprocessorDirective::Conditional { body, .. }
                | PreprocessorDirective::GObjectDeclsBlock { body, .. },
            ) => {
                collect_type_defs_from_items(body, path, defs, typedef_to_tag, tag_to_typedef);
            }
            _ => {}
        }
    }
}

/// Recursively scan preprocessor directives for function/type references.
/// Handles autoptr cleanup, #define bodies, and recurses into #ifdef blocks
/// so that macros defined inside conditional sections are not missed.
fn scan_preprocessor_items(
    items: &[TopLevelItem],
    function_refs: &mut HashSet<String>,
    type_refs: &mut HashSet<String>,
) {
    for item in items {
        if let TopLevelItem::Preprocessor(directive) = item {
            match directive {
                PreprocessorDirective::AutoptrCleanupFunc {
                    type_name,
                    cleanup_function,
                    ..
                } => {
                    function_refs.insert(cleanup_function.clone());
                    type_refs.insert(type_name.clone());
                }
                PreprocessorDirective::AutoCleanupClearFunc {
                    type_name,
                    cleanup_function,
                    ..
                } => {
                    function_refs.insert(cleanup_function.clone());
                    type_refs.insert(type_name.clone());
                }
                PreprocessorDirective::Define {
                    value: Some(value), ..
                } => {
                    extract_function_calls_from_text(value, function_refs);
                }
                PreprocessorDirective::Conditional { body, .. }
                | PreprocessorDirective::GObjectDeclsBlock { body, .. } => {
                    scan_preprocessor_items(body, function_refs, type_refs);
                }
                _ => {}
            }
        }
    }
}

// ── Type reference collection
// ──────────────────────────────────────────────────

fn collect_type_ref(type_info: &TypeInfo, refs: &mut HashSet<String>) {
    if !type_info.base_type.is_empty() {
        refs.insert(type_info.base_type.clone());
    }
    if let Some(auto) = &type_info.auto_cleanup
        && let Some(arg) = auto.type_arg()
    {
        refs.insert(arg.to_string());
    }
}

fn collect_type_refs_from_expr(expr: &Expression, refs: &mut HashSet<String>) {
    expr.walk(&mut |e| match e {
        Expression::Cast(cast) => {
            collect_type_ref(&cast.type_info, refs);
        }
        Expression::Sizeof(sizeof) => {
            if let Some(name) = sizeof.type_name()
                && !name.is_empty()
            {
                refs.insert(name);
            }
        }
        _ => {}
    });
}

fn collect_type_refs_from_stmt(
    stmt: &gobject_ast::model::statement::Statement,
    refs: &mut HashSet<String>,
) {
    use gobject_ast::model::statement::Statement;

    stmt.walk(&mut |s| {
        if let Statement::Declaration(decl) = s {
            collect_type_ref(&decl.type_info, refs);
        }
    });

    stmt.walk_expressions(&mut |expr| {
        collect_type_refs_from_expr(expr, refs);
    });
}

fn collect_type_refs_from_top_level_item(item: &TopLevelItem, refs: &mut HashSet<String>) {
    match item {
        TopLevelItem::Declaration(stmt) => {
            collect_type_refs_from_stmt(stmt, refs);
        }
        // Typedef: its target type name is a reference, and if it wraps a struct body
        // its field types are also references.
        TopLevelItem::TypeDefinition(TypeDefItem::Typedef {
            target_type,
            struct_fields,
            ..
        }) => {
            if !target_type.base_type.is_empty() {
                refs.insert(target_type.base_type.clone());
            }
            for field in struct_fields {
                if !field.field_type.base_type.is_empty() {
                    refs.insert(field.field_type.base_type.clone());
                }
            }
        }
        // Standalone struct definition: `struct _Foo { FieldType f; };`
        TopLevelItem::TypeDefinition(TypeDefItem::Struct { fields, .. }) => {
            for field in fields {
                if !field.field_type.base_type.is_empty() {
                    refs.insert(field.field_type.base_type.clone());
                }
            }
        }
        TopLevelItem::Preprocessor(
            PreprocessorDirective::Conditional { body, .. }
            | PreprocessorDirective::GObjectDeclsBlock { body, .. },
        ) => {
            for body_item in body {
                collect_type_refs_from_top_level_item(body_item, refs);
            }
        }
        _ => {}
    }
}

// ── Function reference collection (unchanged from dead_code_functions)
// ─────────

fn collect_function_references_from_top_level_item(
    item: &TopLevelItem,
    refs: &mut HashSet<String>,
) {
    match item {
        TopLevelItem::Declaration(decl) => {
            collect_function_references(decl, refs);
        }
        TopLevelItem::Preprocessor(
            PreprocessorDirective::Conditional { body, .. }
            | PreprocessorDirective::GObjectDeclsBlock { body, .. },
        ) => {
            for body_item in body {
                collect_function_references_from_top_level_item(body_item, refs);
            }
        }
        _ => {}
    }
}

fn extract_function_calls_from_text(text: &str, refs: &mut HashSet<String>) {
    let mut chars = text.chars().peekable();
    let mut current_identifier = String::new();

    while let Some(c) = chars.next() {
        if c.is_alphanumeric() || c == '_' {
            current_identifier.push(c);
        } else {
            if !current_identifier.is_empty() {
                if c == '(' {
                    refs.insert(current_identifier.clone());
                } else if c.is_whitespace() || c == '\\' {
                    // Skip whitespace and backslash line-continuations, then
                    // check whether '(' follows (macro call on next line).
                    let mut temp_chars = chars.clone();
                    while let Some(&next_c) = temp_chars.peek() {
                        if next_c.is_whitespace() || next_c == '\\' {
                            temp_chars.next();
                        } else {
                            if next_c == '(' {
                                refs.insert(current_identifier.clone());
                            }
                            break;
                        }
                    }
                }
                current_identifier.clear();
            }
        }
    }
}

fn collect_function_references(
    stmt: &gobject_ast::model::statement::Statement,
    refs: &mut HashSet<String>,
) {
    use gobject_ast::model::statement::Statement;

    match stmt {
        Statement::Expression(expr_stmt) => {
            collect_function_references_from_expr(&expr_stmt.expr, refs);
        }
        Statement::Compound(compound) => {
            for stmt in &compound.statements {
                collect_function_references(stmt, refs);
            }
        }
        Statement::If(if_stmt) => {
            collect_function_references_from_expr(&if_stmt.condition, refs);
            for stmt in &if_stmt.then_body {
                collect_function_references(stmt, refs);
            }
            if let Some(else_body) = &if_stmt.else_body {
                for stmt in else_body {
                    collect_function_references(stmt, refs);
                }
            }
        }
        Statement::Return(ret) => {
            if let Some(expr) = &ret.value {
                collect_function_references_from_expr(expr, refs);
            }
        }
        Statement::Declaration(decl) => {
            if let Some(init) = &decl.initializer {
                collect_function_references_from_expr(init, refs);
            }
        }
        Statement::Switch(switch) => {
            collect_function_references_from_expr(&switch.condition, refs);
            for case in &switch.cases {
                for stmt in &case.body {
                    collect_function_references(stmt, refs);
                }
            }
        }
        Statement::For(for_stmt) => {
            if let Some(init) = &for_stmt.initializer {
                collect_function_references_from_expr(init, refs);
            }
            if let Some(cond) = &for_stmt.condition {
                collect_function_references_from_expr(cond, refs);
            }
            if let Some(update) = &for_stmt.update {
                collect_function_references_from_expr(update, refs);
            }
            for stmt in &for_stmt.body {
                collect_function_references(stmt, refs);
            }
        }
        Statement::While(while_stmt) => {
            collect_function_references_from_expr(&while_stmt.condition, refs);
            for stmt in &while_stmt.body {
                collect_function_references(stmt, refs);
            }
        }
        Statement::DoWhile(do_while) => {
            for stmt in &do_while.body {
                collect_function_references(stmt, refs);
            }
            collect_function_references_from_expr(&do_while.condition, refs);
        }
        Statement::Labeled(labeled) => {
            collect_function_references(&labeled.statement, refs);
        }
        Statement::Preprocessor(directive) => {
            if let PreprocessorDirective::Define {
                value: Some(value), ..
            } = directive
            {
                extract_function_calls_from_text(value, refs);
            }
        }
        Statement::Break(_) | Statement::Continue(_) | Statement::Goto(_) => {}
    }
}

fn collect_function_references_from_expr(
    expr: &gobject_ast::model::expression::Expression,
    refs: &mut HashSet<String>,
) {
    use gobject_ast::model::expression::{Argument, Expression};

    match expr {
        Expression::Call(call_expr) => {
            if let Expression::Identifier(id) = &*call_expr.function {
                refs.insert(id.name.clone());
            } else {
                collect_function_references_from_expr(&call_expr.function, refs);
            }
            for arg in &call_expr.arguments {
                let Argument::Expression(e) = arg;
                collect_function_references_from_expr(e, refs);
            }
        }
        Expression::MacroCall(macro_call) => {
            for arg in &macro_call.arguments {
                let Argument::Expression(e) = arg;
                collect_function_references_from_expr(e, refs);
            }
        }
        Expression::Binary(bin) => {
            collect_function_references_from_expr(&bin.left, refs);
            collect_function_references_from_expr(&bin.right, refs);
        }
        Expression::Unary(un) => {
            collect_function_references_from_expr(&un.operand, refs);
        }
        Expression::Conditional(cond) => {
            collect_function_references_from_expr(&cond.condition, refs);
            collect_function_references_from_expr(&cond.then_expr, refs);
            collect_function_references_from_expr(&cond.else_expr, refs);
        }
        Expression::Assignment(assign) => {
            collect_function_references_from_expr(&assign.lhs, refs);
            collect_function_references_from_expr(&assign.rhs, refs);
        }
        Expression::Update(update) => {
            collect_function_references_from_expr(&update.operand, refs);
        }
        Expression::Cast(cast) => {
            collect_function_references_from_expr(&cast.operand, refs);
        }
        Expression::Subscript(subscript) => {
            collect_function_references_from_expr(&subscript.array, refs);
            collect_function_references_from_expr(&subscript.index, refs);
        }
        Expression::Identifier(id) => {
            refs.insert(id.name.clone());
        }
        Expression::InitializerList(init) => {
            for item in &init.items {
                collect_function_references_from_expr(&item.value, refs);
            }
        }
        Expression::FieldAccess(field) => {
            collect_function_references_from_expr(&field.base, refs);
        }
        Expression::NumberLiteral(_)
        | Expression::StringLiteral(_)
        | Expression::CharLiteral(_)
        | Expression::Boolean(_)
        | Expression::Null(_)
        | Expression::Sizeof(_)
        | Expression::Comment(_)
        | Expression::Generic(_) => {}
    }
}
