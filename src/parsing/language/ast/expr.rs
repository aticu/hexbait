//! Implements expressions in the AST.

use crate::parsing::language::Int;

use super::Symbol;

/// Represents an expression in the AST.
#[derive(Debug)]
pub struct Expr {
    /// The kind of the expression.
    pub kind: ExprKind,
}

/// The type of binary operation to perform.
#[derive(Debug)]
pub enum BinOp {
    /// Perform addition.
    Add,
    /// Perform subtraction.
    Sub,
}

/// The kind of an expression.
#[derive(Debug)]
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
}
