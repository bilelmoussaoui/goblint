mod assignment;
mod binary;
mod call;
mod cast;
mod conditional;
mod sizeof;
mod subscript;
mod unary;
mod update;

use tree_sitter::Node;

use super::Parser;
use crate::model::Expression;

impl Parser {
    pub(super) fn parse_expression(&self, node: Node, source: &[u8]) -> Option<Expression> {
        use crate::model::*;

        match node.kind() {
            "call_expression" => self
                .parse_call_expression(node, source)
                .map(Expression::Call),
            "assignment_expression" => self
                .parse_assignment(node, source)
                .map(Expression::Assignment),
            "binary_expression" => self
                .parse_binary_expression(node, source)
                .map(Expression::Binary),
            "unary_expression" | "pointer_expression" => self
                .parse_unary_expression(node, source)
                .map(Expression::Unary),
            "parenthesized_expression" => {
                // Unwrap the parentheses and parse the inner expression
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.is_named() && child.kind() != "(" && child.kind() != ")" {
                        if Parser::is_expression_node(&child) {
                            return self.parse_expression(child, source);
                        }
                    }
                }
                None
            }
            "identifier" => {
                let name = std::str::from_utf8(&source[node.byte_range()])
                    .ok()?
                    .to_owned();
                Some(Expression::Identifier(IdentifierExpression {
                    name,
                    location: self.node_location(node),
                }))
            }
            "field_expression" => {
                // Parse field_expression: base->field or base.field
                let argument_node = node.child_by_field_name("argument")?;
                let base = Box::new(self.parse_expression(argument_node, source)?);

                let operator_node = node.child_by_field_name("operator")?;
                let operator_str = std::str::from_utf8(&source[operator_node.byte_range()]).ok()?;
                let operator = FieldAccessOp::from_str(operator_str)?;

                let field_node = node.child_by_field_name("field")?;
                let field = std::str::from_utf8(&source[field_node.byte_range()])
                    .ok()?
                    .to_owned();

                Some(Expression::FieldAccess(FieldAccessExpression {
                    base,
                    operator,
                    field,
                    location: self.node_location(node),
                }))
            }
            "string_literal" => {
                let value = std::str::from_utf8(&source[node.byte_range()])
                    .ok()?
                    .to_owned();
                Some(Expression::StringLiteral(StringLiteralExpression {
                    value,
                    location: self.node_location(node),
                }))
            }
            "number_literal" => {
                let value = std::str::from_utf8(&source[node.byte_range()])
                    .ok()?
                    .to_owned();
                Some(Expression::NumberLiteral(NumberLiteralExpression {
                    value,
                    location: self.node_location(node),
                }))
            }
            "null" | "NULL" => Some(Expression::Null(NullExpression {
                location: self.node_location(node),
            })),
            "true" | "TRUE" => Some(Expression::Boolean(BooleanExpression {
                value: true,
                location: self.node_location(node),
            })),
            "false" | "FALSE" => Some(Expression::Boolean(BooleanExpression {
                value: false,
                location: self.node_location(node),
            })),
            "cast_expression" => self.parse_cast_expression(node, source),
            "conditional_expression" => self.parse_conditional_expression(node, source),
            "sizeof_expression" => self
                .parse_sizeof_expression(node, source)
                .map(Expression::Sizeof),
            "alignof_expression" => {
                // alignof(type) or _Alignof(type)
                let text = std::str::from_utf8(&source[node.byte_range()])
                    .ok()?
                    .to_owned();
                Some(Expression::Generic(GenericExpression {
                    text,
                    location: self.node_location(node),
                }))
            }
            "subscript_expression" => self.parse_subscript_expression(node, source),
            "initializer_list" => self
                .parse_initializer_list(node, source)
                .map(Expression::InitializerList),
            "char_literal" => {
                let value = std::str::from_utf8(&source[node.byte_range()])
                    .ok()?
                    .to_owned();
                Some(Expression::CharLiteral(CharLiteralExpression {
                    value,
                    location: self.node_location(node),
                }))
            }
            "update_expression" => self.parse_update_expression(node, source),
            "concatenated_string" => {
                // Concatenated string literals: "foo" "bar" → semantically a string literal
                let value = std::str::from_utf8(&source[node.byte_range()])
                    .ok()?
                    .to_owned();
                Some(Expression::StringLiteral(StringLiteralExpression {
                    value,
                    location: self.node_location(node),
                }))
            }
            "compound_literal_expression" => {
                // Compound literal: (Type){.x = 1, .y = 2}
                // Parse the initializer_list child
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "initializer_list" {
                        return self.parse_expression(child, source);
                    }
                }
                // Fallback if no initializer_list found
                None
            }
            "comma_expression" => {
                // Comma operator: (a, b, c) → value is rightmost expression
                let mut cursor = node.walk();
                let mut last_expr = None;
                for child in node.children(&mut cursor) {
                    if child.is_named() && child.kind() != "," {
                        if Parser::is_expression_node(&child) {
                            last_expr = self.parse_expression(child, source);
                        }
                    }
                }
                last_expr
            }
            "offsetof_expression" => {
                // offsetof(struct, field)
                let text = std::str::from_utf8(&source[node.byte_range()])
                    .ok()?
                    .to_owned();
                Some(Expression::Generic(GenericExpression {
                    text,
                    location: self.node_location(node),
                }))
            }
            "gnu_asm_expression" => {
                // GNU inline assembly: __asm__ ("...")
                let text = std::str::from_utf8(&source[node.byte_range()])
                    .ok()?
                    .to_owned();
                Some(Expression::Generic(GenericExpression {
                    text,
                    location: self.node_location(node),
                }))
            }
            "compound_statement" => {
                // GNU C statement expression: ({ int x = 5; x + 1; })
                let text = std::str::from_utf8(&source[node.byte_range()])
                    .ok()?
                    .to_owned();
                Some(Expression::Generic(GenericExpression {
                    text,
                    location: self.node_location(node),
                }))
            }
            "comment" => {
                // Preserve comments so rules can restore them
                let text = std::str::from_utf8(&source[node.byte_range()])
                    .ok()?
                    .to_owned();
                Some(Expression::Comment(CommentExpression {
                    text,
                    location: self.node_location(node),
                }))
            }
            "ERROR" => {
                // Skip parse errors gracefully
                None
            }
            _ => {
                // Unknown expression type - fail loudly so we implement it immediately
                todo!(
                    "Unimplemented expression type: {} at {}:{}",
                    node.kind(),
                    node.start_position().row + 1,
                    node.start_position().column + 1
                )
            }
        }
    }

    fn parse_initializer_list(
        &self,
        node: Node,
        source: &[u8],
    ) -> Option<crate::model::InitializerListExpression> {
        use crate::model::expression::{Designator, InitializerItem};

        let mut items = Vec::new();
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            match child.kind() {
                "{" | "}" | "," => {
                    // Skip delimiters
                    continue;
                }
                "initializer_pair" => {
                    // Designated initializer: .field = value or [index] = value
                    let mut pair_cursor = child.walk();
                    let mut designator = None;
                    let mut value = None;

                    for pair_child in child.children(&mut pair_cursor) {
                        match pair_child.kind() {
                            "field_designator" => {
                                // .field_name
                                let field_text =
                                    std::str::from_utf8(&source[pair_child.byte_range()]).ok()?;
                                // Remove the leading '.'
                                let field_name = field_text.strip_prefix('.')?.to_owned();
                                designator = Some(Designator::Field(field_name));
                            }
                            "subscript_designator" => {
                                // [index_expression]
                                // Parse the expression inside the brackets
                                let mut sub_cursor = pair_child.walk();
                                for sub_child in pair_child.children(&mut sub_cursor) {
                                    if sub_child.kind() != "[" && sub_child.kind() != "]" {
                                        if let Some(index_expr) =
                                            self.parse_expression(sub_child, source)
                                        {
                                            designator =
                                                Some(Designator::Subscript(Box::new(index_expr)));
                                            break;
                                        }
                                    }
                                }
                            }
                            "=" => {
                                // Skip the equals sign
                                continue;
                            }
                            _ => {
                                // This should be the value expression
                                if Parser::is_expression_node(&pair_child) {
                                    value = self.parse_expression(pair_child, source);
                                }
                            }
                        }
                    }

                    if let Some(val) = value {
                        items.push(InitializerItem {
                            designator,
                            value: Box::new(val),
                        });
                    }
                }
                _ => {
                    // Direct value (no designator): just an expression
                    if Parser::is_expression_node(&child) {
                        if let Some(expr) = self.parse_expression(child, source) {
                            items.push(InitializerItem {
                                designator: None,
                                value: Box::new(expr),
                            });
                        }
                    }
                }
            }
        }

        Some(crate::model::InitializerListExpression {
            items,
            location: self.node_location(node),
        })
    }
}
