use tree_sitter::Node;

use crate::model::ReturnStatement;
use crate::parser::Parser;

impl Parser {
    pub(crate) fn parse_return_statement(&self, node: Node, source: &[u8]) -> Option<ReturnStatement> {
        let value = node.child(1).and_then(|v| {
            // Check if it's actually an expression (not a semicolon)
            if v.is_named() && v.kind() != ";" {
                self.parse_expression(v, source)
            } else {
                None
            }
        });

        Some(ReturnStatement {
            value,
            location: self.node_location(node),
        })
    }
}
