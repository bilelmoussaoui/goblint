mod call;
mod assignment;
mod binary;
mod unary;
mod cast;
mod conditional;
mod subscript;
mod update;

use tree_sitter::Node;

use crate::model::Expression;
use super::Parser;

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
                        return self.parse_expression(child, source);
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
            "sizeof_expression" => {
                let text = std::str::from_utf8(&source[node.byte_range()])
                    .ok()?
                    .to_owned();
                Some(Expression::Sizeof(SizeofExpression {
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
