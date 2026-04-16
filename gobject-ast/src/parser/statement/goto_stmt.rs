use tree_sitter::Node;

use crate::model::GotoStatement;
use crate::parser::Parser;

impl Parser {
    pub(crate) fn parse_goto_statement(&self, node: Node, source: &[u8]) -> Option<GotoStatement> {
        let label_node = node.child_by_field_name("label")?;
        let label = std::str::from_utf8(&source[label_node.byte_range()])
            .ok()?
            .to_owned();

        Some(GotoStatement {
            label,
            location: self.node_location(node),
        })
    }
}
