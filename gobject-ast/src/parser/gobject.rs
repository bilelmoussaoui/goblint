use tree_sitter::Node;

use super::Parser;
use crate::model::{
    Expression,
    types::{ClassStruct, DeclareKind, DefineKind, GObjectType, GObjectTypeKind, VirtualFunction},
};

impl Parser {
    pub(super) fn extract_gobject_from_identifier(
        &self,
        _id_node: Node,
        parent: Node,
        source: &[u8],
        macro_name: &str,
    ) -> Option<GObjectType> {
        // Recursively find all identifiers in parent node
        let mut arg_values = Vec::new();

        // Special handling for function_declarator nodes (from declarations with
        // parameter_list) These have parameter_declaration children OR direct
        // identifier children
        if parent.kind() == "function_declarator" {
            // Find parameter_list child
            let mut cursor = parent.walk();
            for child in parent.children(&mut cursor) {
                if child.kind() == "parameter_list" {
                    let mut params_cursor = child.walk();
                    for param in child.children(&mut params_cursor) {
                        if param.kind() == "parameter_declaration" {
                            // Extract the type_identifier from the parameter_declaration
                            let mut param_cursor = param.walk();
                            for param_child in param.children(&mut param_cursor) {
                                if param_child.kind() == "type_identifier"
                                    || param_child.kind() == "identifier"
                                {
                                    if let Ok(text) =
                                        std::str::from_utf8(&source[param_child.byte_range()])
                                    {
                                        arg_values.push(text);
                                    }
                                    break;
                                }
                            }
                        } else if param.kind() == "identifier" || param.kind() == "type_identifier"
                        {
                            // Direct identifier children (e.g., in ERROR nodes)
                            if let Ok(text) = std::str::from_utf8(&source[param.byte_range()]) {
                                arg_values.push(text);
                            }
                        }
                    }
                    break;
                }
            }
        } else {
            // Normal case: argument_list or call_expression
            self.collect_identifiers(parent, source, &mut arg_values);
        }

        tracing::debug!(
            "extract_gobject_from_identifier for {} (parent: {}): collected identifiers: {:?}",
            macro_name,
            parent.kind(),
            arg_values
        );

        // Remove the macro name itself from the list
        arg_values.retain(|name| *name != macro_name);

        // G_DECLARE_*_TYPE needs 5 args
        if macro_name.starts_with("G_DECLARE_") && arg_values.len() >= 5 {
            let type_name = arg_values[0];
            let function_prefix = arg_values[1];
            let module_prefix = arg_values[2];
            let type_prefix = arg_values[3];
            let parent_type = arg_values[4];

            let type_macro = format!("{}_TYPE_{}", module_prefix, type_prefix);

            let declare_kind = match macro_name {
                "G_DECLARE_FINAL_TYPE" => DeclareKind::Final,
                "G_DECLARE_DERIVABLE_TYPE" => DeclareKind::Derivable,
                "G_DECLARE_INTERFACE" => DeclareKind::Interface,
                _ => return None,
            };

            return Some(GObjectType {
                type_name: type_name.to_owned(),
                type_macro,
                function_prefix: function_prefix.to_owned(),
                parent_type: Some(parent_type.to_owned()),
                flags: None,
                kind: GObjectTypeKind::Declare {
                    kind: declare_kind,
                    module_prefix: module_prefix.to_owned(),
                    type_prefix: type_prefix.to_owned(),
                },
                class_struct: None,
                interfaces: Vec::new(),
                has_private: false,
                code_block_statements: Vec::new(),
                export_macros: Vec::new(),
                location: self.node_location(parent),
            });
        }

        // G_DEFINE_BOXED_TYPE* needs 4 args: TypeName, function_prefix, copy_func,
        // free_func
        if (macro_name == "G_DEFINE_BOXED_TYPE" || macro_name == "G_DEFINE_BOXED_TYPE_WITH_CODE")
            && arg_values.len() >= 4
        {
            let type_name = arg_values[0];
            let function_prefix = arg_values[1];
            let copy_func = arg_values[2];
            let free_func = arg_values[3];

            let type_macro = format!("TYPE_{}", type_name.to_uppercase());

            let (interfaces, has_private, code_block_statements) =
                if macro_name.ends_with("_WITH_CODE") {
                    self.extract_code_block_info_from_parent(parent, source, &arg_values)
                } else {
                    (Vec::new(), false, Vec::new())
                };

            return Some(GObjectType {
                type_name: type_name.to_owned(),
                type_macro,
                function_prefix: function_prefix.to_owned(),
                parent_type: None,
                flags: None,
                kind: GObjectTypeKind::DefineBoxed {
                    copy_func: copy_func.to_owned(),
                    free_func: free_func.to_owned(),
                },
                class_struct: None,
                interfaces,
                has_private,
                code_block_statements,
                export_macros: Vec::new(),
                location: self.node_location(parent),
            });
        }

        // G_DEFINE_* needs 3 args
        if macro_name.starts_with("G_DEFINE_") && arg_values.len() >= 3 {
            let type_name = arg_values[0];
            let function_prefix = arg_values[1];
            let parent_type = arg_values[2];

            let type_macro = format!("TYPE_{}", type_name.to_uppercase());

            // _WITH_PRIVATE variants always have a private struct
            let has_private_from_macro = matches!(
                macro_name,
                "G_DEFINE_TYPE_WITH_PRIVATE"
                    | "G_DEFINE_FINAL_TYPE_WITH_PRIVATE"
                    | "G_DEFINE_ABSTRACT_TYPE_WITH_PRIVATE"
            );

            let kind = match macro_name {
                "G_DEFINE_TYPE" => GObjectTypeKind::Define(DefineKind::Type),
                "G_DEFINE_TYPE_WITH_PRIVATE" => {
                    GObjectTypeKind::Define(DefineKind::TypeWithPrivate)
                }
                "G_DEFINE_ABSTRACT_TYPE" => GObjectTypeKind::Define(DefineKind::AbstractType),
                "G_DEFINE_TYPE_WITH_CODE" => GObjectTypeKind::Define(DefineKind::TypeWithCode),
                "G_DEFINE_FINAL_TYPE" => GObjectTypeKind::Define(DefineKind::FinalType),
                "G_DEFINE_FINAL_TYPE_WITH_CODE" => {
                    GObjectTypeKind::Define(DefineKind::FinalTypeWithCode)
                }
                "G_DEFINE_FINAL_TYPE_WITH_PRIVATE" => {
                    GObjectTypeKind::Define(DefineKind::FinalTypeWithPrivate)
                }
                "G_DEFINE_ABSTRACT_TYPE_WITH_CODE" => {
                    GObjectTypeKind::Define(DefineKind::AbstractTypeWithCode)
                }
                "G_DEFINE_ABSTRACT_TYPE_WITH_PRIVATE" => {
                    GObjectTypeKind::Define(DefineKind::AbstractTypeWithPrivate)
                }
                "G_DEFINE_INTERFACE" => GObjectTypeKind::Define(DefineKind::Interface),
                "G_DEFINE_INTERFACE_WITH_CODE" => {
                    GObjectTypeKind::Define(DefineKind::InterfaceWithCode)
                }
                "G_DEFINE_POINTER_TYPE" => GObjectTypeKind::Define(DefineKind::Pointer),
                // G_DEFINE_TYPE_EXTENDED(TypeName, prefix, ParentType, flags, CODE)
                "G_DEFINE_TYPE_EXTENDED" => GObjectTypeKind::Define(DefineKind::TypeExtended),
                _ => return None,
            };

            let extended_flags = if macro_name == "G_DEFINE_TYPE_EXTENDED" {
                Some(extract_nth_expression_text(parent, source, 3))
            } else {
                None
            };

            // For *_WITH_CODE macros and G_DEFINE_TYPE_EXTENDED, extract interfaces,
            // has_private, and code statements from the code block.
            let (interfaces, has_private_from_code, code_block_statements) =
                if macro_name.ends_with("_WITH_CODE") || macro_name == "G_DEFINE_TYPE_EXTENDED" {
                    self.extract_code_block_info_from_parent(parent, source, &arg_values)
                } else {
                    (Vec::new(), false, Vec::new())
                };

            return Some(GObjectType {
                type_name: type_name.to_owned(),
                type_macro,
                function_prefix: function_prefix.to_owned(),
                parent_type: if matches!(kind, GObjectTypeKind::Define(DefineKind::Pointer)) {
                    None
                } else {
                    Some(parent_type.to_owned())
                },
                flags: extended_flags,
                kind,
                class_struct: None,
                interfaces,
                has_private: has_private_from_macro || has_private_from_code,
                code_block_statements,
                export_macros: Vec::new(),
                location: self.node_location(parent),
            });
        }

        None
    }

    pub(super) fn extract_class_structs_from_ast(
        &self,
        node: Node,
        source: &[u8],
        gobject_types: &mut Vec<&mut GObjectType>,
    ) {
        // Look for struct_specifier nodes
        if node.kind() == "struct_specifier" {
            if let Some(name_node) = node.child_by_field_name("name") {
                if let Ok(struct_name) = std::str::from_utf8(&source[name_node.byte_range()]) {
                    // Check if this is a class struct (ends with "Class" and starts with "_")
                    if struct_name.starts_with("_") && struct_name.ends_with("Class") {
                        // Extract the type name: _CoglWinsysClass -> CoglWinsys
                        let type_name = &struct_name[1..struct_name.len() - 5]; // Remove leading "_" and trailing "Class"

                        // Find matching GObjectType
                        if let Some(gobject_type) = gobject_types
                            .iter_mut()
                            .find(|gt| gt.type_name == type_name)
                        {
                            // Extract virtual functions from this struct
                            if let Some(body) = node.child_by_field_name("body") {
                                let vfuncs = self.extract_vfuncs(body, source);

                                gobject_type.class_struct = Some(ClassStruct {
                                    name: struct_name.to_owned(),
                                    vfuncs,
                                });
                            }
                        }
                    }
                }
            }
        }

        // Recurse
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.extract_class_structs_from_ast(child, source, gobject_types);
        }
    }

    pub(super) fn extract_vfuncs(&self, body_node: Node, source: &[u8]) -> Vec<VirtualFunction> {
        let mut vfuncs = Vec::new();

        let mut cursor = body_node.walk();
        for child in body_node.children(&mut cursor) {
            if child.kind() == "field_declaration" {
                // Look for function pointer fields
                if let Some(vfunc) = self.extract_vfunc_from_field(child, source) {
                    vfuncs.push(vfunc);
                }
            }
        }

        vfuncs
    }

    fn extract_vfunc_from_field(&self, field_node: Node, source: &[u8]) -> Option<VirtualFunction> {
        // A function pointer field looks like:
        // return_type (*name) (params);
        // In tree-sitter this is a field_declaration with a function_declarator

        let mut cursor = field_node.walk();
        for child in field_node.children(&mut cursor) {
            if child.kind() == "function_declarator" {
                // This is a function pointer
                return self.extract_function_pointer(child, field_node, source);
            }
        }

        None
    }

    fn extract_function_pointer(
        &self,
        func_decl: Node,
        field_node: Node,
        source: &[u8],
    ) -> Option<VirtualFunction> {
        // Get the function name from the declarator
        let declarator = func_decl.child_by_field_name("declarator")?;
        let name = extract_pointer_declarator_name(declarator, source)?;

        let return_type = self.extract_return_type(field_node, source);

        // Extract parameters
        let mut parameters = Vec::new();
        if let Some(params_node) = func_decl.child_by_field_name("parameters") {
            parameters = self.extract_parameters(params_node, source);
        }

        Some(VirtualFunction {
            name: name.to_owned(),
            return_type,
            parameters,
        })
    }

    fn extract_code_block_info_from_parent(
        &self,
        parent: Node,
        source: &[u8],
        _arg_values: &[&str],
    ) -> (
        Vec<crate::model::types::InterfaceImplementation>,
        bool,
        Vec<crate::model::Statement>,
    ) {
        use crate::model::types::InterfaceImplementation;

        let mut interfaces = Vec::new();
        let mut has_private = false;
        let code_statements = Vec::new();

        // With the new grammar, *_WITH_CODE macros produce a `gobject_code_block`
        // child containing `gobject_code_block_item` nodes (identifier +
        // argument_list). Walk them directly — no heuristics needed.
        let code_block = {
            let mut cursor = parent.walk();
            parent
                .children(&mut cursor)
                .find(|c| c.kind() == "gobject_code_block")
        };

        if let Some(block) = code_block {
            let mut cursor = block.walk();
            for item in block.children(&mut cursor) {
                if item.kind() != "gobject_code_block_item" {
                    continue;
                }
                // Each item: identifier argument_list
                let mut item_cursor = item.walk();
                let mut children = item.children(&mut item_cursor);
                let name_node = children.find(|c| c.kind() == "identifier");
                let args_node = {
                    let mut item_cursor2 = item.walk();
                    item.children(&mut item_cursor2)
                        .find(|c| c.kind() == "argument_list")
                };

                let name = name_node
                    .and_then(|n| std::str::from_utf8(&source[n.byte_range()]).ok())
                    .unwrap_or("");

                match name {
                    "G_ADD_PRIVATE" => {
                        has_private = true;
                    }
                    "G_IMPLEMENT_INTERFACE" => {
                        if let Some(args) = args_node {
                            let mut iface_args = Vec::new();
                            self.collect_identifiers(args, source, &mut iface_args);
                            if iface_args.len() >= 2 {
                                interfaces.push(InterfaceImplementation {
                                    interface_type: iface_args[0].to_owned(),
                                    init_function: iface_args[1].to_owned(),
                                });
                            }
                        }
                    }
                    _ => {
                        // Other code-block calls — record as statements
                        if let Some(args) = args_node {
                            // Reconstruct a minimal call expression text for the statement
                            let item_text =
                                std::str::from_utf8(&source[item.byte_range()]).unwrap_or("");
                            tracing::debug!("code block statement: {}", item_text);
                            let _ = args; // statement parsing handled separately if needed
                        }
                    }
                }
            }
        }

        (interfaces, has_private, code_statements)
    }

    pub(super) fn collect_identifiers<'a>(
        &self,
        node: Node,
        source: &'a [u8],
        result: &mut Vec<&'a str>,
    ) {
        // Direct identifier or type_identifier nodes
        if node.kind() == "identifier" || node.kind() == "type_identifier" {
            if let Ok(text) = std::str::from_utf8(&source[node.byte_range()]) {
                result.push(text);
                return;
            }
        }

        // Only parse if this is actually an expression node
        if Parser::is_expression_node(&node) {
            if let Some(expr) = self.parse_expression(node, source) {
                collect_identifiers_from_expr(&expr, source, result);
                return;
            }
        }

        // If not an expression, recurse into ALL children (not just named ones)
        // because some tree-sitter grammars don't mark all nodes as named
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.collect_identifiers(child, source, result);
        }
    }
}

/// Return the raw source text of the n-th expression argument (0-indexed)
/// inside a `gobject_type_macro` node that uses the `_WITH_CODE` grammar rule.
/// Falls back to "0" when the argument is not found (e.g. it is a number
/// literal whose node is not reached) or cannot be decoded.
fn extract_nth_expression_text(parent: Node, source: &[u8], n: usize) -> String {
    let mut expr_count = 0;
    let mut cursor = parent.walk();
    for child in parent.children(&mut cursor) {
        if child.is_named() && child.kind() != "gobject_code_block" {
            // Skip the macro name itself (first named child)
            if expr_count == 0 {
                expr_count += 1;
                continue;
            }
            if expr_count - 1 == n {
                return std::str::from_utf8(&source[child.byte_range()])
                    .unwrap_or("0")
                    .trim()
                    .to_owned();
            }
            expr_count += 1;
        }
    }
    "0".to_owned()
}

fn collect_identifiers_from_expr<'a>(
    expr: &Expression,
    source: &'a [u8],
    result: &mut Vec<&'a str>,
) {
    expr.walk(&mut |e| {
        if let Expression::Identifier(id) = e {
            if let Ok(text) =
                std::str::from_utf8(&source[id.location.start_byte..id.location.end_byte])
            {
                result.push(text);
            }
        }
    });
}

fn extract_pointer_declarator_name<'a>(declarator: Node, source: &'a [u8]) -> Option<&'a str> {
    // For function pointers, the declarator can be:
    // - parenthesized_declarator containing pointer_declarator
    // - pointer_declarator containing identifier or field_identifier

    if declarator.kind() == "parenthesized_declarator" {
        // Look for pointer_declarator inside
        let mut cursor = declarator.walk();
        for child in declarator.children(&mut cursor) {
            if child.kind() == "pointer_declarator" {
                return extract_pointer_declarator_name(child, source);
            } else if child.kind() == "identifier" || child.kind() == "field_identifier" {
                return std::str::from_utf8(&source[child.byte_range()]).ok();
            }
        }
    } else if declarator.kind() == "pointer_declarator" {
        if let Some(inner) = declarator.child_by_field_name("declarator") {
            if inner.kind() == "identifier" || inner.kind() == "field_identifier" {
                return std::str::from_utf8(&source[inner.byte_range()]).ok();
            }
            return extract_pointer_declarator_name(inner, source);
        }
    } else if declarator.kind() == "identifier" || declarator.kind() == "field_identifier" {
        return std::str::from_utf8(&source[declarator.byte_range()]).ok();
    }

    None
}
