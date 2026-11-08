//! Implements evaluation of declarations.

use hexbait_common::{Len, RelativeOffset};

use crate::{
    Int,
    compile::{
        Span,
        ir::{Declaration, DeclarationKind},
    },
    eval::{
        BytesValue, Diagnostic, DiagnosticLevel, Provenance,
        parse::{
            ParseContext, StaticAnalysisImpossible as _,
            cursor::Cursor,
            diagnostics::{Result, SeekError},
            static_analysis_impossible,
            struct_context::{RecoveryStrategy, StructContext},
        },
    },
};

impl ParseContext<'_> {
    /// Determines if a seek to the given offset is possible.
    pub fn probe_seek(
        &mut self,
        new_offset: &Int,
        provenance: &Provenance,
        span: Span,
        cursor: &Cursor,
        context: &str,
    ) -> Result<RelativeOffset> {
        u64::try_from(new_offset)
            .map(RelativeOffset::from)
            .map_err(|_| SeekError::NegativeOffset)
            .and_then(|offset| cursor.probe_seek(offset))
            .map_err(|err| self.diagnostics.seek_err(err, provenance, span, context))
    }

    /// Attempts to set the cursor to the given offset.
    fn set_offset(
        &mut self,
        cursor: &mut Cursor,
        compute_offset: impl FnOnce(RelativeOffset) -> Result<RelativeOffset, SeekError>,
        provenance: &Provenance,
        span: Span,
        context: &str,
    ) -> Result<()> {
        let old_offset = cursor.offset();

        match compute_offset(old_offset).and_then(|new_offset| cursor.set_offset(new_offset)) {
            Ok(()) => Ok(()),
            Err(err) => Err(self.diagnostics.seek_err(err, provenance, span, context)),
        }
    }

    /// Evaluates the given declaration.
    pub fn eval_declaration(
        &mut self,
        declaration: &Declaration,
        cursor: &mut Cursor,
        struct_ctx: &mut StructContext,
    ) -> Result<()> {
        match &declaration.kind {
            DeclarationKind::Endianness(endianness) => cursor.set_endianness(*endianness),
            DeclarationKind::Align(expr) => {
                let value = self.eval_expr(expr, cursor, struct_ctx, Default::default())?;
                let align = value.kind.expect_int();
                let Ok(align) = u64::try_from(align) else {
                    return Err(self.diagnostics.new_err(
                        "alignment too large or negative".to_string(),
                        value.provenance.clone(),
                        expr.span,
                    ));
                };

                if !align.is_power_of_two() {
                    return Err(self.diagnostics.new_err(
                        "non-power-of-two alignment".to_string(),
                        value.provenance.clone(),
                        expr.span,
                    ));
                }

                self.set_offset(
                    cursor,
                    |offset| Ok(offset.align_up(Len::from(align))),
                    &value.provenance,
                    expr.span,
                    "during alignment",
                )?;
            }
            DeclarationKind::SeekBy(expr) => {
                let value = self.eval_expr(expr, cursor, struct_ctx, Default::default())?;
                let offset = value.kind.expect_int();

                self.set_offset(
                    cursor,
                    |old_offset| {
                        u64::try_from(offset + Int::from(old_offset.as_u64()))
                            .map(RelativeOffset::from)
                            .map_err(|_| SeekError::NegativeOffset)
                    },
                    &value.provenance,
                    expr.span,
                    "during seek",
                )?;
            }
            DeclarationKind::SeekTo(expr) => {
                let value = self.eval_expr(expr, cursor, struct_ctx, Default::default())?;
                let offset = value.kind.expect_int();

                self.set_offset(
                    cursor,
                    |_| {
                        u64::try_from(offset)
                            .map(RelativeOffset::from)
                            .map_err(|_| SeekError::NegativeOffset)
                    },
                    &value.provenance,
                    expr.span,
                    "during seek",
                )?;
            }
            DeclarationKind::Assert { condition, message } => {
                let condition_value =
                    self.eval_expr(condition, cursor, struct_ctx, Default::default())?;
                if !condition_value.kind.expect_bool() {
                    let message = if let Some(message) = message {
                        let message_val =
                            self.eval_expr(message, cursor, struct_ctx, Default::default())?;

                        format!(
                            "assertion failed: {}",
                            match message_val.kind.expect_bytes() {
                                BytesValue::Lit(lit) =>
                                    std::str::from_utf8(lit).static_analysis_expect(),
                                _ => static_analysis_impossible(),
                            }
                        )
                    } else {
                        String::from("assertion failed")
                    };

                    return Err(self.diagnostics.new_err(
                        message,
                        condition_value.provenance.clone(),
                        condition.span,
                    ));
                }
            }
            DeclarationKind::WarnIf { condition, message } => {
                let condition_value =
                    self.eval_expr(condition, cursor, struct_ctx, Default::default())?;
                if condition_value.kind.expect_bool() {
                    let message = if let Some(message) = message {
                        let message_val =
                            self.eval_expr(message, cursor, struct_ctx, Default::default())?;
                        format!(
                            "warning triggered: {}",
                            match message_val.kind.expect_bytes() {
                                BytesValue::Lit(lit) =>
                                    std::str::from_utf8(lit).static_analysis_expect(),
                                _ => static_analysis_impossible(),
                            }
                        )
                    } else {
                        String::from("warning triggered")
                    };

                    let id = self.diagnostics.new_diagnostic(Diagnostic {
                        message,
                        level: DiagnosticLevel::Warn,
                        provenance: condition_value.provenance.clone(),
                        span: condition.span,
                    });

                    struct_ctx.push_diagnostic(id);
                }
            }
            DeclarationKind::Recover { at } => {
                let offset = self.eval_expr(at, cursor, struct_ctx, Default::default())?;
                let offset = self.probe_seek(
                    offset.kind.expect_int(),
                    &offset.provenance,
                    at.span,
                    cursor,
                    "recovery offset",
                )?;

                struct_ctx.set_recovery_strategy(RecoveryStrategy::SkipTo { offset });
            }
        }

        Ok(())
    }
}
