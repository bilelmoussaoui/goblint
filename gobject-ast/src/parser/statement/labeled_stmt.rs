use tree_sitter::Node;

use crate::model::LabeledStatement;
use crate::parser::Parser;

impl Parser {
    pub(crate) fn parse_labeled_statement(&self, node: Node, source: &[u8]) -> Option<LabeledStatement> {
        let label_node = node.child_by_field_name("label")?;
        let label = std::str::from_utf8(&source[label_node.byte_range()])
            .ok()?
            .to_owned();

        // Get the statement after the label
        let mut cursor = node.walk();
        let mut statement = None;
        for child in node.children(&mut cursor) {
            if child.kind() != "label" && child.kind() != ":" && child.is_named() {
                statement = self.parse_statement(child, source);
                break;
            }
        }

        Some(LabeledStatement {
            label,
            statement: Box::new(statement?),
            location: self.node_location(node),
        })
    }
}
