use tree_sitter::Node;

use super::Parser;
use crate::model::statement::ContinueStatement;

impl Parser {
    pub(super) fn parse_continue_statement(
        &self,
        node: Node,
        _source: &[u8],
    ) -> Option<ContinueStatement> {
        Some(ContinueStatement {
            location: self.node_location(node),
        })
    }
}
