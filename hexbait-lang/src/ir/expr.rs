//! Implements expressions in the IR.

use std::sync::Arc;

use crate::{Int, span::Span};

use super::{ParseType, Spanned, Symbol};

/// A literal expression.
#[derive(Debug)]
pub enum Lit {
    /// An integer literal.
    Int(Int),
    /// A bytes literal.
    Bytes(Arc<[u8]>),
    /// A boolean literal.
    Bool(bool),
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
    /// The logical and operator: `&&`.
    LogicalAnd,
    /// The logical or operator: `||`.
    LogicalOr,
    /// The bitwise and operator: `&`.
    BitAnd,
    /// The bitwise or operator: `|`.
    BitOr,
    /// The bitwise xor operator: `^`.
    BitXor,
    /// The shift left operator: `<<`.
    ShiftLeft,
    /// The shift right operator: `>>`.
    ShiftRight,
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
    /// The current parsing offset.
    Offset,
    /// Accesses the partially parsed parent node.
    Parent,
    /// The last parsed element in a repeating expression.
    Last,
    /// The current length of the element in a repeating expression.
    Len,
    /// A field access expression.
    FieldAccess {
        /// The expression of which the field will be accessed.
        expr: Box<Expr>,
        /// The field to access.
        field: Spanned<Symbol>,
    },
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
    /// A `peek` expression that parses a value from the underlying view.
    Peek {
        /// The type to parse.
        ty: Box<ParseType>,
        /// Where to parse the given type.
        offset: Option<Box<Expr>>,
    },
    /// An expression that contained an error during parsing.
    Error,
}
