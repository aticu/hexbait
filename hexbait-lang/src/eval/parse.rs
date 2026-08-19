//! Implements the parsing evaluation logic.

use std::fmt;

use crate::{
    BytesValue, Int, Span,
    ir::{
        Declaration, ElsePart, File, IfChain, LetStatement, ScopeKind, StructContent, StructField,
    },
    parse::{
        cursor::Cursor,
        diagnostics::{ParseErr, Result},
        struct_context::StructContext,
    },
};

use super::{
    provenance::Provenance,
    value::{Value, ValueKind},
    view::View,
};

use hexbait_common::{Len, RelativeOffset};

pub use diagnostics::{Diagnostic, DiagnosticId, DiagnosticLevel};

mod cursor;
mod diagnostics;
mod expr;
mod parse_ty;
mod struct_context;

/// The result of parsing.
pub struct ParseResult {
    /// The parsed value.
    pub value: Value,
    /// The diagnostics that occurred during parsing.
    pub diagnostics: Vec<Diagnostic>,
}

/// Evaluates the given IR on the given input.
pub fn eval_ir(file: &File, view: View, start_offset: RelativeOffset) -> ParseResult {
    let mut struct_ctx = StructContext::new();
    // the start offset should always be valid
    let mut cursor = Cursor::new(view, start_offset).static_analysis_expect();

    let diagnostics = Diagnostics {
        diagnostics: Vec::new(),
    };

    let mut parse_ctx = ParseContext { diagnostics };

    parse_ctx
        .eval_struct_content(&file.content, &mut cursor, &mut struct_ctx)
        .ok();

    ParseResult {
        value: struct_ctx.into_value(),
        diagnostics: parse_ctx.diagnostics.diagnostics,
    }
}

/// The context used during parsing.
#[derive(Debug)]
struct ParseContext {
    /// Stores the diagnostics during parsing.
    diagnostics: Diagnostics,
}

/// Stores the diagnostics that occur during parsing.
#[derive(Debug)]
struct Diagnostics {
    /// The diagnostics that occurred during parsing.
    diagnostics: Vec<Diagnostic>,
}

impl Diagnostics {
    /// Creates a new diagnostic.
    fn new_diagnostic(&mut self, diagnostic: Diagnostic) -> DiagnosticId {
        DiagnosticId::new(diagnostic, &mut self.diagnostics)
    }

    /// Creates a new parsing error.
    fn new_err(&mut self, message: String, provenance: Provenance, span: Span) -> ParseErr {
        ParseErr::new(self.new_diagnostic(Diagnostic {
            message,
            level: DiagnosticLevel::Fail,
            provenance,
            span,
        }))
    }

    /// Creates a parsing error for the given seek error.
    fn seek_err(
        &mut self,
        err: SeekError,
        provenance: &Provenance,
        span: Span,
        context: &str,
    ) -> ParseErr {
        self.new_err(
            format!(
                "could not set cursor {context}: {}",
                match err {
                    SeekError::NegativeOffset => String::from("negative offset"),
                    SeekError::SeekPastEnd { end, seek_offset } => {
                        format!(
                            "scope end is {end}, but new cursor position would be {seek_offset}"
                        )
                    }
                    SeekError::Overflow => String::from("integer overflow"),
                }
            ),
            provenance.clone(),
            span,
        )
    }
}

/// The different recovery strategies.
#[derive(Debug)]
enum RecoveryStrategy {
    /// Divert to the recovery strategy of the parent `struct`.
    Fallback,
    /// Skips to the given offset.
    SkipTo {
        /// The offset to skip to.
        offset: RelativeOffset,
    },
}

/// An error that can occur when seeking the input.
#[derive(Debug)]
enum SeekError {
    /// A seek was attempted to a negative offset.
    NegativeOffset,
    /// A seek past the end of the current scope.
    SeekPastEnd {
        /// The end of the scope.
        end: RelativeOffset,
        /// The offset where the seek was attempted.
        seek_offset: RelativeOffset,
    },
    /// The value overflowed the offset type.
    Overflow,
}

impl ParseContext {
    /// Determines if a seek to the given offset is possible.
    fn probe_seek(
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
    fn eval_declaration(
        &mut self,
        declaration: &Declaration,
        cursor: &mut Cursor,
        struct_ctx: &mut StructContext,
    ) -> Result<()> {
        match declaration {
            Declaration::Endianness(endianness) => cursor.set_endianness(*endianness),
            Declaration::Align(expr) => {
                let value = self.eval_expr(expr, cursor, struct_ctx, Default::default())?;
                let align = value.kind.expect_int();
                let align = Len::from(u64::try_from(align).static_analysis_expect());

                self.set_offset(
                    cursor,
                    |offset| Ok(offset.align_up(align)),
                    &value.provenance,
                    expr.span,
                    "during alignment",
                )?;
            }
            Declaration::SeekBy(expr) => {
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
            Declaration::SeekTo(expr) => {
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
            Declaration::Scope { kind, content } => {
                let (view, span) = match kind {
                    ScopeKind::At { start, end } => {
                        let span = start.span;
                        let start_expr =
                            self.eval_expr(start, cursor, struct_ctx, Default::default())?;

                        let start = self.probe_seek(
                            start_expr.kind.expect_int(),
                            &start_expr.provenance,
                            span,
                            cursor,
                            "for start of new scope",
                        )?;

                        let end = if let Some(end) = end {
                            let end_expr =
                                self.eval_expr(end, cursor, struct_ctx, Default::default())?;

                            self.probe_seek(
                                end_expr.kind.expect_int(),
                                &end_expr.provenance,
                                span,
                                cursor,
                                "for end of new scope",
                            )?
                        } else {
                            cursor.view().end_offset()
                        };

                        (cursor.view().subview(start..end), span)
                    }
                    ScopeKind::In { bytes } => {
                        let bytes_expr =
                            self.eval_expr(bytes, cursor, struct_ctx, Default::default())?;

                        (
                            View::from_bytes(bytes_expr.kind.expect_bytes_take()),
                            bytes.span,
                        )
                    }
                };

                let mut subcursor = cursor
                    .child_with_view_and_offset(view, RelativeOffset::ZERO)
                    .map_err(|err| {
                        self.diagnostics
                            .seek_err(err, &Provenance::empty(), span, "for scope")
                    })?;

                for single_content in content {
                    self.eval_single_struct_content(single_content, &mut subcursor, struct_ctx)?;
                }
            }
            Declaration::If(if_chain) => {
                self.eval_if_chain(if_chain, cursor, struct_ctx)?;
            }
            Declaration::Assert { condition, message } => {
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
            Declaration::WarnIf { condition, message } => {
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

                    self.diagnostics.new_diagnostic(Diagnostic {
                        message,
                        level: DiagnosticLevel::Warn,
                        provenance: condition_value.provenance.clone(),
                        span: condition.span,
                    });
                }
            }
            Declaration::Recover { at } => {
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

    /// Evaluates the given `if` chain.
    fn eval_if_chain(
        &mut self,
        if_chain: &IfChain,
        cursor: &mut Cursor,
        struct_ctx: &mut StructContext,
    ) -> Result<()> {
        let condition =
            self.eval_expr(&if_chain.condition, cursor, struct_ctx, Default::default())?;

        if condition.kind.expect_bool() {
            for single_content in &if_chain.then_block {
                self.eval_single_struct_content(single_content, cursor, struct_ctx)?;
            }
        } else if let Some(else_part) = &if_chain.else_part {
            match else_part {
                ElsePart::IfChain(if_chain) => {
                    self.eval_if_chain(if_chain, cursor, struct_ctx)?;
                }
                ElsePart::ElseBlock(else_block) => {
                    for single_content in else_block {
                        self.eval_single_struct_content(single_content, cursor, struct_ctx)?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Reads a bytes value.
    fn read_bytes_value(&mut self, count: u64, span: Span, cursor: &mut Cursor) -> Result<Value> {
        let start = cursor.offset();
        let len = Len::from(count);
        let Some(end) = start.checked_add(len) else {
            return Err(self.diagnostics.seek_err(
                SeekError::Overflow,
                &cursor.provenance_from_range(start..cursor.view().end_offset()),
                span,
                "when reading",
            ));
        };
        let mut buf = [0; BytesValue::INLINE_LEN];
        let provenance = cursor.provenance_from_range(start..end);

        if count > BytesValue::INLINE_LEN as u64 {
            let prefix_suffix_len = Len::from(BytesValue::PREFIX_SUFFIX_LEN as u64);

            let (prefix, _) =
                cursor.peek_bytes(start, prefix_suffix_len, span, &mut self.diagnostics)?;
            buf[..BytesValue::PREFIX_SUFFIX_LEN].copy_from_slice(&prefix);
            let (suffix, _) = cursor.peek_bytes(
                end - prefix_suffix_len,
                prefix_suffix_len,
                span,
                &mut self.diagnostics,
            )?;
            buf[BytesValue::PREFIX_SUFFIX_LEN..].copy_from_slice(&suffix);

            cursor.advance_by(len).map_err(|err| {
                self.diagnostics
                    .seek_err(err, &provenance, span, "after reading")
            })?;
        } else {
            let (bytes, _) = cursor.read_bytes_and_advance(len, span, &mut self.diagnostics)?;
            buf[..bytes.len()].copy_from_slice(&bytes);
        };

        Ok(Value {
            kind: ValueKind::Bytes(BytesValue::FromView {
                view: cursor.view().clone(),
                start,
                len,
                buf,
            }),
            provenance,
        })
    }

    /// Evaluates the given `struct` field.
    fn eval_struct_field(
        &mut self,
        field: &StructField,
        cursor: &mut Cursor,
        struct_ctx: &mut StructContext,
    ) -> Result<()> {
        let value = self.eval_parse_type(&field.ty, cursor, struct_ctx)?;

        if let Some(expected) = &field.expected {
            let span = expected.span;
            let expected = self.eval_expr(expected, cursor, struct_ctx, Default::default())?;
            if expected != value {
                return Err(self
                    .diagnostics
                    .new_err(
                        format!(
                            "field expectation failed: {:?} != {:?}",
                            expected.kind, value.kind
                        ),
                        &value.provenance + &expected.provenance,
                        span,
                    )
                    .with_partial_result(value));
            }
        }

        struct_ctx.insert(field.name.inner.clone(), value);

        Ok(())
    }

    /// Evaluates the given `let` statement.
    fn eval_let_statement(
        &mut self,
        let_statement: &LetStatement,
        cursor: &mut Cursor,
        struct_ctx: &mut StructContext,
    ) -> Result<()> {
        let value = self.eval_expr(&let_statement.expr, cursor, struct_ctx, Default::default())?;

        struct_ctx.insert(let_statement.name.inner.clone(), value);

        Ok(())
    }

    /// Evaluates the given single `struct` content.
    fn eval_single_struct_content(
        &mut self,
        content: &StructContent,
        cursor: &mut Cursor,
        struct_ctx: &mut StructContext,
    ) -> Result<()> {
        match content {
            StructContent::Field(field) => {
                match self.eval_struct_field(field, cursor, struct_ctx) {
                    Ok(()) => Ok(()),
                    Err(mut err) => {
                        if let Some(partial_result) = err.take_partial_result() {
                            struct_ctx.insert(field.name.inner.clone(), partial_result);
                        }
                        Err(err)
                    }
                }
            }
            StructContent::Declaration(declaration) => {
                Ok(self.eval_declaration(declaration, cursor, struct_ctx)?)
            }
            StructContent::LetStatement(let_statement) => {
                Ok(self.eval_let_statement(let_statement, cursor, struct_ctx)?)
            }
            StructContent::Error => static_analysis_impossible(),
        }
    }

    /// Evaluates the content of a `struct`.
    fn eval_struct_content(
        &mut self,
        content: &[StructContent],
        cursor: &mut Cursor,
        struct_ctx: &mut StructContext,
    ) -> Result<()> {
        for content in content {
            if let Err(err) = self.eval_single_struct_content(content, cursor, struct_ctx) {
                return struct_ctx.recover(cursor, err);
            }
        }

        Ok(())
    }
}

/// Indicates that something is impossible because of static analysis.
#[track_caller]
fn static_analysis_impossible() -> ! {
    unreachable!("impossible because of static analysis")
}

/// An extension trait to unwrap with a message that a situation should be impossible because of
/// static analysis
trait StaticAnalysisImpossible {
    /// The type that is unwrapped to.
    type Target;

    /// Unwraps a value with a message telling that the value must exist because of static
    /// analysis.
    fn static_analysis_expect(self) -> Self::Target;
}

impl<T> StaticAnalysisImpossible for Option<T> {
    type Target = T;

    #[track_caller]
    fn static_analysis_expect(self) -> Self::Target {
        self.expect("impossible because of static analysis")
    }
}

impl<T, E: fmt::Debug> StaticAnalysisImpossible for Result<T, E> {
    type Target = T;

    #[track_caller]
    fn static_analysis_expect(self) -> Self::Target {
        self.expect("impossible because of static analysis")
    }
}
