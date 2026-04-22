use tree_sitter::Node;

use super::Parser;
use crate::model::statement::BreakStatement;

impl Parser {
    pub(super) fn parse_break_statement(
        &self,
        node: Node,
        _source: &[u8],
    ) -> Option<BreakStatement> {
        Some(BreakStatement {
            location: self.node_location(node),
        })
    }
}
