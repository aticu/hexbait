//! Implements analysis of struct contents.

use crate::compile::{
    Diagnostic,
    diagnostics::Label,
    ir::{
        LetStatement, Spanned, StructContent, StructContentKind, StructField, Symbol,
        analysis::{
            AnalysisCtx, Env,
            expr::ExtraExprCtx,
            ty::{Availability, FieldTy, Ty},
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
}
