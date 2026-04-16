use tree_sitter::Node;

use crate::model::{CompoundStatement, Statement};
use crate::parser::Parser;

impl Parser {
    pub(crate) fn parse_compound_statement(&self, node: Node, source: &[u8]) -> Option<CompoundStatement> {
        let statements = self.parse_function_body(node, source);

        Some(CompoundStatement {
            statements,
            location: self.node_location(node),
        })
    }

    pub(in crate::parser) fn parse_function_body(&self, body_node: Node, source: &[u8]) -> Vec<Statement> {
        let mut statements = Vec::new();

        let mut cursor = body_node.walk();
        for child in body_node.children(&mut cursor) {
            if let Some(stmt) = self.parse_statement(child, source) {
                statements.push(stmt);
            }
        }

        statements
    }
}
