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
                let text = std::str::from_utf8(&source[node.byte_range()])
                    .ok()?
                    .to_owned();
                Some(Expression::FieldAccess(FieldAccessExpression {
                    text,
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
            "initializer_list" => {
                let text = std::str::from_utf8(&source[node.byte_range()])
                    .ok()?
                    .to_owned();
                Some(Expression::InitializerList(InitializerListExpression {
                    text,
                    location: self.node_location(node),
                }))
            }
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
                // Compound literal: (struct foo){.x = 1}
                let text = std::str::from_utf8(&source[node.byte_range()])
                    .ok()?
                    .to_owned();
                Some(Expression::Generic(GenericExpression {
                    text,
                    location: self.node_location(node),
                }))
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
}
