use tree_sitter::Node;

use crate::{
    model::{Expression, UpdateExpression, UpdateOp},
    parser::Parser,
};

impl Parser {
    pub(crate) fn parse_update_expression(&self, node: Node, source: &[u8]) -> Option<Expression> {
        let operator_node = node.child_by_field_name("operator")?;
        let operator_str = std::str::from_utf8(&source[operator_node.byte_range()]).ok()?;
        let operator = UpdateOp::from_str(operator_str)?;

        let argument_node = node.child_by_field_name("argument")?;
        let operand = self.parse_expression(argument_node, source)?;

        // Determine if prefix or postfix based on node positions
        let is_prefix = operator_node.start_byte() < argument_node.start_byte();

        Some(Expression::Update(UpdateExpression {
            operator,
            operand: Box::new(operand),
            is_prefix,
            location: self.node_location(node),
        }))
    }
}
