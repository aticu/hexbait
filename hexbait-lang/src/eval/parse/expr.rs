//! Implements evaluation of expressions.

use std::sync::Arc;

use hexbait_common::RelativeOffset;
use num_traits::Zero as _;

use crate::{
    Int,
    compile::ir::{BinOp, ConcatArg, Expr, ExprKind, Lit, UnOp},
    eval::{
        BytesValue, Provenance, Value, ValueKind,
        parse::{
            ParseContext, StaticAnalysisImpossible as _,
            cursor::Cursor,
            diagnostics::{Result, SeekError},
            static_analysis_impossible,
            struct_context::StructContext,
        },
    },
};

impl ParseContext {
    /// Evaluates the given expression.
    pub fn eval_expr(
        &mut self,
        expr: &Expr,
        cursor: &Cursor,
        struct_ctx: &StructContext,
        additional_ctx: AdditionalExprContext,
    ) -> Result<Value> {
        match &expr.kind {
            ExprKind::Lit(lit) => Ok(Value {
                kind: match lit {
                    Lit::Int(int) => ValueKind::Integer(int.clone()),
                    Lit::Bytes(bytes) => ValueKind::Bytes(BytesValue::Lit(Arc::clone(bytes))),
                    Lit::Bool(val) => ValueKind::Boolean(*val),
                },
                provenance: Provenance::empty(),
            }),
            ExprKind::VarUse(var) => Ok(struct_ctx
                .field(&var.inner)
                .static_analysis_expect()
                .clone()),
            ExprKind::Offset => Ok(Value {
                kind: ValueKind::Integer(Int::from(cursor.offset().as_u64())),
                provenance: Provenance::empty(),
            }),
            ExprKind::Parent => Ok(struct_ctx.parent().static_analysis_expect().as_value()),
            ExprKind::Last => Ok(additional_ctx.last.static_analysis_expect().clone()),
            ExprKind::Len => Ok(additional_ctx.len.static_analysis_expect().clone()),
            ExprKind::UnOp { op, operand } => {
                let Value {
                    kind: operand,
                    provenance,
                } = self.eval_expr(operand, cursor, struct_ctx, additional_ctx)?;

                Ok(match op {
                    UnOp::Neg => Value {
                        kind: ValueKind::Integer(-operand.expect_int()),
                        provenance,
                    },
                    UnOp::Plus => Value {
                        kind: operand,
                        provenance,
                    },
                    UnOp::Not => Value {
                        kind: ValueKind::Boolean(!operand.expect_bool()),
                        provenance,
                    },
                })
            }
            ExprKind::BinOp { op, lhs, rhs } => {
                let Value {
                    kind: lhs,
                    mut provenance,
                } = self.eval_expr(lhs, cursor, struct_ctx, additional_ctx)?;

                match op {
                    BinOp::LogicalAnd if !lhs.expect_bool() => {
                        return Ok(Value {
                            kind: ValueKind::Boolean(false),
                            provenance,
                        });
                    }
                    BinOp::LogicalOr if lhs.expect_bool() => {
                        return Ok(Value {
                            kind: ValueKind::Boolean(true),
                            provenance,
                        });
                    }
                    _ => (),
                }

                let Value {
                    kind: rhs,
                    provenance: rhs_provenance,
                } = self.eval_expr(rhs, cursor, struct_ctx, additional_ctx)?;
                provenance += &rhs_provenance;

                enum OpKind {
                    IntOp(fn(&Int, &Int) -> Int),
                    FallibleIntOp(fn(&Int, &Int) -> Result<Int, String>),
                    CmpOp(fn(&Int, &Int) -> bool),
                    Eq,
                    Neq,
                    BoolRhsIdentity,
                }

                let op_kind = match op {
                    BinOp::Add => OpKind::IntOp(|x, y| x + y),
                    BinOp::Sub => OpKind::IntOp(|x, y| x - y),
                    BinOp::Mul => OpKind::IntOp(|x, y| x * y),
                    BinOp::Div => OpKind::FallibleIntOp(|x, y| {
                        if y.is_zero() {
                            Err("division by zero".to_string())
                        } else {
                            Ok(x / y)
                        }
                    }),
                    BinOp::Mod => OpKind::FallibleIntOp(|x, y| {
                        if y.is_zero() {
                            Err("modulo by zero".to_string())
                        } else {
                            Ok(x % y)
                        }
                    }),
                    BinOp::Eq => OpKind::Eq,
                    BinOp::Neq => OpKind::Neq,
                    BinOp::Gt => OpKind::CmpOp(|x, y| x > y),
                    BinOp::Geq => OpKind::CmpOp(|x, y| x >= y),
                    BinOp::Lt => OpKind::CmpOp(|x, y| x < y),
                    BinOp::Leq => OpKind::CmpOp(|x, y| x <= y),
                    BinOp::BitAnd => OpKind::IntOp(|x, y| x & y),
                    BinOp::BitOr => OpKind::IntOp(|x, y| x | y),
                    BinOp::BitXor => OpKind::IntOp(|x, y| x ^ y),
                    BinOp::ShiftLeft => OpKind::FallibleIntOp(|x, y| {
                        u32::try_from(y)
                            .map_err(|_| "shift offset too large".to_string())
                            .map(|y| x << y)
                    }),
                    BinOp::ShiftRight => OpKind::FallibleIntOp(|x, y| {
                        u32::try_from(y)
                            .map_err(|_| "shift offset too large".to_string())
                            .map(|y| x >> y)
                    }),
                    BinOp::LogicalAnd | BinOp::LogicalOr => OpKind::BoolRhsIdentity,
                };

                Ok(match op_kind {
                    OpKind::IntOp(func) => Value {
                        kind: ValueKind::Integer(func(lhs.expect_int(), rhs.expect_int())),
                        provenance,
                    },
                    OpKind::FallibleIntOp(func) => {
                        let value =
                            func(lhs.expect_int(), rhs.expect_int()).map_err(|message| {
                                self.diagnostics
                                    .new_err(message, provenance.clone(), expr.span)
                            })?;

                        Value {
                            kind: ValueKind::Integer(value),
                            provenance,
                        }
                    }
                    OpKind::CmpOp(func) => Value {
                        kind: ValueKind::Boolean(func(lhs.expect_int(), rhs.expect_int())),
                        provenance,
                    },
                    OpKind::Eq => Value {
                        kind: ValueKind::Boolean(lhs == rhs),
                        provenance,
                    },
                    OpKind::Neq => Value {
                        kind: ValueKind::Boolean(lhs != rhs),
                        provenance,
                    },
                    OpKind::BoolRhsIdentity => Value {
                        kind: ValueKind::Boolean(rhs.expect_bool()),
                        provenance,
                    },
                })
            }
            ExprKind::FieldAccess { struct_ref, field } => {
                let struct_ref = struct_ctx.eval_struct_ref(struct_ref, additional_ctx.last);

                Ok(struct_ref.field(&field.inner).clone())
            }
            ExprKind::Peek { ty, offset } => {
                let mut cursor = if let Some(offset_expr) = offset {
                    let offset = self.eval_expr(offset_expr, cursor, struct_ctx, additional_ctx)?;

                    u64::try_from(offset.kind.expect_int())
                        .map_err(|_| SeekError::NegativeOffset)
                        .and_then(|offset| {
                            cursor.child_with_same_view(RelativeOffset::from(offset))
                        })
                        .map_err(|err| {
                            self.diagnostics.seek_err(
                                err,
                                &offset.provenance,
                                offset_expr.span,
                                "during peek",
                            )
                        })?
                } else {
                    // the current cursor is valid
                    cursor
                        .child_with_same_view(cursor.offset())
                        .static_analysis_expect()
                };

                self.eval_parse_type(ty, &mut cursor, struct_ctx)
            }
            ExprKind::Concat { args } => {
                let mut parts = Vec::new();
                let mut provenance = Provenance::empty();

                for arg in args {
                    let (expr, expand) = match arg {
                        ConcatArg::Direct(expr) => (expr, false),
                        ConcatArg::Expanding(expr) => (expr, true),
                    };

                    let expr = self.eval_expr(expr, cursor, struct_ctx, additional_ctx)?;

                    provenance += &expr.provenance;
                    if expand {
                        parts.extend(
                            expr.kind
                                .expect_array_take()
                                .into_iter()
                                .map(|val| val.kind.expect_bytes_take()),
                        );
                    } else {
                        parts.push(expr.kind.expect_bytes_take());
                    }
                }

                Ok(Value {
                    kind: ValueKind::Bytes(BytesValue::Concat { parts }),
                    provenance,
                })
            }
            ExprKind::Error => static_analysis_impossible(),
        }
    }
}

/// Additional context that can be used during expression evaluation.
#[derive(Debug, Default, Clone, Copy)]
pub struct AdditionalExprContext<'parent> {
    /// The last parsed value in the current repeat expression.
    pub last: Option<&'parent Value>,
    /// The length of the current repeat expression.
    pub len: Option<&'parent Value>,
}
