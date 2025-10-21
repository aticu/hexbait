//! Implements expressions in the AST.

use std::fmt;

use crate::parsing::language::Int;

use super::{Node, Symbol};

/// Represents an expression in the AST.
#[derive(Clone)]
pub struct Expr {
    /// The kind of the expression.
    pub kind: ExprKind,
}

impl fmt::Debug for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.kind.fmt(f)
    }
}

/// The type of unary operation to perform.
#[derive(Debug, Clone)]
pub enum UnOp {
    /// Logical negation.
    Not,
}

/// The type of binary operation to perform.
#[derive(Clone)]
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

impl fmt::Debug for BinOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Eq => write!(f, "=="),
            Self::Neq => write!(f, "!="),
            Self::Gt => write!(f, ">"),
            Self::Geq => write!(f, ">="),
            Self::Lt => write!(f, "<"),
            Self::Leq => write!(f, "<="),
            Self::Add => write!(f, "+"),
            Self::Sub => write!(f, "-"),
            Self::Mul => write!(f, "*"),
            Self::Div => write!(f, "/"),
            Self::Mod => write!(f, "%"),
            Self::And => write!(f, "&"),
            Self::Or => write!(f, "|"),
        }
    }
}

/// The kind of an expression.
#[derive(Clone)]
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

impl fmt::Debug for ExprKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConstantInt { value } => write!(f, "{value}"),
            Self::ConstantBytes { value } => {
                let needs_escape =
                    |byte: &u8| !byte.is_ascii_graphic() && !b"\n\r\t ".contains(&byte);
                let any_needs_escape = value.iter().any(needs_escape);

                if any_needs_escape {
                    write!(f, "<")?;
                }

                let mut in_str = None;
                for byte in value {
                    if needs_escape(byte) {
                        match in_str {
                            None => (),
                            Some(true) => write!(f, "\" ")?,
                            Some(false) => write!(f, " ")?,
                        }
                        in_str = Some(false);

                        write!(f, "{byte:02x}")?;
                    } else {
                        match in_str {
                            None => write!(f, "\"")?,
                            Some(true) => (),
                            Some(false) => write!(f, " \"")?,
                        }
                        in_str = Some(true);

                        match byte {
                            b'\n' => write!(f, "\\n")?,
                            b'\r' => write!(f, "\\r")?,
                            b'\t' => write!(f, "\\t")?,
                            b' ' => write!(f, " ")?,
                            _ => write!(f, "{}", *byte as char)?,
                        }
                    }
                }
                if in_str.unwrap_or(false) {
                    write!(f, "\"")?;
                }

                if any_needs_escape {
                    write!(f, ">")?;
                }

                Ok(())
            }
            Self::Offset => write!(f, "Offset"),
            Self::Parent => write!(f, "Parent"),
            Self::Last => write!(f, "Last"),
            Self::UnOp {
                operand,
                op: UnOp::Not,
            } => write!(f, "-{operand:?}"),
            Self::BinOp { left, right, op } => write!(f, "{left:?} {op:?} {right:?}"),
            Self::VariableUse { var } => write!(f, "{var:?}"),
            Self::FieldAccess {
                field_holder,
                field,
            } => write!(f, "{field_holder:?}.{field:?}"),
            Self::ParseAt { node } => write!(f, "parse({node:?})"),
            Self::If {
                condition,
                true_branch,
                false_branch,
            } => write!(
                f,
                "if ({condition:?}) {{ {true_branch:?} }} else {{ {false_branch:?} }}"
            ),
        }
    }
}
