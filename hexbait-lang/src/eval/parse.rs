//! Implements the parsing evaluation logic.

use std::{fmt, sync::Arc};

use crate::{
    BytesValue, Int, Span,
    ir::{
        BinOp, ConcatArg, Declaration, ElsePart, Expr, ExprKind, File, IfChain, LetStatement, Lit,
        ParseType, ParseTypeKind, RepeatKind, ScopeKind, StructContent, StructField, UnOp,
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

use hexbait_common::{Endianness, Len, RelativeOffset};
use num_traits::Zero as _;

pub use diagnostics::{Diagnostic, DiagnosticId, DiagnosticLevel};

mod cursor;
mod diagnostics;
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

macro_rules! impossible {
    () => {
        unreachable!("impossible because of static analysis")
    };
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

/// An accumulator for repeating parse types.
struct RepetitionAccumulator {
    /// The provenance of the resulting array.
    provenance: Provenance,
    /// The already parsed values.
    values: Vec<Value>,
}

impl RepetitionAccumulator {
    /// Creates a new accumulator.
    fn new() -> RepetitionAccumulator {
        RepetitionAccumulator {
            provenance: Provenance::empty(),
            values: Vec::new(),
        }
    }

    /// Pushes a new value into the accumulator.
    fn push(
        &mut self,
        parse_ctx: &mut ParseContext,
        cursor: &mut Cursor,
        struct_ctx: &StructContext,
        parse_type: &ParseType,
    ) -> Result<()> {
        match parse_ctx.eval_parse_type(parse_type, cursor, struct_ctx) {
            Ok(parsed_value) => {
                self.provenance += &parsed_value.provenance;
                self.values.push(parsed_value);

                Ok(())
            }
            Err(mut err) => {
                if let Some(partial_result) = err.take_partial_result() {
                    self.provenance += &partial_result.provenance;
                    self.values.push(partial_result);
                }
                let err_id = err.id();

                Err(err.with_partial_result(Value {
                    kind: ValueKind::Array {
                        items: std::mem::take(&mut self.values),
                        error: Some(err_id),
                    },
                    provenance: std::mem::take(&mut self.provenance),
                }))
            }
        }
    }

    /// Returns access to the already accumulated values.
    fn values(&self) -> &[Value] {
        &self.values
    }

    /// Turns the accumulator into a finished value.
    fn into_value(self) -> Value {
        Value {
            kind: ValueKind::Array {
                items: self.values,
                error: None,
            },
            provenance: self.provenance,
        }
    }
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

    /// Evaluates the given expression.
    fn eval_expr(
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
            ExprKind::FieldAccess { expr, field } => {
                let expr = self.eval_expr(expr, cursor, struct_ctx, additional_ctx)?;

                Ok(expr
                    .kind
                    .expect_struct()
                    .iter()
                    .find_map(|(name, value)| (name == &field.inner).then(|| value.clone()))
                    .static_analysis_expect())
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
            ExprKind::Error => impossible!(),
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
                                _ => impossible!(),
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
                                _ => impossible!(),
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

    /// Evaluates the given parsing type.
    fn eval_parse_type(
        &mut self,
        parse_type: &ParseType,
        cursor: &mut Cursor,
        struct_ctx: &StructContext,
    ) -> Result<Value> {
        let value = match &parse_type.kind {
            ParseTypeKind::Named { name } => {
                todo!("trying to parse named `{name:?}` unimplemented")
            }
            ParseTypeKind::Bytes { repetition_kind } => match repetition_kind {
                RepeatKind::Len { count: count_expr } => {
                    let count_val =
                        self.eval_expr(count_expr, cursor, struct_ctx, Default::default())?;

                    if let Ok(count) = u64::try_from(count_val.kind.expect_int()) {
                        self.read_bytes_value(count, parse_type.span, cursor)?
                    } else {
                        return Err(self.diagnostics.new_err(
                            "count too large".into(),
                            count_val.provenance.clone(),
                            count_expr.span,
                        ));
                    }
                }
                RepeatKind::While { condition } => {
                    let mut last_byte = None;
                    let mut len = 0;
                    let mut peek_cursor = cursor.clone();
                    while self
                        .eval_expr(
                            condition,
                            &peek_cursor,
                            struct_ctx,
                            AdditionalExprContext {
                                last: last_byte.as_ref(),
                                len: Some(&Value {
                                    kind: ValueKind::Integer(Int::from(len)),
                                    provenance: Provenance::empty(),
                                }),
                            },
                        )?
                        .kind
                        .expect_bool()
                    {
                        let (bytes, provenance) = peek_cursor.read_bytes_and_advance(
                            Len::from(1),
                            parse_type.span,
                            &mut self.diagnostics,
                        )?;

                        last_byte = Some(Value {
                            kind: ValueKind::Integer(bytes[0].into()),
                            provenance,
                        });
                        len += 1;
                    }

                    self.read_bytes_value(len, parse_type.span, cursor)?
                }
                RepeatKind::Error => impossible!(),
            },
            ParseTypeKind::Integer { signed, .. }
            | ParseTypeKind::DynamicInteger { signed, .. } => {
                let bit_width = match &parse_type.kind {
                    ParseTypeKind::Integer { bit_width, .. } => *bit_width,
                    ParseTypeKind::DynamicInteger { bit_width, .. } => {
                        let val =
                            self.eval_expr(bit_width, cursor, struct_ctx, Default::default())?;

                        u32::try_from(val.kind.expect_int()).map_err(|_| {
                            self.diagnostics.new_err(
                                "bit width is too large".to_string(),
                                val.provenance,
                                bit_width.span,
                            )
                        })?
                    }
                    _ => unreachable!(),
                };
                let signed = *signed;

                assert!(
                    bit_width % 8 == 0,
                    "non byte aligned integers currently unimplemented"
                );
                let size_in_bytes = (bit_width / 8) as usize;

                let endianness = *cursor.endianness();
                let (parsed_bytes, provenance) = cursor.read_bytes_and_advance(
                    Len::from(u64::try_from(size_in_bytes).unwrap()),
                    parse_type.span,
                    &mut self.diagnostics,
                )?;

                let num = match (endianness, signed) {
                    (Endianness::Little, true) => Int::from_signed_bytes_le(&parsed_bytes),
                    (Endianness::Big, true) => Int::from_signed_bytes_be(&parsed_bytes),
                    (Endianness::Little, false) => {
                        Int::from_bytes_le(num_bigint::Sign::Plus, &parsed_bytes)
                    }
                    (Endianness::Big, false) => {
                        Int::from_bytes_be(num_bigint::Sign::Plus, &parsed_bytes)
                    }
                };

                Value {
                    kind: ValueKind::Integer(num),
                    provenance,
                }
            }
            ParseTypeKind::Repeating {
                parse_type,
                repetition_kind,
            } => match repetition_kind {
                crate::ir::RepeatKind::Len { count } => {
                    let count_val =
                        self.eval_expr(count, cursor, struct_ctx, Default::default())?;
                    let mut accumulator = RepetitionAccumulator::new();

                    if let Ok(count) = u64::try_from(count_val.kind.expect_int()) {
                        for _ in 0..count {
                            accumulator.push(self, cursor, struct_ctx, parse_type)?;
                        }
                    } else {
                        return Err(self.diagnostics.new_err(
                            "count too large".into(),
                            count_val.provenance.clone(),
                            count.span,
                        ));
                    }

                    accumulator.into_value()
                }
                crate::ir::RepeatKind::While { condition } => {
                    let mut accumulator = RepetitionAccumulator::new();

                    while self
                        .eval_expr(
                            condition,
                            cursor,
                            struct_ctx,
                            AdditionalExprContext {
                                last: accumulator.values().last(),
                                len: Some(&Value {
                                    kind: ValueKind::Integer(Int::from(accumulator.values().len())),
                                    provenance: Provenance::empty(),
                                }),
                            },
                        )?
                        .kind
                        .expect_bool()
                    {
                        accumulator.push(self, cursor, struct_ctx, parse_type)?;
                    }

                    accumulator.into_value()
                }
                crate::ir::RepeatKind::Error => impossible!(),
            },
            ParseTypeKind::Struct { content } => {
                let mut ctx = struct_ctx.child();

                match self.eval_struct_content(content, cursor, &mut ctx) {
                    Ok(()) => ctx.into_value(),
                    Err(err) => Err(err.with_partial_result(ctx.into_value()))?,
                }
            }
            ParseTypeKind::Switch {
                scrutinee,
                branches,
                default,
            } => {
                let scrutinee_val =
                    self.eval_expr(scrutinee, cursor, struct_ctx, Default::default())?;

                'result: {
                    for (lit, parse_type) in branches {
                        if scrutinee_val.kind == *lit {
                            break 'result self.eval_parse_type(parse_type, cursor, struct_ctx)?;
                        }
                    }

                    self.eval_parse_type(default, cursor, struct_ctx)?
                }
            }
            ParseTypeKind::Error => impossible!(),
        };

        Ok(value)
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
            StructContent::Error => impossible!(),
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

/// Additional context that can be used during expression evaluation.
#[derive(Debug, Default, Clone, Copy)]
struct AdditionalExprContext<'parent> {
    /// The last parsed value in the current repeat expression.
    last: Option<&'parent Value>,
    /// The length of the current repeat expression.
    len: Option<&'parent Value>,
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
