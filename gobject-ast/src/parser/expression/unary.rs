use tree_sitter::Node;

use crate::{model::{UnaryExpression, UnaryOp}, parser::Parser};

impl Parser {
    pub(crate) fn parse_unary_expression(
        &self,
        node: Node,
        source: &[u8],
    ) -> Option<UnaryExpression> {
        let operator_node = node.child_by_field_name("operator")?;
        let operator_str = std::str::from_utf8(&source[operator_node.byte_range()]).ok()?;
        let operator = UnaryOp::from_str(operator_str)?;

        let operand_node = node.child_by_field_name("argument")?;
        let operand = self.parse_expression(operand_node, source)?;

        Some(UnaryExpression {
            operator,
            operand: Box::new(operand),
            location: self.node_location(node),
        })
    }
}
