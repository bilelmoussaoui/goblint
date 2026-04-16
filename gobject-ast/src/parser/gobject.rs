use tree_sitter::Node;

use super::Parser;
use crate::model::{
    Expression,
    types::{ClassStruct, GObjectType, GObjectTypeKind, VirtualFunction},
};

impl Parser {
    pub(super) fn extract_gobject_type_declaration(
        &self,
        node: Node,
        source: &[u8],
    ) -> Option<GObjectType> {
        let directive = node.child_by_field_name("directive")?;
        let directive_text = std::str::from_utf8(&source[directive.byte_range()]).ok()?;

        // Check if it's a G_DECLARE_* or G_DEFINE_* macro
        match directive_text {
            "G_DECLARE_FINAL_TYPE" | "G_DECLARE_DERIVABLE_TYPE" | "G_DECLARE_INTERFACE" => {
                self.extract_g_declare(node, source, directive_text)
            }
            "G_DEFINE_TYPE" | "G_DEFINE_TYPE_WITH_PRIVATE" | "G_DEFINE_ABSTRACT_TYPE" => {
                self.extract_g_define(node, source, directive_text)
            }
            _ => None,
        }
    }

    pub(super) fn extract_gobject_from_identifier(
        &self,
        _id_node: Node,
        parent: Node,
        source: &[u8],
        macro_name: &str,
    ) -> Option<GObjectType> {
        // Recursively find all identifiers in parent node
        let mut arg_values = Vec::new();
        self.collect_identifiers(parent, source, &mut arg_values);

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

            let kind = match macro_name {
                "G_DECLARE_FINAL_TYPE" => GObjectTypeKind::DeclareFinal {
                    function_prefix: function_prefix.to_owned(),
                    module_prefix: module_prefix.to_owned(),
                    type_prefix: type_prefix.to_owned(),
                    parent_type: parent_type.to_owned(),
                },
                "G_DECLARE_DERIVABLE_TYPE" => GObjectTypeKind::DeclareDerivable {
                    function_prefix: function_prefix.to_owned(),
                    module_prefix: module_prefix.to_owned(),
                    type_prefix: type_prefix.to_owned(),
                    parent_type: parent_type.to_owned(),
                },
                "G_DECLARE_INTERFACE" => GObjectTypeKind::DeclareInterface {
                    function_prefix: function_prefix.to_owned(),
                    module_prefix: module_prefix.to_owned(),
                    type_prefix: type_prefix.to_owned(),
                    prerequisite_type: parent_type.to_owned(),
                },
                _ => return None,
            };

            return Some(GObjectType {
                type_name: type_name.to_owned(),
                type_macro,
                kind,
                class_struct: None,
                line: parent.start_position().row + 1,
            });
        }

        // G_DEFINE_* needs 3 args
        if macro_name.starts_with("G_DEFINE_") && arg_values.len() >= 3 {
            let type_name = arg_values[0];
            let function_prefix = arg_values[1];
            let parent_type = arg_values[2];

            let type_macro = format!("TYPE_{}", type_name.to_uppercase());

            let kind = match macro_name {
                "G_DEFINE_TYPE" => GObjectTypeKind::DefineType {
                    function_prefix: function_prefix.to_owned(),
                    parent_type: parent_type.to_owned(),
                },
                "G_DEFINE_TYPE_WITH_PRIVATE" => GObjectTypeKind::DefineTypeWithPrivate {
                    function_prefix: function_prefix.to_owned(),
                    parent_type: parent_type.to_owned(),
                },
                "G_DEFINE_ABSTRACT_TYPE" => GObjectTypeKind::DefineAbstractType {
                    function_prefix: function_prefix.to_owned(),
                    parent_type: parent_type.to_owned(),
                },
                _ => return None,
            };

            return Some(GObjectType {
                type_name: type_name.to_owned(),
                type_macro,
                kind,
                class_struct: None,
                line: parent.start_position().row + 1,
            });
        }

        None
    }

    pub(super) fn extract_class_structs_from_ast(
        &self,
        node: Node,
        source: &[u8],
        gobject_types: &mut [GObjectType],
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

        // Get return type from the field_declaration type
        let return_type = field_node
            .child_by_field_name("type")
            .and_then(|t| std::str::from_utf8(&source[t.byte_range()]).ok());

        // Extract parameters
        let mut parameters = Vec::new();
        if let Some(params_node) = func_decl.child_by_field_name("parameters") {
            parameters = self.extract_parameters(params_node, source);
        }

        Some(VirtualFunction {
            name: name.to_owned(),
            return_type: return_type.map(ToOwned::to_owned),
            parameters,
        })
    }

    fn extract_g_declare(
        &self,
        node: Node,
        source: &[u8],
        macro_name: &str,
    ) -> Option<GObjectType> {
        // G_DECLARE_*_TYPE (TypeName, function_prefix, MODULE, TYPE_NAME, ParentType)
        let args = node.child_by_field_name("arguments")?;

        // Collect identifiers from the arguments using our AST walker
        let mut arg_values = Vec::new();
        self.collect_identifiers(args, source, &mut arg_values);

        if arg_values.len() < 5 {
            return None;
        }

        let type_name = arg_values[0];
        let function_prefix = arg_values[1];
        let module_prefix = arg_values[2];
        let type_prefix = arg_values[3];
        let parent_type = arg_values[4];

        let type_macro = format!("{}_TYPE_{}", module_prefix, type_prefix);

        let kind = match macro_name {
            "G_DECLARE_FINAL_TYPE" => GObjectTypeKind::DeclareFinal {
                function_prefix: function_prefix.to_owned(),
                module_prefix: module_prefix.to_owned(),
                type_prefix: type_prefix.to_owned(),
                parent_type: parent_type.to_owned(),
            },
            "G_DECLARE_DERIVABLE_TYPE" => GObjectTypeKind::DeclareDerivable {
                function_prefix: function_prefix.to_owned(),
                module_prefix: module_prefix.to_owned(),
                type_prefix: type_prefix.to_owned(),
                parent_type: parent_type.to_owned(),
            },
            "G_DECLARE_INTERFACE" => GObjectTypeKind::DeclareInterface {
                function_prefix: function_prefix.to_owned(),
                module_prefix: module_prefix.to_owned(),
                type_prefix: type_prefix.to_owned(),
                prerequisite_type: parent_type.to_owned(),
            },
            _ => return None,
        };

        Some(GObjectType {
            type_name: type_name.to_owned(),
            type_macro,
            kind,
            class_struct: None,
            line: node.start_position().row + 1,
        })
    }

    fn extract_g_define(&self, node: Node, source: &[u8], macro_name: &str) -> Option<GObjectType> {
        // G_DEFINE_TYPE (TypeName, function_prefix, PARENT_TYPE)
        let args = node.child_by_field_name("arguments")?;

        // Collect identifiers from the arguments using our AST walker
        let mut arg_values = Vec::new();
        self.collect_identifiers(args, source, &mut arg_values);

        if arg_values.len() < 3 {
            return None;
        }

        let type_name = arg_values[0];
        let function_prefix = arg_values[1];
        let parent_type = arg_values[2];

        // Generate type macro from type name
        let type_macro = format!("TYPE_{}", type_name.to_uppercase());

        let kind = match macro_name {
            "G_DEFINE_TYPE" => GObjectTypeKind::DefineType {
                function_prefix: function_prefix.to_owned(),
                parent_type: parent_type.to_owned(),
            },
            "G_DEFINE_TYPE_WITH_PRIVATE" => GObjectTypeKind::DefineTypeWithPrivate {
                function_prefix: function_prefix.to_owned(),
                parent_type: parent_type.to_owned(),
            },
            "G_DEFINE_ABSTRACT_TYPE" => GObjectTypeKind::DefineAbstractType {
                function_prefix: function_prefix.to_owned(),
                parent_type: parent_type.to_owned(),
            },
            _ => return None,
        };

        Some(GObjectType {
            type_name: type_name.to_owned(),
            type_macro,
            kind,
            class_struct: None,
            line: node.start_position().row + 1,
        })
    }

    fn collect_identifiers<'a>(&self, node: Node, source: &'a [u8], result: &mut Vec<&'a str>) {
        // Only parse if this is actually an expression node
        if Parser::is_expression_node(&node) {
            if let Some(expr) = self.parse_expression(node, source) {
                collect_identifiers_from_expr(&expr, source, result);
                return;
            }
        }

        // If not an expression, recurse into children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.is_named() {
                self.collect_identifiers(child, source, result);
            }
        }
    }
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
