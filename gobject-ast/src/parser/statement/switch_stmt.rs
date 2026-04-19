use tree_sitter::Node;

use crate::{
    model::{CaseLabel, SwitchStatement},
    parser::Parser,
};

impl Parser {
    pub(crate) fn parse_switch_statement(
        &self,
        node: Node,
        source: &[u8],
    ) -> Option<SwitchStatement> {
        let condition_node = node.child(1)?; // parenthesized_expression
        // Parse the expression inside the parentheses
        let inner_expr = condition_node.child(1).or(Some(condition_node))?;
        let condition = self.parse_expression(inner_expr, source)?;
        let condition_location = self.node_location(inner_expr);

        let body_node = node.child(2)?; // compound_statement
        let body = self.parse_function_body(body_node, source);

        // Extract case labels from the body
        let cases = self.extract_case_labels(body_node, source);

        Some(SwitchStatement {
            condition,
            condition_location,
            cases,
            body,
            location: self.node_location(node),
        })
    }

    /// Extract case labels from a switch statement body
    fn extract_case_labels(&self, body_node: Node, source: &[u8]) -> Vec<CaseLabel> {
        let mut cases = Vec::new();
        let mut cursor = body_node.walk();

        for child in body_node.children(&mut cursor) {
            if child.kind() == "case_statement" {
                // Check if it's a default case or regular case
                let first_child = child.child(0);
                if let Some(first) = first_child {
                    let is_default = first.kind() == "default";

                    if is_default {
                        // Default case has no value
                        cases.push(CaseLabel {
                            value: None,
                            location: self.node_location(child),
                        });
                    } else {
                        // Regular case: child 1 is the value expression
                        if let Some(value_node) = child.child(1) {
                            if let Some(value) = self.parse_expression(value_node, source) {
                                cases.push(CaseLabel {
                                    value: Some(value),
                                    location: self.node_location(child),
                                });
                            }
                        }
                    }
                }
            }
        }

        cases
    }
}
