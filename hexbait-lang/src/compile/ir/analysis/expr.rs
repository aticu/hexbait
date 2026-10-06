//! Implements analysis of expressions.

use num_traits::{Signed as _, Zero as _};

use crate::compile::{
    Diagnostic, Span,
    diagnostics::Label,
    ir::{
        BinOp, ConcatArg, Expr, ExprKind, Lit, ParseType, Spanned, StructRef, StructRefPart,
        Symbol, UnOp,
        analysis::{
            AnalysisCtx, Env,
            ty::{Availability, StructTy, Ty, TyKind},
        },
    },
};

/// Additional expression checking context.
pub struct ExtraExprCtx {
    /// The type of the currently repeated element in a repetition.
    while_repetition_ty: Option<Ty>,
}

impl ExtraExprCtx {
    /// No extra expression context.
    pub fn none() -> ExtraExprCtx {
        ExtraExprCtx {
            while_repetition_ty: None,
        }
    }

    /// The extra expression context within a while condition.
    pub fn in_while_condition(element_ty: Ty) -> ExtraExprCtx {
        ExtraExprCtx {
            while_repetition_ty: Some(element_ty),
        }
    }
}

impl Env<'_> {
    /// Checks the given expression, returning its type.
    pub fn check_expr(&self, ctx: &mut AnalysisCtx, expr: &Expr, extra_ctx: &ExtraExprCtx) -> Ty {
        let kind = match &expr.kind {
            ExprKind::Lit(lit) => self.check_lit(lit),
            ExprKind::Offset => self.check_offset(),
            ExprKind::Last => self.check_last(ctx, expr.span, extra_ctx),
            ExprKind::Len => self.check_len(ctx, expr.span, extra_ctx),
            ExprKind::FieldAccess { struct_ref, field } => {
                self.check_field_access(ctx, struct_ref, field, extra_ctx)
            }
            ExprKind::UnOp { op, operand } => self.check_unop(ctx, op, operand, extra_ctx),
            ExprKind::BinOp { op, lhs, rhs } => {
                self.check_binop(ctx, op, lhs, rhs, expr.span, extra_ctx)
            }
            ExprKind::Peek { ty, offset } => self.check_peek(ctx, ty, offset, extra_ctx),
            ExprKind::Concat { args } => self.check_concat(ctx, args, extra_ctx),
            ExprKind::Error => {
                assert!(ctx.diagnostics.contains_errors());
                TyKind::Error
            }
        };

        Ty {
            kind,
            span: expr.span,
        }
    }

    /// Checks the given literal expression.
    pub fn check_lit(&self, lit: &Lit) -> TyKind {
        match lit {
            Lit::Int(big_int) => TyKind::Int {
                non_zero: !big_int.is_zero(),
                non_negative: !big_int.is_negative(),
            },
            Lit::Bytes(_) => TyKind::Bytes,
            Lit::Bool(_) => TyKind::Bool,
        }
    }

    /// Checks an offset expression.
    fn check_offset(&self) -> TyKind {
        TyKind::Int {
            non_zero: false,
            non_negative: true,
        }
    }

    /// Checks the given `$last` expression.
    fn check_last(
        &self,
        ctx: &mut AnalysisCtx,
        expr_span: Span,
        extra_ctx: &ExtraExprCtx,
    ) -> TyKind {
        if let Some(element_ty) = &extra_ctx.while_repetition_ty {
            element_ty.kind.clone()
        } else {
            ctx.add_diagnostic(Diagnostic::error(
                "`$last` cannot be used outside of a `while` repetition condition",
                Label::new("invalid use of `$last`", expr_span),
            ));
            TyKind::Error
        }
    }

    /// Checks the given `$len` expression.
    fn check_len(
        &self,
        ctx: &mut AnalysisCtx,
        expr_span: Span,
        extra_ctx: &ExtraExprCtx,
    ) -> TyKind {
        if extra_ctx.while_repetition_ty.is_some() {
            TyKind::Int {
                non_zero: false,
                non_negative: true,
            }
        } else {
            ctx.add_diagnostic(Diagnostic::error(
                "`$len` cannot be used outside of a `while` repetition condition",
                Label::new("invalid use of `$len`", expr_span),
            ));
            TyKind::Error
        }
    }

    /// Checks the given field access expression.
    fn check_field_access(
        &self,
        ctx: &mut AnalysisCtx,
        struct_ref: &StructRef,
        field: &Spanned<Symbol>,
        extra_ctx: &ExtraExprCtx,
    ) -> TyKind {
        enum StructRef<'parent> {
            Env(&'parent Env<'parent>),
            Finished { value: &'parent StructTy },
        }

        let mut current = StructRef::Env(self);
        for (i, part) in struct_ref.parts.iter().enumerate() {
            current = match &part.inner {
                StructRefPart::Parent => match current {
                    StructRef::Env(env) => {
                        if let Some(parent) = env.parent {
                            StructRef::Env(parent)
                        } else {
                            ctx.add_diagnostic(Diagnostic::error(
                                "`$parent` used when there is no more parent",
                                Label::new("no parent here", part.span),
                            ));
                            return TyKind::Error;
                        }
                    }
                    StructRef::Finished { .. } => {
                        ctx.add_diagnostic(Diagnostic::error(
                            "cannot access parent of a finished struct",
                            Label::new("cannot access parent", part.span),
                        ));
                        return TyKind::Error;
                    }
                },
                StructRefPart::Last => match &extra_ctx.while_repetition_ty {
                    Some(ty) => {
                        if i == 0 {
                            if let TyKind::Struct { struct_ty } = &ty.kind {
                                StructRef::Finished { value: struct_ty }
                            } else {
                                ctx.add_diagnostic(Diagnostic::error(
                                    "field access on something that is not a `struct`",
                                    Label::new("cannot access field", part.span),
                                ));
                                return TyKind::Error;
                            }
                        } else {
                            ctx.add_diagnostic(Diagnostic::error(
                                "`$last` must be the top-level reference",
                                Label::new("cannot be a subrefence", part.span),
                            ));
                            return TyKind::Error;
                        }
                    }
                    None => {
                        ctx.add_diagnostic(Diagnostic::error(
                            "`$last` is inaccesible outside of `while` repetition conditions",
                            Label::new("`$last` is undefined here", part.span),
                        ));
                        return TyKind::Error;
                    }
                },
                StructRefPart::Named(name) => {
                    let value = match current {
                        StructRef::Env(env) => &env.values,
                        StructRef::Finished { value } => value,
                    };

                    let Some(field) = value.field(name) else {
                        ctx.add_diagnostic(Diagnostic::error(
                            "cannot access non-existant field",
                            Label::new("field does not exist", part.span),
                        ));
                        return TyKind::Error;
                    };

                    match field.availability {
                        Availability::Guaranteed => (),
                        Availability::Conditional {
                            defined_at,
                            undefined_at,
                            undefined_is_missing_else,
                        } => {
                            ctx.add_diagnostic(
                                Diagnostic::error(
                                    format!(
                                        "field `{}` is not guaranteed to be available",
                                        name.as_str()
                                    ),
                                    Label::new("not guaranteed to be available", part.span),
                                )
                                .with_label(Label::new("defined here", defined_at))
                                .with_label(Label::new(
                                    if undefined_is_missing_else {
                                        "not defined if this is `false`"
                                    } else {
                                        "not defined in this branch"
                                    },
                                    undefined_at,
                                )),
                            );
                        }
                    }

                    if let TyKind::Struct { struct_ty } = &field.ty.kind {
                        StructRef::Finished { value: struct_ty }
                    } else {
                        ctx.add_diagnostic(Diagnostic::error(
                            "field access on something that is not a `struct`",
                            Label::new("cannot access field", part.span),
                        ));
                        return TyKind::Error;
                    }
                }
            };
        }

        let value = match current {
            StructRef::Env(env) => &env.values,
            StructRef::Finished { value } => value,
        };

        let Some(resolved_field) = value.field(&field.inner) else {
            ctx.add_diagnostic(Diagnostic::error(
                "cannot access non-existant field",
                Label::new("field does not exist", field.span),
            ));
            return TyKind::Error;
        };

        match resolved_field.availability {
            Availability::Guaranteed => (),
            Availability::Conditional {
                defined_at,
                undefined_at,
                undefined_is_missing_else,
            } => {
                ctx.add_diagnostic(
                    Diagnostic::error(
                        format!(
                            "field `{}` is not guaranteed to be available",
                            field.inner.as_str()
                        ),
                        Label::new("not guaranteed to be available", field.span),
                    )
                    .with_label(Label::new("defined here", defined_at))
                    .with_label(Label::new(
                        if undefined_is_missing_else {
                            "not defined if this is `false`"
                        } else {
                            "not defined in this branch"
                        },
                        undefined_at,
                    )),
                );
            }
        }

        resolved_field.ty.kind.clone()
    }

    /// Checks the given unary operation.
    fn check_unop(
        &self,
        ctx: &mut AnalysisCtx,
        op: &UnOp,
        operand: &Expr,
        extra_ctx: &ExtraExprCtx,
    ) -> TyKind {
        let op_ty = self.check_expr(ctx, operand, extra_ctx);
        if op_ty.contains_err() {
            return TyKind::Error;
        }

        match op {
            UnOp::Neg => match &op_ty.kind {
                TyKind::Int {
                    non_zero,
                    non_negative: _,
                } => TyKind::Int {
                    non_zero: *non_zero,
                    non_negative: false,
                },
                _ => ctx.ty_err(
                    "unary operator `-` can only be applied to integers",
                    &op_ty,
                    operand.span,
                ),
            },
            UnOp::Plus => match &op_ty.kind {
                TyKind::Int {
                    non_zero,
                    non_negative,
                } => TyKind::Int {
                    non_zero: *non_zero,
                    non_negative: *non_negative,
                },
                _ => ctx.ty_err(
                    "unary operator `+` can only be applied to integers",
                    &op_ty,
                    operand.span,
                ),
            },
            UnOp::Not => match &op_ty.kind {
                TyKind::Bool => TyKind::Bool,
                _ => ctx.ty_err(
                    "unary operator `!` can only be applied to bools",
                    &op_ty,
                    operand.span,
                ),
            },
        }
    }

    /// Checks the given binary operation.
    fn check_binop(
        &self,
        ctx: &mut AnalysisCtx,
        op: &BinOp,
        lhs: &Expr,
        rhs: &Expr,
        expr_span: Span,
        extra_ctx: &ExtraExprCtx,
    ) -> TyKind {
        let lhs_ty = self.check_expr(ctx, lhs, extra_ctx);
        let rhs_ty = self.check_expr(ctx, rhs, extra_ctx);
        if lhs_ty.contains_err() || rhs_ty.contains_err() {
            return TyKind::Error;
        }

        macro_rules! both_sides_match {
            (
                 $ty_kind:literal: ($lhs:pat, $rhs:pat) => $result:expr
            ) => {
                match &lhs_ty.kind {
                    $lhs => match &rhs_ty.kind {
                        $rhs => $result,
                        _ => ctx.ty_err(
                            format!(
                                concat!("binary operator `{}` can only be applied to ", $ty_kind),
                                op
                            ),
                            &rhs_ty,
                            rhs.span,
                        ),
                    },
                    _ => ctx.ty_err(
                        format!(
                            concat!("binary operator `{}` can only be applied to ", $ty_kind),
                            op
                        ),
                        &lhs_ty,
                        lhs.span,
                    ),
                }
            };
        }

        match op {
            BinOp::Add => {
                both_sides_match!("ints": (TyKind::Int { non_zero: lnz, non_negative: lnn }, TyKind::Int { non_zero: rnz, non_negative: rnn }) => {
                    let non_negative = *lnn && *rnn;
                    TyKind::Int {
                        non_zero: non_negative && (*lnz || *rnz),
                        non_negative: *lnn && *rnn
                    }
                })
            }
            BinOp::Sub => {
                both_sides_match!("ints": (TyKind::Int { .. }, TyKind::Int { .. }) => {
                    TyKind::Int {
                        non_zero: false,
                        non_negative: false
                    }
                })
            }
            BinOp::Mul => {
                both_sides_match!("ints": (TyKind::Int { non_zero: lnz, non_negative: lnn }, TyKind::Int { non_zero: rnz, non_negative: rnn }) => {
                    TyKind::Int {
                        non_zero: *lnz && *rnz,
                        non_negative: *lnn && *rnn
                    }
                })
            }
            BinOp::Div => {
                both_sides_match!("ints": (TyKind::Int { non_zero: _, non_negative: lnn }, TyKind::Int { non_zero: _, non_negative: rnn }) => {
                    TyKind::Int {
                        non_zero: false,
                        non_negative: *lnn && *rnn
                    }
                })
            }
            BinOp::Mod => {
                both_sides_match!("ints": (TyKind::Int { non_zero: _, non_negative: lnn }, TyKind::Int { .. }) => {
                    TyKind::Int {
                        non_zero: false,
                        non_negative: *lnn
                    }
                })
            }
            BinOp::Eq | BinOp::Neq => {
                if lhs_ty.unifies(&rhs_ty) && lhs_ty.supports_comparisons() {
                    TyKind::Bool
                } else {
                    ctx.add_diagnostic(
                        Diagnostic::error(
                            format!(
                                "binary operator `{op}` cannot be applied to `{}` and `{}`",
                                lhs_ty.name(),
                                rhs_ty.name()
                            ),
                            Label::new("unsupported comparison", expr_span),
                        )
                        .with_label(Label::new("left type", lhs_ty.span))
                        .with_label(Label::new("right type", rhs_ty.span)),
                    );
                    TyKind::Error
                }
            }
            BinOp::Gt | BinOp::Geq | BinOp::Lt | BinOp::Leq => {
                both_sides_match!("ints": (TyKind::Int { .. }, TyKind::Int { .. }) => {
                    TyKind::Bool
                })
            }
            BinOp::LogicalAnd | BinOp::LogicalOr => {
                both_sides_match!("bools": (TyKind::Bool, TyKind::Bool) => {
                    TyKind::Bool
                })
            }
            BinOp::BitAnd => {
                both_sides_match!("ints": (TyKind::Int { non_zero: _, non_negative: lnn }, TyKind::Int { non_zero: _, non_negative: rnn }) => {
                    TyKind::Int {
                        non_zero: false,
                        non_negative: *lnn || *rnn,
                    }
                })
            }
            BinOp::BitOr => {
                both_sides_match!("ints": (TyKind::Int { non_zero: lnz, non_negative: lnn }, TyKind::Int { non_zero: rnz, non_negative: rnn }) => {
                    TyKind::Int {
                        non_zero: *lnz || *rnz,
                        non_negative: *lnn && *rnn,
                    }
                })
            }
            BinOp::BitXor => {
                both_sides_match!("ints": (TyKind::Int { non_zero: _, non_negative: lnn }, TyKind::Int { non_zero: _, non_negative: rnn }) => {
                    TyKind::Int {
                        non_zero: false,
                        non_negative: *lnn && *rnn
                    }
                })
            }
            BinOp::ShiftLeft | BinOp::ShiftRight => {
                both_sides_match!("ints": (TyKind::Int { non_zero: lnz, non_negative: lnn }, TyKind::Int { non_zero: _, non_negative: rnn }) => {
                    if !*rnn {
                        ctx.add_diagnostic(
                            Diagnostic::error(
                                format!("cannot prove right-hand side of `{op}` to non-negative"),
                                Label::new("could be negative", rhs.span)
                            )
                            .with_label(Label::new("value origin", rhs_ty.span))
                        );
                        TyKind::Error
                    } else {
                        TyKind::Int {
                            non_zero: if *op == BinOp::ShiftLeft { *lnz } else { false },
                            non_negative: *lnn,
                        }
                    }
                })
            }
        }
    }

    /// Checks the give peek expression.
    fn check_peek(
        &self,
        ctx: &mut AnalysisCtx,
        parse_ty: &ParseType,
        offset: &Option<Box<Expr>>,
        extra_ctx: &ExtraExprCtx,
    ) -> TyKind {
        if let Some(offset) = offset {
            let offset_ty = self.check_expr(ctx, offset, extra_ctx);
            if !matches!(offset_ty.kind, TyKind::Int { .. }) {
                return ctx.ty_err(
                    "expected peek offset to be an integer",
                    &offset_ty,
                    offset.span,
                );
            }
        }

        self.check_parse_type(ctx, parse_ty).kind
    }

    /// Checks the given concatenation expression.
    fn check_concat(
        &self,
        ctx: &mut AnalysisCtx,
        args: &[ConcatArg],
        extra_ctx: &ExtraExprCtx,
    ) -> TyKind {
        let mut had_err = false;
        for arg in args {
            match arg {
                ConcatArg::Direct(expr) => {
                    let arg_ty = self.check_expr(ctx, expr, extra_ctx);
                    if !matches!(&arg_ty.kind, TyKind::Bytes) {
                        ctx.ty_err(
                            "expected `concat` argument to be of type `bytes`",
                            &arg_ty,
                            expr.span,
                        );
                        had_err = true;
                    }
                }
                ConcatArg::Expanding(expr) => {
                    let arg_ty = self.check_expr(ctx, expr, extra_ctx);
                    if !matches!(
                        &arg_ty.kind,
                        TyKind::Array {
                            item_ty
                        } if matches!(&item_ty.kind, TyKind::Bytes)
                    ) {
                        ctx.ty_err(
                            "expected `concat` `..`-argument to be of type `[bytes]`",
                            &arg_ty,
                            expr.span,
                        );
                        had_err = true;
                    }
                }
            }
        }

        if had_err {
            TyKind::Error
        } else {
            TyKind::Bytes
        }
    }

    /// Checks that the given expression evaluates to an integer.
    pub fn check_int(&self, ctx: &mut AnalysisCtx, expr: &Expr, message: impl ToString) -> bool {
        let expr_ty = self.check_expr(ctx, expr, &ExtraExprCtx::none());

        if matches!(&expr_ty.kind, TyKind::Int { .. }) {
            true
        } else {
            ctx.ty_err(message, &expr_ty, expr.span);
            false
        }
    }
}
