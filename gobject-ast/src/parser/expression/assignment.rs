use tree_sitter::Node;

use crate::{
    model::{Assignment, AssignmentOp},
    parser::Parser,
};

impl Parser {
    pub(crate) fn parse_assignment(&self, node: Node, source: &[u8]) -> Option<Assignment> {
        let left_node = node.child_by_field_name("left")?;
        let lhs = std::str::from_utf8(&source[left_node.byte_range()])
            .ok()?
            .to_owned();

        let operator_node = node.child_by_field_name("operator")?;
        let operator_str = std::str::from_utf8(&source[operator_node.byte_range()]).ok()?;
        let operator = AssignmentOp::from_str(operator_str)?;

        let right_node = node.child_by_field_name("right")?;
        let rhs = self.parse_expression(right_node, source)?;

        Some(Assignment {
            lhs,
            operator,
            rhs: Box::new(rhs),
            location: self.node_location(node),
        })
    }
}
