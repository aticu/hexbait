//! Implements expressions in the AST.

use crate::parsing::language::Int;

use super::{Node, Symbol};

/// Represents an expression in the AST.
#[derive(Debug, Clone)]
pub struct Expr {
    /// The kind of the expression.
    pub kind: ExprKind,
}

/// The type of unary operation to perform.
#[derive(Debug, Clone)]
pub enum UnOp {
    /// Logical negation.
    Not,
}

/// The type of binary operation to perform.
#[derive(Debug, Clone)]
pub enum BinOp {
    /// Perform an equality check.
    Eq,
    /// Perform a negated equality check.
    Neq,
    /// Perform a check whether the left operand is greater than the right operand.
    Gt,
    /// Perform a check whether the left operand is greater than or equal to the right operand.
    Geq,
    /// Perform a check whether the left operand is less than the right operand.
    Lt,
    /// Perform a check whether the left operand is less than or equal to the right operand.
    Leq,
    /// Perform addition.
    Add,
    /// Perform subtraction.
    Sub,
    /// Perform multiplication.
    Mul,
    /// Perform division.
    Div,
    /// Perform a modulo operation.
    Mod,
    /// Perform a logical or bitwise AND operation.
    And,
    /// Perform a logical or bitwise OR operation.
    Or,
}

/// The kind of an expression.
#[derive(Debug, Clone)]
pub enum ExprKind {
    /// The expression is a constant integer value.
    ConstantInt {
        /// The value of the constant integer.
        value: Int,
    },
    /// The expression is a constant byte slice.
    ConstantBytes {
        /// The value of the constant byte slice.
        value: Vec<u8>,
    },
    /// The expression refers to the current offset.
    Offset,
    /// The currently parsed parent context.
    Parent,
    /// The last parsed entry in a repetition.
    Last,
    /// The expression is a unary operation.
    UnOp {
        /// The operand of the expression.
        operand: Box<Expr>,
        /// The operator of the expression.
        op: UnOp,
    },
    /// The expression is a binary operation.
    BinOp {
        /// The left operand of the expression.
        left: Box<Expr>,
        /// The right operand of the expression.
        right: Box<Expr>,
        /// The operator of the expression.
        op: BinOp,
    },
    /// The expression is a use of a variable.
    VariableUse {
        /// The variable references by the expression.
        var: Symbol,
    },
    /// The expression accesses a field.
    FieldAccess {
        /// The expression resolving to the holder of the field.
        field_holder: Box<Expr>,
        /// The field that is being held.
        field: Symbol,
    },
    /// Parses the given node without modifying the offset.
    ParseAt {
        /// The node to parse.
        node: Box<Node>,
    },
    /// A conditional expression.
    If {
        /// The condition that determines the branch to take.
        condition: Box<Expr>,
        /// The expression that is computed if the condition is true.
        true_branch: Box<Expr>,
        /// The expression that is computed if the condition is false.
        false_branch: Box<Expr>,
    },
}

impl Expr {
    /// Whether this expression contains a `last` expression.
    pub fn contains_last(&self) -> bool {
        match &self.kind {
            ExprKind::ConstantInt { .. }
            | ExprKind::ConstantBytes { .. }
            | ExprKind::Offset
            | ExprKind::Parent
            | ExprKind::VariableUse { .. }
            | ExprKind::ParseAt { .. } => false,
            ExprKind::Last => true,
            ExprKind::UnOp { operand, .. } => operand.contains_last(),
            ExprKind::BinOp { left, right, .. } => left.contains_last() || right.contains_last(),
            ExprKind::FieldAccess { field_holder, .. } => field_holder.contains_last(),
            ExprKind::If {
                condition,
                true_branch,
                false_branch,
            } => {
                condition.contains_last()
                    || true_branch.contains_last()
                    || false_branch.contains_last()
            }
        }
    }
}
