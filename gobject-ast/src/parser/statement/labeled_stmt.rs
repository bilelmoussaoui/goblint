use tree_sitter::Node;

use crate::{model::LabeledStatement, parser::Parser};

impl Parser {
    pub(crate) fn parse_labeled_statement(
        &self,
        node: Node,
        source: &[u8],
    ) -> Option<LabeledStatement> {
        let label_node = node.child_by_field_name("label")?;
        let label = std::str::from_utf8(&source[label_node.byte_range()])
            .ok()?
            .to_owned();

        // The statement is the named child after the label and ":".
        // Comments can appear between the colon and the statement, so keep
        // trying named children until one successfully parses.
        let mut cursor = node.walk();
        let mut statement = None;
        for child in node.children(&mut cursor) {
            if child.kind() == "statement_identifier" || child.kind() == ":" {
                continue;
            }
            if child.is_named() {
                if let Some(s) = self.parse_statement(child, source) {
                    statement = Some(s);
                    break;
                }
            }
        }

        Some(LabeledStatement {
            label,
            statement: Box::new(statement?),
            location: self.node_location(node),
        })
    }
}
