use tree_sitter::Node;

use crate::model::{Expression, SubscriptExpression};
use crate::parser::Parser;

impl Parser {
    pub(crate) fn parse_subscript_expression(&self, node: Node, source: &[u8]) -> Option<Expression> {
        let argument_node = node.child_by_field_name("argument")?;
        let array = self.parse_expression(argument_node, source)?;

        let index_node = node.child_by_field_name("index")?;
        let index = self.parse_expression(index_node, source)?;

        Some(Expression::Subscript(SubscriptExpression {
            array: Box::new(array),
            index: Box::new(index),
            location: self.node_location(node),
        }))
    }
}
