use tree_sitter::Node;

use crate::{
    model::{CaseLabel, SwitchCase, SwitchStatement},
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

        // Parse cases with their grouped statement bodies
        let cases = self.parse_switch_cases(body_node, source);

        Some(SwitchStatement {
            condition,
            condition_location,
            cases,
            location: self.node_location(node),
        })
    }

    /// Parse switch cases, grouping statements with their case labels
    fn parse_switch_cases(&self, body_node: Node, source: &[u8]) -> Vec<SwitchCase> {
        let mut cases = Vec::new();
        let mut cursor = body_node.walk();
        let mut current_case: Option<(CaseLabel, Vec<crate::model::Statement>)> = None;

        for child in body_node.children(&mut cursor) {
            if child.kind() == "case_statement" {
                // Save the previous case if it exists
                if let Some((label, body)) = current_case.take() {
                    cases.push(SwitchCase { label, body });
                }

                // Parse the new case label
                let first_child = child.child(0);
                if let Some(first) = first_child {
                    let is_default = first.kind() == "default";

                    let label = if is_default {
                        CaseLabel {
                            value: None,
                            location: self.node_location(child),
                        }
                    } else {
                        // Regular case: child 1 is the value expression
                        if let Some(value_node) = child.child(1) {
                            if let Some(value) = self.parse_expression(value_node, source) {
                                CaseLabel {
                                    value: Some(value),
                                    location: self.node_location(child),
                                }
                            } else {
                                continue;
                            }
                        } else {
                            continue;
                        }
                    };

                    // Parse statements in this case_statement node
                    let mut case_body = Vec::new();
                    let mut case_cursor = child.walk();
                    for case_child in child.children(&mut case_cursor) {
                        if case_child.is_named()
                            && case_child.kind() != "case"
                            && case_child.kind() != "default"
                            && case_child.kind() != ":"
                            && !case_child.kind().ends_with("_expression")
                        {
                            if let Some(stmt) = self.parse_statement(case_child, source) {
                                case_body.push(stmt);
                            }
                        }
                    }

                    current_case = Some((label, case_body));
                }
            } else if let Some((_, ref mut body)) = current_case {
                // Statement belongs to the current case
                if child.is_named() && child.kind() != "{" && child.kind() != "}" {
                    if let Some(stmt) = self.parse_statement(child, source) {
                        body.push(stmt);
                    }
                }
            }
        }

        // Don't forget the last case
        if let Some((label, body)) = current_case {
            cases.push(SwitchCase { label, body });
        }

        cases
    }
}
