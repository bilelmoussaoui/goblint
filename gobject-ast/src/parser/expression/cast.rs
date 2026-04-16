use tree_sitter::Node;

use crate::model::{CastExpression, Expression};
use crate::parser::Parser;

impl Parser {
    pub(crate) fn parse_cast_expression(&self, node: Node, source: &[u8]) -> Option<Expression> {
        // Get the type node
        let type_node = node.child_by_field_name("type")?;
        let type_name = std::str::from_utf8(&source[type_node.byte_range()])
            .ok()?
            .to_owned();

        // Get the value node
        let value_node = node.child_by_field_name("value")?;
        let operand = self.parse_expression(value_node, source)?;

        Some(Expression::Cast(CastExpression {
            type_name,
            operand: Box::new(operand),
            location: self.node_location(node),
        }))
    }
}
