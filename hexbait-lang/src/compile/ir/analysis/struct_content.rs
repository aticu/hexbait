//! Implements analysis of struct contents.

use crate::compile::{
    Diagnostic, Span,
    diagnostics::Label,
    ir::{
        Block, IfChain, LetStatement, Spanned, StructContent, StructContentKind, StructField,
        Symbol,
        analysis::{
            AnalysisCtx, EndiannessState, Env,
            expr::ExtraExprCtx,
            ty::{Availability, FieldTy, Ty, TyKind},
        },
    },
};

impl Env<'_> {
    /// Checks the given struct content.
    pub fn check_struct_content(&mut self, ctx: &mut AnalysisCtx, content: &StructContent) {
        match &content.kind {
            StructContentKind::Field(struct_field) => self.check_struct_field(ctx, struct_field),
            StructContentKind::Declaration(declaration) => {
                self.check_declaration(ctx, declaration);
            }
            StructContentKind::LetStatement(let_statement) => {
                self.check_let_statement(ctx, let_statement);
            }
            StructContentKind::IfChain(if_chain) => {
                self.check_if_chain(ctx, if_chain, content.span);
            }
            StructContentKind::Error => {
                assert!(ctx.diagnostics.contains_error_in(content.span));
            }
        }
    }

    /// Adds the given field to the environment.
    fn add_field(&mut self, ctx: &mut AnalysisCtx, name: &Spanned<Symbol>, ty: Ty) {
        if let Some(field) = self.values.field(&name.inner) {
            ctx.add_diagnostic(
                Diagnostic::error(
                    "field should not be duplicated",
                    Label::new("duplicated field name", name.span),
                )
                .with_label(Label::new("original field defined here", field.ty.span)),
            );
            return;
        }

        self.values.add_field(FieldTy {
            name: name.inner.clone(),
            ty,
            availability: Availability::Guaranteed,
        });
    }

    /// Checks the given struct field.
    fn check_struct_field(&mut self, ctx: &mut AnalysisCtx, struct_field: &StructField) {
        let StructField { name, ty, expected } = struct_field;

        let ty = self.check_parse_type(ctx, ty);
        if let Some(expected) = expected {
            let expected_ty = self.check_expr(ctx, expected, &ExtraExprCtx::none());

            if let Some(incomparable_reason) = ty.supports_comparisons_with(&expected_ty) {
                ctx.add_diagnostic(Diagnostic::error(
                    "expected value cannot be checked because the type does not support comparisons with the parsed type",
                    Label::new("cannot be compared to the parsed type", expected.span)
                ).with_help(incomparable_reason));
            }
        }

        self.add_field(ctx, name, ty);
    }

    /// Checks the given let statement.
    fn check_let_statement(&mut self, ctx: &mut AnalysisCtx, let_statement: &LetStatement) {
        let LetStatement { name, expr } = let_statement;

        let ty = self.check_expr(ctx, expr, &ExtraExprCtx::none());
        self.add_field(ctx, name, ty);
    }

    /// Checks the given `if` chain.
    fn check_if_chain(&mut self, ctx: &mut AnalysisCtx, if_chain: &IfChain, span: Span) {
        let mut branches = Vec::new();
        let mut undefined_endianness_branch = None;
        let mut defined_endianness_branch = None;
        let mut unset_is_missing_else = false;

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
                    unset_is_missing_else: missing_else,
                } => {
                    defined_endianness_branch.get_or_insert(set_branch_span);
                    undefined_endianness_branch.get_or_insert(unset_branch_span);
                    unset_is_missing_else |= missing_else;
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
            let span = if_chain.if_blocks.last().unwrap().condition.span;
            branches.push(Ty {
                kind: TyKind::Struct {
                    struct_ty: self.values.clone(),
                },
                span,
            });
            match self.endianness_state {
                EndiannessState::Undefined => {
                    if undefined_endianness_branch.is_none() {
                        unset_is_missing_else = true;
                    }
                    undefined_endianness_branch.get_or_insert(span);
                }
                EndiannessState::Set => {
                    defined_endianness_branch.get_or_insert(span);
                }
                EndiannessState::PartiallySet {
                    set_branch_span,
                    unset_branch_span,
                    unset_is_missing_else: missing_else,
                } => {
                    defined_endianness_branch.get_or_insert(set_branch_span);
                    undefined_endianness_branch.get_or_insert(unset_branch_span);
                    unset_is_missing_else |= missing_else;
                }
            }
        }

        let struct_ty = match Ty::join(&branches, span, if_chain.else_block.is_none()).kind {
            TyKind::Error => return,
            TyKind::Struct { struct_ty } => struct_ty,
            TyKind::Indeterminate { .. }
            | TyKind::Bool
            | TyKind::Int { .. }
            | TyKind::EnumLit { .. }
            | TyKind::EnumVal { .. }
            | TyKind::FlagsVal { .. }
            | TyKind::Bytes
            | TyKind::Array { .. } => unreachable!(),
        };

        self.values = struct_ty;
        self.endianness_state = match (defined_endianness_branch, undefined_endianness_branch) {
            (None, None) => unreachable!(),
            (None, Some(_)) => EndiannessState::Undefined,
            (Some(_), None) => EndiannessState::Set,
            (Some(set_branch), Some(unset_branch)) => EndiannessState::PartiallySet {
                set_branch_span: set_branch,
                unset_branch_span: unset_branch,
                unset_is_missing_else,
            },
        };
    }
}
