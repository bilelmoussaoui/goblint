use tree_sitter::Node;

use crate::{
    model::{CastExpression, Expression, TypeInfo},
    parser::Parser,
};

impl Parser {
    pub(crate) fn parse_cast_expression(&self, node: Node, source: &[u8]) -> Option<Expression> {
        // Get the type node
        let type_node = node.child_by_field_name("type")?;
        let type_text = std::str::from_utf8(&source[type_node.byte_range()])
            .ok()?
            .to_owned();

        // Parse type info from the type text
        let type_location = self.node_location(type_node);
        let type_info = TypeInfo::new(type_text, type_location);

        // Get the value node
        let value_node = node.child_by_field_name("value")?;
        let operand = self.parse_expression(value_node, source)?;

        Some(Expression::Cast(CastExpression {
            type_info,
            operand: Box::new(operand),
            location: self.node_location(node),
        }))
    }
}
