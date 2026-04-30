use tree_sitter::Node;

use crate::{model::IfStatement, parser::Parser};

impl Parser {
    pub(crate) fn parse_if_statement(&self, node: Node, source: &[u8]) -> Option<IfStatement> {
        let condition_node = node.child_by_field_name("condition")?;
        let condition = self.parse_expression(condition_node, source)?;

        let consequence_node = node.child_by_field_name("consequence")?;
        let then_has_braces = consequence_node.kind() == "compound_statement";
        let then_body = if then_has_braces {
            self.parse_function_body(consequence_node, source)
        } else {
            // Single statement
            self.parse_statement(consequence_node, source)
                .map(|s| vec![s])
                .unwrap_or_default()
        };

        let else_body = node.child_by_field_name("alternative").map(|alt_node| {
            // The alternative can be an else_clause or directly an if_statement (else if)
            let statement_node = if alt_node.kind() == "else_clause" {
                // else_clause contains the actual statement(s)
                // Find the first named child that's not a comment or the 'else' keyword
                let mut cursor = alt_node.walk();
                alt_node
                    .children(&mut cursor)
                    .find(|c| c.is_named() && c.kind() != "comment" && c.kind() != "else")
                    .unwrap_or(alt_node)
            } else {
                alt_node
            };

            if statement_node.kind() == "compound_statement" {
                self.parse_function_body(statement_node, source)
            } else {
                self.parse_statement(statement_node, source)
                    .map(|s| vec![s])
                    .unwrap_or_default()
            }
        });

        Some(IfStatement {
            condition,
            then_body,
            then_has_braces,
            else_body,
            location: self.node_location(node),
        })
    }
}
