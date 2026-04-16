use tree_sitter::Node;

use crate::{
    model::{BinaryExpression, BinaryOp},
    parser::Parser,
};

impl Parser {
    pub(crate) fn parse_binary_expression(
        &self,
        node: Node,
        source: &[u8],
    ) -> Option<BinaryExpression> {
        let left_node = node.child_by_field_name("left")?;
        let left = self.parse_expression(left_node, source)?;

        let operator_node = node.child_by_field_name("operator")?;
        let operator_str = std::str::from_utf8(&source[operator_node.byte_range()]).ok()?;
        let operator = BinaryOp::from_str(operator_str)?;

        let right_node = node.child_by_field_name("right")?;
        let right = self.parse_expression(right_node, source)?;

        Some(BinaryExpression {
            left: Box::new(left),
            operator,
            right: Box::new(right),
            location: self.node_location(node),
        })
    }
}
