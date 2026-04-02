use super::{Rule, Violation};
use crate::config::Config;
use std::path::Path;
use tree_sitter::Node;

pub struct DisposeFinalizeChainsUp;

impl DisposeFinalizeChainsUp {
    fn is_dispose_or_finalize_function(&self, node: Node, source: &[u8]) -> Option<String> {
        if node.kind() != "function_definition" {
            return None;
        }

        let declarator = node.child_by_field_name("declarator")?;
        let function_name = self.extract_function_name(declarator, source)?;

        // Check if function name ends with _dispose or _finalize
        let method_type = if function_name.ends_with("_dispose") {
            "dispose"
        } else if function_name.ends_with("_finalize") {
            "finalize"
        } else {
            return None;
        };

        // Verify this is actually a GObject virtual method by checking the parameter type
        if !self.is_gobject_virtual_method(declarator, source) {
            return None;
        }

        Some(method_type.to_string())
    }

    fn is_gobject_virtual_method(&self, declarator: Node, source: &[u8]) -> bool {
        // Find the function_declarator which contains parameters
        let Some(function_declarator) = self.find_function_declarator(declarator) else {
            return false;
        };

        // Get the parameter list
        let Some(parameters) = function_declarator.child_by_field_name("parameters") else {
            return false;
        };

        // Check the first parameter - should be GObject* or similar
        let mut cursor = parameters.walk();
        for child in parameters.children(&mut cursor) {
            if child.kind() == "parameter_declaration" {
                return self.is_gobject_parameter(child, source);
            }
        }

        false
    }

    fn find_function_declarator<'a>(&self, node: Node<'a>) -> Option<Node<'a>> {
        if node.kind() == "function_declarator" {
            return Some(node);
        }

        // Recursively search children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(result) = self.find_function_declarator(child) {
                return Some(result);
            }
        }

        None
    }

    fn is_gobject_parameter(&self, param_node: Node, source: &[u8]) -> bool {
        // Get the type of the parameter
        let param_text = self.get_node_text(param_node, source);

        // Check if parameter type is GObject* or the parameter name is "object"
        // Common patterns:
        // - "GObject *object"
        // - "GObject* object"
        // - Any type ending with "Object *" (MetaObject*, MyObject*, etc.)
        param_text.contains("GObject")
            || (param_text.contains("Object") && param_text.contains('*'))
            || param_text.contains("* object")
            || param_text.contains("*object")
    }

    fn extract_function_name(&self, declarator: Node, source: &[u8]) -> Option<String> {
        // Handle function_declarator -> identifier
        if declarator.kind() == "function_declarator" {
            if let Some(inner_declarator) = declarator.child_by_field_name("declarator") {
                if inner_declarator.kind() == "identifier" {
                    let name = &source[inner_declarator.byte_range()];
                    return Some(std::str::from_utf8(name).ok()?.to_string());
                }
            }
        }

        // Handle pointer_declarator -> function_declarator -> identifier
        if declarator.kind() == "pointer_declarator" {
            if let Some(inner) = declarator.child_by_field_name("declarator") {
                return self.extract_function_name(inner, source);
            }
        }

        None
    }

    fn has_chainup_call(&self, node: Node, source: &[u8], method_type: &str) -> bool {
        // Pattern 1: Direct call - G_OBJECT_CLASS (xxx)->dispose/finalize
        // Pattern 2: Indirect - variable assigned from parent class, then variable->dispose/finalize

        if node.kind() == "field_expression" {
            // Check if field name matches method_type (dispose/finalize)
            if let Some(field) = node.child_by_field_name("field") {
                let field_text = &source[field.byte_range()];
                let field_str = std::str::from_utf8(field_text).unwrap_or("");

                if field_str == method_type {
                    // Check if the argument contains G_OBJECT_CLASS or similar parent class macro
                    if let Some(argument) = node.child_by_field_name("argument") {
                        let arg_text = self.get_node_text(argument, source);

                        // Pattern 1: Direct parent class cast
                        if self.looks_like_parent_class_cast(&arg_text) {
                            return true;
                        }

                        // Pattern 2: Variable that looks like it holds a parent class
                        // Examples: parent_object_class, parent_class, klass, object_class
                        if self.looks_like_parent_class_variable(&arg_text) {
                            return true;
                        }
                    }
                }
            }
        }

        // Recursively check children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if self.has_chainup_call(child, source, method_type) {
                return true;
            }
        }

        false
    }

    fn looks_like_parent_class_cast(&self, text: &str) -> bool {
        // Common patterns for parent class access:
        // G_OBJECT_CLASS (parent_class)
        // G_OBJECT_CLASS (my_class_parent_class)
        // FOO_CLASS (parent_class)
        text.contains("_CLASS") && text.contains("parent")
    }

    fn looks_like_parent_class_variable(&self, text: &str) -> bool {
        // Common variable names that hold parent class:
        // parent_object_class, parent_class, object_class, klass, parent_klass
        let text_lower = text.to_lowercase();

        // Check for explicit parent references
        if text_lower.contains("parent")
            && (text_lower.contains("class") || text_lower.contains("klass"))
        {
            return true;
        }

        // Check for *_class or *_klass variables (object_class, gobject_class, etc.)
        if text_lower.ends_with("_class") || text_lower.ends_with("_klass") {
            return true;
        }

        // Just "klass" or "class" by itself
        if text_lower == "klass" || text_lower == "class" {
            return true;
        }

        false
    }

    fn get_node_text(&self, node: Node, source: &[u8]) -> String {
        let text = &source[node.byte_range()];
        std::str::from_utf8(text).unwrap_or("").to_string()
    }
}

impl Rule for DisposeFinalizeChainsUp {
    fn name(&self) -> &str {
        "dispose_finalize_chains_up"
    }

    fn check(&self, node: Node, source: &[u8], file_path: &Path) -> Vec<Violation> {
        let mut violations = Vec::new();

        let Some(method_type) = self.is_dispose_or_finalize_function(node, source) else {
            return violations;
        };

        // Get the function body
        let Some(body) = node.child_by_field_name("body") else {
            return violations;
        };

        // Check if there's a chain-up call in the function body
        if !self.has_chainup_call(body, source, &method_type) {
            let position = node.start_position();

            // Try to get function name for better error message
            let function_name = if let Some(declarator) = node.child_by_field_name("declarator") {
                self.extract_function_name(declarator, source)
                    .unwrap_or_else(|| format!("{}()", method_type))
            } else {
                format!("{}()", method_type)
            };

            violations.push(Violation {
                file: file_path.display().to_string(),
                line: position.row + 1,
                column: position.column + 1,
                message: format!(
                    "{} must chain up to parent class (e.g., G_OBJECT_CLASS (parent_class)->{} (object))",
                    function_name, method_type
                ),
                rule: self.name().to_string(),
                snippet: None,
            });
        }

        violations
    }

    fn is_enabled(&self, config: &Config) -> bool {
        config.rules.dispose_finalize_chains_up
    }
}
