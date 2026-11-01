//! Implements analysis of declarations.

use hexbait_common::Endianness;

use crate::compile::{
    Diagnostic,
    diagnostics::Label,
    ir::{
        Block, Declaration, DeclarationKind, Expr, ExprKind, Lit, ScopeKind,
        analysis::{AnalysisCtx, EndiannessState, Env, expr::ExtraExprCtx, ty::TyKind},
    },
};

impl Env<'_> {
    /// Checks the given declaration.
    pub fn check_declaration(&mut self, ctx: &mut AnalysisCtx, decl: &Declaration) {
        match &decl.kind {
            DeclarationKind::Endianness(endianness) => self.check_endianness_decl(endianness),
            DeclarationKind::Align(expr) => self.check_align_decl(ctx, expr),
            DeclarationKind::SeekBy(expr) => self.check_seek_by_decl(ctx, expr),
            DeclarationKind::SeekTo(expr) => self.check_seek_to_decl(ctx, expr),
            DeclarationKind::Scope { kind, block } => self.check_scope_decl(ctx, kind, block),
            DeclarationKind::Assert { condition, message } => {
                self.check_assert_warn_decl(ctx, condition, message, "assert")
            }
            DeclarationKind::WarnIf { condition, message } => {
                self.check_assert_warn_decl(ctx, condition, message, "warn")
            }
            DeclarationKind::Recover { at } => self.check_recover_at_decl(ctx, at),
        }
    }

    /// Checks the given endianness declaration.
    fn check_endianness_decl(&mut self, _: &Endianness) {
        self.endianness_state = EndiannessState::Set;
    }

    /// Checks the given alignment declaration.
    fn check_align_decl(&mut self, ctx: &mut AnalysisCtx, expr: &Expr) {
        let expr_ty = self.check_expr(ctx, expr, &ExtraExprCtx::none());

        match expr_ty.kind {
            TyKind::Error => (),
            TyKind::Int {
                non_zero,
                non_negative,
            } => {
                if !non_zero || !non_negative {
                    ctx.add_diagnostic(Diagnostic::error(
                        "expected positive integer as alignment",
                        Label::new("found possibly zero or negative integer", expr.span),
                    ));
                    return;
                }

                if let ExprKind::Lit(Lit::Int(int)) = &expr.kind {
                    let Ok(int) = u64::try_from(int) else {
                        ctx.add_diagnostic(Diagnostic::warning(
                            "this expression will error during execution",
                            Label::new("alignment too large", expr.span),
                        ));
                        return;
                    };

                    if !int.is_power_of_two() {
                        ctx.add_diagnostic(Diagnostic::warning(
                            "this expression will error during execution",
                            Label::new("alignment must be a power of two", expr.span),
                        ));
                    }
                }
            }
            _ => {
                ctx.ty_err("expected an integer as alignment", &expr_ty, expr.span);
            }
        }
    }

    /// Checks the given `seek by` declaration.
    fn check_seek_by_decl(&mut self, ctx: &mut AnalysisCtx, expr: &Expr) {
        self.check_int(ctx, expr, "expected argument to `seek by` to be an integer");
    }

    /// Checks the given `seek to` declaration.
    fn check_seek_to_decl(&mut self, ctx: &mut AnalysisCtx, expr: &Expr) {
        self.check_int(ctx, expr, "expected argument to `seek to` to be an integer");
    }

    /// Checks the given scope declaration.
    fn check_scope_decl(&mut self, ctx: &mut AnalysisCtx, kind: &ScopeKind, block: &Block) {
        match kind {
            ScopeKind::At { start, end } => {
                self.check_int(ctx, start, "expected scope start to be an integer");

                if let Some(end) = end {
                    self.check_int(ctx, end, "expected scope end to be an integer");
                }
            }
            ScopeKind::In { bytes } => {
                let bytes_ty = self.check_expr(ctx, bytes, &ExtraExprCtx::none());

                if !matches!(&bytes_ty.kind, TyKind::Bytes) {
                    ctx.ty_err(
                        "expected bytes type as argument to `scope in`",
                        &bytes_ty,
                        bytes.span,
                    );
                }
            }
        }

        for content in &block.content {
            self.check_struct_content(ctx, content);
        }
    }

    /// Checks the given assert or warning declaration.
    fn check_assert_warn_decl(
        &mut self,
        ctx: &mut AnalysisCtx,
        condition: &Expr,
        message: &Option<Expr>,
        ty: &str,
    ) {
        let condition_ty = self.check_expr(ctx, condition, &ExtraExprCtx::none());

        if !matches!(&condition_ty.kind, TyKind::Bool) {
            ctx.ty_err(
                format!("expected `{ty}` condition to be boolean"),
                &condition_ty,
                condition.span,
            );
        }

        if let Some(message) = message {
            match &message.kind {
                ExprKind::Lit(Lit::Bytes(val)) if std::str::from_utf8(val).is_ok() => (),
                _ => {
                    ctx.add_diagnostic(Diagnostic::error(
                        format!("expected `{ty}` message to be a valid UTF-8 string literal"),
                        Label::new("not a valid string literal", message.span),
                    ));
                }
            }
        }
    }

    /// Checks the given recover at declaration.
    fn check_recover_at_decl(&mut self, ctx: &mut AnalysisCtx, at: &Expr) {
        self.check_int(ctx, at, "expected recovery position to be an integer");
    }
}
