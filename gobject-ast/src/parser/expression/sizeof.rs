use tree_sitter::Node;

use crate::{
    Parser,
    model::expression::{SizeofExpression, SizeofOperand},
};

impl Parser {
    pub(in crate::parser) fn parse_sizeof_expression(
        &self,
        node: Node,
        source: &[u8],
    ) -> Option<SizeofExpression> {
        let text = std::str::from_utf8(&source[node.byte_range()])
            .ok()?
            .to_owned();

        let mut operand = None;

        // Walk children to find what sizeof is operating on
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "sizeof" | "(" | ")" => continue,

                // tree-sitter gives us type_descriptor for explicit type contexts
                "type_descriptor" => {
                    let type_text = std::str::from_utf8(&source[child.byte_range()])
                        .ok()?
                        .to_owned();
                    operand = Some(SizeofOperand::Type(type_text));
                }

                // Parenthesized expression is ambiguous - could be type or expression
                // Just parse as expression and let the rule decide what to do with it
                _ if child.is_named() && Parser::is_expression_node(&child) => {
                    if let Some(expr) = self.parse_expression(child, source) {
                        operand = Some(SizeofOperand::Expression(Box::new(expr)));
                    }
                }
                _ => {}
            }
        }

        Some(SizeofExpression {
            operand,
            text,
            location: self.node_location(node),
        })
    }
}
