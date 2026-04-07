use super::Rule;
use crate::ast_context::AstContext;
use crate::config::Config;
use crate::rules::Violation;
use tree_sitter::Node;

pub struct DisposeFinalizeChainsUp;

impl Rule for DisposeFinalizeChainsUp {
    const NAME: &'static str = "dispose_finalize_chains_up";

    fn check_all(
        &self,
        ast_context: &AstContext,
        _config: &Config,
        violations: &mut Vec<Violation>,
    ) {
        for (path, file) in ast_context.iter_c_files() {
            for func in &file.functions {
                if !func.is_definition {
                    continue;
                }

                // Check if function name ends with _dispose or _finalize
                let method_type = if func.name.ends_with("_dispose") {
                    "dispose"
                } else if func.name.ends_with("_finalize") {
                    "finalize"
                } else {
                    continue;
                };

                if let Some(func_source) = ast_context.get_function_source(path, func) {
                    if let Some(tree) = ast_context.parse_c_source(func_source) {
                        let root = tree.root_node();

                        // Verify it's a GObject virtual method
                        if !self.is_gobject_virtual_method_from_source(root, func_source) {
                            continue;
                        }

                        // Find the body
                        if let Some(body) = self.find_body(root) {
                            if !self.has_chainup_call(body, func_source, method_type) {
                                violations.push(self.violation(path, func.line, 1, format!(
                                        "{} must chain up to parent class (e.g., G_OBJECT_CLASS (parent_class)->{} (object))",
                                        func.name, method_type
                                    )));
                            }
                        }
                    }
                }
            }
        }
    }
}

impl DisposeFinalizeChainsUp {
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

    fn find_body<'a>(&self, node: Node<'a>) -> Option<Node<'a>> {
        if node.kind() == "compound_statement" {
            return Some(node);
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(result) = self.find_body(child) {
                return Some(result);
            }
        }

        None
    }

    fn is_gobject_virtual_method_from_source(&self, node: Node, source: &[u8]) -> bool {
        if let Some(func_decl) = self.find_function_declarator(node) {
            if let Some(parameters) = func_decl.child_by_field_name("parameters") {
                let mut cursor = parameters.walk();
                for child in parameters.children(&mut cursor) {
                    if child.kind() == "parameter_declaration" {
                        return self.is_gobject_parameter(child, source);
                    }
                }
            }
        }
        false
    }
}
