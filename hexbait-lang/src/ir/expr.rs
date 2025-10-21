//! Implements expressions in the IR.

use crate::{Int, span::Span};

use super::{Spanned, Symbol};

/// A literal expression.
#[derive(Debug)]
pub enum Lit {
    /// An integer literal.
    Int(Int),
    /// A bytes literal.
    Bytes(Vec<u8>),
}

/// A unary operator.
#[derive(Debug)]
pub enum UnOp {
    /// The negation operator: `-`.
    Neg,
    /// The plus operator: `+`.
    ///
    /// This is a no-op.
    Plus,
    /// The not operator: `!`.
    Not,
}

/// A binary operator.
#[derive(Debug)]
pub enum BinOp {
    /// The addition operator: `+`.
    Add,
    /// The subtraction operator: `-`.
    Sub,
    /// The multiplication operator: `*`.
    Mul,
    /// The division operator: `/`.
    Div,
    /// The equality operator: `==`.
    Eq,
    /// The inequality operator: `!=`.
    Neq,
    /// The greater than operator: `>`.
    Gt,
    /// The greater than or equals operator: `>=`.
    Geq,
    /// The less than operator: `<`.
    Lt,
    /// The less than or equal operator: `<=`.
    Leq,
}

/// An expression.
#[derive(Debug)]
pub struct Expr {
    /// The kind of the expression.
    pub kind: ExprKind,
    /// The span of the expression.
    pub span: Span,
}

/// The different kinds of expressions.
#[derive(Debug)]
pub enum ExprKind {
    /// A literal expression.
    Lit(Lit),
    /// A use of a variable.
    VarUse(Spanned<Symbol>),
    /// A unary operator expression.
    UnOp {
        /// The operator.
        op: UnOp,
        /// The operand of this expression.
        operand: Box<Expr>,
    },
    /// A binary operator expression.
    BinOp {
        /// The operator.
        op: BinOp,
        /// The left hand side of the operation.
        lhs: Box<Expr>,
        /// The left hand side of the operation.
        rhs: Box<Expr>,
    },
    /// An expression that contained an error during parsing.
    Error,
}
