use tree_sitter::Node;

use crate::{model::ExpressionStmt, parser::Parser};

impl Parser {
    pub(crate) fn parse_expression_stmt(
        &self,
        node: Node,
        source: &[u8],
    ) -> Option<ExpressionStmt> {
        // Get the actual expression inside the statement
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.is_named() && child.kind() != ";" {
                if Parser::is_expression_node(&child) {
                    if let Some(expr) = self.parse_expression(child, source) {
                        return Some(ExpressionStmt {
                            expr,
                            location: self.node_location(node),
                        });
                    }
                }
            }
        }
        None
    }
}
