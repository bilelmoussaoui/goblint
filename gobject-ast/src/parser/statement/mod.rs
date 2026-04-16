mod expression_stmt;
mod if_stmt;
mod return_stmt;
mod goto_stmt;
mod labeled_stmt;
mod compound_stmt;
mod variable_decl;

use tree_sitter::Node;

use crate::model::Statement;
use super::Parser;

impl Parser {
    pub(super) fn parse_statement(&self, node: Node, source: &[u8]) -> Option<Statement> {
        match node.kind() {
            "declaration" => {
                // Variable declaration
                self.parse_variable_decl(node, source)
                    .map(Statement::Declaration)
            }
            "expression_statement" => {
                // Expression like function call, assignment, etc.
                self.parse_expression_stmt(node, source)
                    .map(Statement::Expression)
            }
            "if_statement" => self.parse_if_statement(node, source).map(Statement::If),
            "return_statement" => self
                .parse_return_statement(node, source)
                .map(Statement::Return),
            "goto_statement" => self.parse_goto_statement(node, source).map(Statement::Goto),
            "labeled_statement" => self
                .parse_labeled_statement(node, source)
                .map(Statement::Labeled),
            "compound_statement" => self
                .parse_compound_statement(node, source)
                .map(Statement::Compound),
            _ => None,
        }
    }
}
