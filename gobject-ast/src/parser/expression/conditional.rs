use tree_sitter::Node;

use crate::model::{ConditionalExpression, Expression};
use crate::parser::Parser;

impl Parser {
    pub(crate) fn parse_conditional_expression(&self, node: Node, source: &[u8]) -> Option<Expression> {
        let condition_node = node.child_by_field_name("condition")?;
        let condition = self.parse_expression(condition_node, source)?;

        let consequence_node = node.child_by_field_name("consequence")?;
        let then_expr = self.parse_expression(consequence_node, source)?;

        let alternative_node = node.child_by_field_name("alternative")?;
        let else_expr = self.parse_expression(alternative_node, source)?;

        Some(Expression::Conditional(ConditionalExpression {
            condition: Box::new(condition),
            then_expr: Box::new(then_expr),
            else_expr: Box::new(else_expr),
            location: self.node_location(node),
        }))
    }
}
