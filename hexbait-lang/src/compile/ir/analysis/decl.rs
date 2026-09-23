//! Implements analysis of declarations.

use hexbait_common::Endianness;

use crate::compile::{
    Diagnostic, Span,
    diagnostics::Label,
    ir::{
        Block, Declaration, DeclarationKind, Expr, ExprKind, IfChain, Lit, ScopeKind,
        analysis::{
            AnalysisCtx, EndiannessState, Env,
            expr::ExtraExprCtx,
            ty::{StructTy, Ty, TyKind},
        },
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
            DeclarationKind::If(if_chain) => self.check_if_decl(ctx, if_chain, decl.span),
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
        let expr_ty = self.check_expr(ctx, expr, &ExtraExprCtx::none());

        if !matches!(&expr_ty.kind, TyKind::Int { .. }) {
            ctx.ty_err(
                "expected integer as argument to `seek by`",
                &expr_ty,
                expr.span,
            );
        }
    }

    /// Checks the given `seek to` declaration.
    fn check_seek_to_decl(&mut self, ctx: &mut AnalysisCtx, expr: &Expr) {
        self.check_positive_int(
            ctx,
            expr,
            "expected argument to `seek to` to be a positive integer",
        );
    }

    /// Checks the given scope declaration.
    fn check_scope_decl(&mut self, ctx: &mut AnalysisCtx, kind: &ScopeKind, block: &Block) {
        match kind {
            ScopeKind::At { start, end } => {
                self.check_positive_int(
                    ctx,
                    start,
                    "expected scope start to be a positive integer",
                );

                if let Some(end) = end {
                    self.check_positive_int(
                        ctx,
                        end,
                        "expected scope end to be a positive integer",
                    );
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

    /// Checks the given `if` declaration.
    fn check_if_decl(&mut self, ctx: &mut AnalysisCtx, if_chain: &IfChain, span: Span) {
        let mut branches = Vec::new();
        let mut undefined_endianness_branch = None;
        let mut defined_endianness_branch = None;

        let mut check_block = |ctx: &mut AnalysisCtx, block: &Block| {
            let mut env_clone = self.create_branch_clone();

            for content in &block.content {
                env_clone.check_struct_content(ctx, content);
            }

            branches.push(Ty {
                kind: TyKind::Struct {
                    struct_ty: env_clone.values,
                },
                span: block.span,
            });

            match env_clone.endianness_state {
                EndiannessState::Undefined => {
                    undefined_endianness_branch.get_or_insert(block.span);
                }
                EndiannessState::Set => {
                    defined_endianness_branch.get_or_insert(block.span);
                }
                EndiannessState::PartiallySet {
                    set_branch_span,
                    unset_branch_span,
                } => {
                    defined_endianness_branch.get_or_insert(set_branch_span);
                    undefined_endianness_branch.get_or_insert(unset_branch_span);
                }
            }
        };

        for branch in &if_chain.if_blocks {
            let condition_ty = self.check_expr(ctx, &branch.condition, &ExtraExprCtx::none());

            if !matches!(&condition_ty.kind, TyKind::Bool) {
                ctx.ty_err(
                    "expected `if` condition to be of type boolean",
                    &condition_ty,
                    branch.condition.span,
                );
            }

            check_block(ctx, &branch.block);
        }

        if let Some(else_block) = &if_chain.else_block {
            check_block(ctx, else_block);
        } else {
            branches.push(Ty {
                kind: TyKind::Struct {
                    struct_ty: std::mem::replace(&mut self.values, StructTy::empty()),
                },
                span: if_chain.if_blocks.last().unwrap().condition.span,
            });
        }

        let TyKind::Struct { struct_ty } = Ty::join(&branches, span).kind else {
            unreachable!()
        };

        self.values = struct_ty;
        self.endianness_state = match (defined_endianness_branch, undefined_endianness_branch) {
            (None, None) => unreachable!(),
            (None, Some(_)) => EndiannessState::Undefined,
            (Some(_), None) => EndiannessState::Set,
            (Some(set_branch), Some(unset_branch)) => EndiannessState::PartiallySet {
                set_branch_span: set_branch,
                unset_branch_span: unset_branch,
            },
        };
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

        if let Some(message) = message
            && !matches!(&message.kind, ExprKind::Lit(Lit::Bytes(_)))
        {
            ctx.add_diagnostic(Diagnostic::error(
                "expected `{ty}` message to be a string literal",
                Label::new("not a string literal", message.span),
            ));
        }
    }

    /// Checks the given recover at declaration.
    fn check_recover_at_decl(&mut self, ctx: &mut AnalysisCtx, at: &Expr) {
        self.check_positive_int(
            ctx,
            at,
            "expected recovery position to be a positive integer",
        );
    }
}
