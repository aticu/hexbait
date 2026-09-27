//! Implements analysis of parse types.

use crate::compile::{
    Diagnostic, Span,
    diagnostics::Label,
    ir::{
        BinOp, Expr, ExprKind, Lit, ParseType, ParseTypeKind, RepeatKind, Repetition, Spanned,
        StructContent, Symbol,
        analysis::{
            AnalysisCtx, Env,
            expr::ExtraExprCtx,
            ty::{Ty, TyKind},
        },
    },
};

impl Env<'_> {
    /// Checks the given parse type.
    pub fn check_parse_type(&self, ctx: &mut AnalysisCtx, parse_ty: &ParseType) -> Ty {
        let kind = 'ty: {
            match &parse_ty.kind {
                ParseTypeKind::Named { name } => self.check_named_parse_ty(ctx, name),
                ParseTypeKind::Integer { bit_width, signed } => {
                    self.check_int_parse_ty(ctx, Some(*bit_width), *signed, parse_ty.span)
                }
                ParseTypeKind::DynamicInteger { bit_width, signed } => {
                    self.check_dynamic_int_parse_ty(ctx, bit_width, *signed, parse_ty.span)
                }
                ParseTypeKind::Bytes { repetition } => self.check_bytes_parse_ty(ctx, repetition),
                ParseTypeKind::Repeating {
                    parse_type,
                    repetition,
                } => self.check_repeating_parse_ty(ctx, parse_type, repetition),
                ParseTypeKind::Struct { block } => self.check_struct_parse_ty(ctx, &block.content),
                ParseTypeKind::Switch {
                    scrutinee,
                    branches,
                    default,
                } => {
                    let scrutinee_ty = self.check_expr(ctx, scrutinee, &ExtraExprCtx::none());
                    if !scrutinee_ty.supports_comparisons() {
                        ctx.add_diagnostic(Diagnostic::error(
                            "`switch` scrutinee type does not support comparisons",
                            Label::new("type does not support comparisons", scrutinee.span),
                        ));
                        break 'ty TyKind::Error;
                    }

                    let mut branch_tys = Vec::new();

                    for (lit, lit_span, parse_ty) in branches {
                        let lit_ty = self.check_lit(lit);

                        if !scrutinee_ty.kind.unifies(&lit_ty) {
                            break 'ty ctx.ty_err(
                                "expected branch literal to be of the same type as the scrutinee",
                                &scrutinee_ty,
                                *lit_span,
                            );
                        }

                        branch_tys.push(self.check_parse_type(ctx, parse_ty));
                    }

                    branch_tys.push(self.check_parse_type(ctx, default));

                    Ty::join(&branch_tys, parse_ty.span, false).kind
                }
                ParseTypeKind::Error => {
                    assert!(ctx.diagnostics.contains_errors());
                    TyKind::Error
                }
            }
        };

        Ty {
            kind,
            span: parse_ty.span,
        }
    }

    /// Checks the given named parse type.
    fn check_named_parse_ty(&self, ctx: &mut AnalysisCtx, name: &Spanned<Symbol>) -> TyKind {
        ctx.add_diagnostic(Diagnostic::error(
            "named parse types currently unsupported",
            Label::new("unsupported feature use here", name.span),
        ));

        TyKind::Error
    }

    /// Checks an integer kind with the given size in bits and signedness.
    fn check_int_parse_ty(
        &self,
        ctx: &mut AnalysisCtx,
        size: Option<u32>,
        signed: bool,
        span: Span,
    ) -> TyKind {
        if let Some(size) = size
            && !size.is_multiple_of(8)
        {
            ctx.add_diagnostic(Diagnostic::error(
                "unsupported feature non-bit-multiple integers",
                Label::new("unsupported feature", span),
            ));
            return TyKind::Error;
        }

        if !self.endianness_state.is_set() && size != Some(0) && size != Some(8) {
            ctx.add_diagnostic(
                Diagnostic::error(
                    "attempt to parse a multi-byte integer before endianness is set",
                    Label::new("endianness unknown here", span),
                )
                .with_labels(self.endianness_state.unset_label())
                .with_labels(self.endianness_state.set_label()),
            );
            TyKind::Error
        } else {
            TyKind::Int {
                non_zero: false,
                non_negative: !signed,
            }
        }
    }

    /// Checks a dynamic integer kind with the given signedness.
    fn check_dynamic_int_parse_ty(
        &self,
        ctx: &mut AnalysisCtx,
        bit_width: &Expr,
        signed: bool,
        span: Span,
    ) -> TyKind {
        let width_ty = self.check_expr(ctx, bit_width, &ExtraExprCtx::none());
        if !matches!(
            &width_ty.kind,
            TyKind::Int {
                non_zero: _,
                non_negative: true,
            }
        ) {
            return ctx.ty_err(
                "expected non-negative int in dynamic int width",
                &width_ty,
                bit_width.span,
            );
        };

        let is_const_8 = |expr: &Expr| matches!(&expr.kind, ExprKind::Lit(Lit::Int(val)) if u8::try_from(val) == Ok(8));

        let is_multiple_of_8 = match &bit_width.kind {
            ExprKind::BinOp {
                op: BinOp::Mul,
                lhs,
                rhs,
            } if is_const_8(lhs) || is_const_8(rhs) => true,
            ExprKind::Lit(Lit::Int(int_lit))
                if let Ok(int_lit) = u64::try_from(int_lit)
                    && int_lit.is_multiple_of(8) =>
            {
                true
            }
            _ => false,
        };

        if is_multiple_of_8 {
            self.check_int_parse_ty(ctx, None, signed, span)
        } else {
            ctx.add_diagnostic(
                Diagnostic::error(
                    "dynamic integer must be a multiple of 8",
                    Label::new("not guaranteed to be a multiple of 8", bit_width.span),
                )
                .with_help("use `val * 8` directly here"),
            );
            TyKind::Error
        }
    }

    /// Checks the given repetition.
    fn check_repetition(&self, ctx: &mut AnalysisCtx, repetition: &Repetition, elem_ty: Ty) {
        match &repetition.kind {
            RepeatKind::Len { count } => {
                let count_ty = self.check_expr(ctx, count, &ExtraExprCtx::none());
                if !matches!(
                    &count_ty.kind,
                    TyKind::Int {
                        non_zero: _,
                        non_negative: _
                    }
                ) {
                    ctx.ty_err(
                        "expected `len` expression to be an integer",
                        &count_ty,
                        count.span,
                    );
                }
            }
            RepeatKind::While { condition } => {
                let condition_ty =
                    self.check_expr(ctx, condition, &ExtraExprCtx::in_while_condition(elem_ty));
                // TODO: check that `$len > 0` before `$last` can be accessed
                // TODO: check that this can terminate, by checking that the supplied parse type can make progress
                if !matches!(&condition_ty.kind, TyKind::Bool) {
                    ctx.ty_err(
                        "expected `while` expression to be a bool",
                        &condition_ty,
                        condition.span,
                    );
                }
            }
            RepeatKind::Error => {
                assert!(ctx.diagnostics.contains_error_in(repetition.span));
            }
        }
    }

    /// Checks a byte parse type with the given repetition kind.
    fn check_bytes_parse_ty(&self, ctx: &mut AnalysisCtx, repetition: &Repetition) -> TyKind {
        self.check_repetition(
            ctx,
            repetition,
            Ty {
                kind: TyKind::Int {
                    non_zero: false,
                    non_negative: true,
                },
                span: repetition.span,
            },
        );

        TyKind::Bytes
    }

    /// Checks a repeating parse type.
    fn check_repeating_parse_ty(
        &self,
        ctx: &mut AnalysisCtx,
        parse_type: &ParseType,
        repetition: &Repetition,
    ) -> TyKind {
        let elem_ty = self.check_parse_type(ctx, parse_type);
        self.check_repetition(ctx, repetition, elem_ty.clone());

        TyKind::Array {
            item_ty: Box::new(elem_ty),
        }
    }

    /// Checks an anonymous struct parse type.
    fn check_struct_parse_ty(&self, ctx: &mut AnalysisCtx, content: &[StructContent]) -> TyKind {
        let mut env = self.child();

        for content in content {
            env.check_struct_content(ctx, content);
        }

        TyKind::Struct {
            struct_ty: env.values,
        }
    }
}
