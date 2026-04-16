use tree_sitter::Node;

use crate::model::*;
use super::Parser;

impl Parser {
    pub(super) fn extract_include(&self, node: Node, source: &[u8]) -> Option<Include> {
        let path_node = node.child_by_field_name("path")?;
        let path_text = std::str::from_utf8(&source[path_node.byte_range()]).ok()?;

        // Check if system include (<>) or local ("")
        let is_system = path_text.starts_with('<');
        let path = path_text.trim_matches(&['<', '>', '"'][..]);

        Some(Include {
            path: path.to_owned(),
            is_system,
            line: node.start_position().row + 1,
        })
    }

    pub(super) fn extract_typedef_from_type_definition(
        &self,
        node: Node,
        source: &[u8],
    ) -> Option<TypedefInfo> {
        // type_definition has "declarator" for the typedef name and "type" for what
        // it's typedef'ing
        let declarator_node = node.child_by_field_name("declarator")?;
        let name = std::str::from_utf8(&source[declarator_node.byte_range()]).ok()?;

        let type_node = node.child_by_field_name("type")?;
        let target_type = std::str::from_utf8(&source[type_node.byte_range()]).ok()?;

        Some(TypedefInfo {
            name: name.to_owned(),
            line: node.start_position().row + 1,
            target_type: target_type.to_owned(),
        })
    }

    pub(super) fn extract_struct(&self, node: Node, source: &[u8]) -> Option<StructInfo> {
        // Look for struct definitions or declarations
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "struct_specifier" {
                if let Some(name_node) = child.child_by_field_name("name") {
                    let name = std::str::from_utf8(&source[name_node.byte_range()]).ok()?;
                    let has_body = child.child_by_field_name("body").is_some();
                    return Some(StructInfo {
                        name: name.to_owned(),
                        line: child.start_position().row + 1,
                        fields: Vec::new(), // TODO: extract fields
                        is_opaque: !has_body,
                    });
                }
            }
        }
        None
    }

    pub(super) fn extract_enum(&self, node: Node, source: &[u8]) -> Option<EnumInfo> {
        // Handle typedef enum { ... } Name;
        if node.kind() == "type_definition" {
            if let Some(type_node) = node.child_by_field_name("type") {
                if type_node.kind() == "enum_specifier" {
                    if let Some(declarator_node) = node.child_by_field_name("declarator") {
                        let name =
                            std::str::from_utf8(&source[declarator_node.byte_range()]).ok()?;
                        if let Some(body) = type_node.child_by_field_name("body") {
                            let values = self.extract_enum_values(body, source);
                            return Some(EnumInfo {
                                name: name.to_owned(),
                                line: node.start_position().row + 1,
                                values,
                                body_start_byte: body.start_byte(),
                                body_end_byte: body.end_byte(),
                            });
                        }
                    }
                }
            }
        }

        // Handle standalone enum Name { ... };
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "enum_specifier" {
                if let Some(name_node) = child.child_by_field_name("name") {
                    let name = std::str::from_utf8(&source[name_node.byte_range()]).ok()?;
                    if let Some(body) = child.child_by_field_name("body") {
                        let values = self.extract_enum_values(body, source);
                        return Some(EnumInfo {
                            name: name.to_owned(),
                            line: child.start_position().row + 1,
                            values,
                            body_start_byte: body.start_byte(),
                            body_end_byte: body.end_byte(),
                        });
                    }
                }
            }
        }
        None
    }

    pub(super) fn extract_enum_values(&self, body_node: Node, source: &[u8]) -> Vec<EnumValue> {
        let mut values = Vec::new();

        let mut cursor = body_node.walk();
        for child in body_node.children(&mut cursor) {
            if child.kind() == "enumerator" {
                if let Some(name_node) = child.child_by_field_name("name") {
                    let name = std::str::from_utf8(&source[name_node.byte_range()])
                        .unwrap_or("")
                        .to_owned();

                    let (value, value_start, value_end) =
                        if let Some(value_node) = child.child_by_field_name("value") {
                            // Try to parse the value as an integer
                            let value_str = std::str::from_utf8(&source[value_node.byte_range()])
                                .unwrap_or("")
                                .trim();
                            (
                                value_str.parse::<i64>().ok(),
                                Some(value_node.start_byte()),
                                Some(value_node.end_byte()),
                            )
                        } else {
                            (None, None, None)
                        };

                    values.push(EnumValue {
                        name,
                        value,
                        start_byte: child.start_byte(),
                        end_byte: child.end_byte(),
                        name_start_byte: name_node.start_byte(),
                        name_end_byte: name_node.end_byte(),
                        value_start_byte: value_start,
                        value_end_byte: value_end,
                    });
                }
            }
        }

        values
    }

    pub(super) fn extract_gobject_type_declaration(&self, node: Node, source: &[u8]) -> Option<GObjectType> {
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

    pub(super) fn extract_g_declare(
        &self,
        node: Node,
        source: &[u8],
        macro_name: &str,
    ) -> Option<GObjectType> {
        // G_DECLARE_*_TYPE (TypeName, function_prefix, MODULE, TYPE_NAME, ParentType)
        let args = node.child_by_field_name("arguments")?;
        let mut cursor = args.walk();
        let mut arg_values = Vec::new();

        for child in args.children(&mut cursor) {
            if child.kind() == "identifier" || child.kind() == "type_identifier" {
                let text = std::str::from_utf8(&source[child.byte_range()]).ok()?;
                arg_values.push(text);
            }
        }

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

    pub(super) fn extract_g_define(&self, node: Node, source: &[u8], macro_name: &str) -> Option<GObjectType> {
        // G_DEFINE_TYPE (TypeName, function_prefix, PARENT_TYPE)
        let args = node.child_by_field_name("arguments")?;
        let mut cursor = args.walk();
        let mut arg_values = Vec::new();

        for child in args.children(&mut cursor) {
            if child.kind() == "identifier" || child.kind() == "type_identifier" {
                let text = std::str::from_utf8(&source[child.byte_range()]).ok()?;
                arg_values.push(text);
            }
        }

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

    pub(super) fn collect_identifiers<'a>(&self, node: Node, source: &'a [u8], result: &mut Vec<&'a str>) {
        if node.kind() == "identifier" || node.kind() == "type_identifier" {
            if let Ok(text) = std::str::from_utf8(&source[node.byte_range()]) {
                result.push(text);
            }
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.collect_identifiers(child, source, result);
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

    pub(super) fn extract_vfunc_from_field(&self, field_node: Node, source: &[u8]) -> Option<VirtualFunction> {
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

    pub(super) fn extract_function_pointer(
        &self,
        func_decl: Node,
        field_node: Node,
        source: &[u8],
    ) -> Option<VirtualFunction> {
        // Get the function name from the declarator
        let declarator = func_decl.child_by_field_name("declarator")?;
        let name = self.extract_pointer_declarator_name(declarator, source)?;

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

    pub(super) fn extract_pointer_declarator_name<'a>(
        &self,
        declarator: Node,
        source: &'a [u8],
    ) -> Option<&'a str> {
        // For function pointers, the declarator can be:
        // - parenthesized_declarator containing pointer_declarator
        // - pointer_declarator containing identifier or field_identifier

        if declarator.kind() == "parenthesized_declarator" {
            // Look for pointer_declarator inside
            let mut cursor = declarator.walk();
            for child in declarator.children(&mut cursor) {
                if child.kind() == "pointer_declarator" {
                    return self.extract_pointer_declarator_name(child, source);
                } else if child.kind() == "identifier" || child.kind() == "field_identifier" {
                    return std::str::from_utf8(&source[child.byte_range()]).ok();
                }
            }
        } else if declarator.kind() == "pointer_declarator" {
            if let Some(inner) = declarator.child_by_field_name("declarator") {
                if inner.kind() == "identifier" || inner.kind() == "field_identifier" {
                    return std::str::from_utf8(&source[inner.byte_range()]).ok();
                }
                return self.extract_pointer_declarator_name(inner, source);
            }
        } else if declarator.kind() == "identifier" || declarator.kind() == "field_identifier" {
            return std::str::from_utf8(&source[declarator.byte_range()]).ok();
        }

        None
    }

    pub(super) fn extract_parameters(&self, params_node: Node, source: &[u8]) -> Vec<Parameter> {
        let mut parameters = Vec::new();

        let mut cursor = params_node.walk();
        for child in params_node.children(&mut cursor) {
            if child.kind() == "parameter_declaration" {
                let type_node = child.child_by_field_name("type");
                let type_name = type_node
                    .and_then(|t| std::str::from_utf8(&source[t.byte_range()]).ok())
                    .unwrap_or_default();

                let declarator = child.child_by_field_name("declarator");
                let name = declarator.and_then(|d| self.extract_declarator_name(d, source));

                parameters.push(Parameter {
                    name: name.map(ToOwned::to_owned),
                    type_name: type_name.to_owned(),
                });
            }
        }

        parameters
    }

    pub(super) fn extract_declarator_name<'a>(&self, declarator: Node, source: &'a [u8]) -> Option<&'a str> {
        if let Some(inner) = declarator.child_by_field_name("declarator") {
            if inner.kind() == "identifier" {
                let name = &source[inner.byte_range()];
                return Some(std::str::from_utf8(name).ok()?);
            }
            return self.extract_declarator_name(inner, source);
        }

        if declarator.kind() == "identifier" {
            let name = &source[declarator.byte_range()];
            return Some(std::str::from_utf8(name).ok()?);
        }

        // Handle parenthesized declarators like (function_name) used to prevent macro
        // expansion
        if declarator.kind() == "parenthesized_declarator" {
            let mut cursor = declarator.walk();
            for child in declarator.children(&mut cursor) {
                if child.kind() == "identifier" {
                    let name = &source[child.byte_range()];
                    return Some(std::str::from_utf8(name).ok()?);
                }
            }
        }

        None
    }
}
