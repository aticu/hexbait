//! Implements evaluation of a parse type.

use hexbait_common::{Endianness, Len};

use crate::{
    Int,
    compile::{
        Span,
        ir::{Expr, ParseType, ParseTypeKind, RepeatKind},
    },
    eval::{
        BytesValue, Provenance, Value, ValueKind,
        parse::{
            ParseContext, StaticAnalysisImpossible,
            cursor::Cursor,
            diagnostics::{Diagnostics, Result, SeekError},
            expr::AdditionalExprContext,
            static_analysis_impossible,
            struct_context::StructContext,
        },
    },
};

impl ParseContext {
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

    /// Evaluates the length of a `len` repetition.
    fn eval_count(
        &mut self,
        expr: &Expr,
        cursor: &mut Cursor,
        struct_ctx: &StructContext,
    ) -> Result<u64> {
        let count_val = self.eval_expr(expr, cursor, struct_ctx, Default::default())?;

        u64::try_from(count_val.kind.expect_int()).map_err(|_| {
            self.diagnostics.new_err(
                "count too large".into(),
                count_val.provenance.clone(),
                expr.span,
            )
        })
    }

    /// Evaluates the condition of a `while` repetition.
    fn eval_while_condition(
        &mut self,
        condition: &Expr,
        cursor: &Cursor,
        struct_ctx: &StructContext,
        last_val: Option<&Value>,
        len: usize,
    ) -> Result<bool> {
        self.eval_expr(
            condition,
            cursor,
            struct_ctx,
            AdditionalExprContext {
                last: last_val,
                len: Some(&Value {
                    kind: ValueKind::Integer(Int::from(len)),
                    provenance: Provenance::empty(),
                }),
            },
        )
        .map(|val| val.kind.expect_bool())
    }

    /// Evaluates the given parsing type.
    pub fn eval_parse_type(
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
                    let count = self.eval_count(count_expr, cursor, struct_ctx)?;
                    self.read_bytes_value(count, parse_type.span, cursor)?
                }
                RepeatKind::While { condition } => {
                    let mut last_byte = None;
                    let mut len = 0;
                    let mut peek_cursor = cursor.clone();
                    while self.eval_while_condition(
                        condition,
                        &peek_cursor,
                        struct_ctx,
                        last_byte.as_ref(),
                        len,
                    )? {
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

                    self.read_bytes_value(len as u64, parse_type.span, cursor)?
                }
                RepeatKind::Error => static_analysis_impossible(),
            },
            ParseTypeKind::Integer { signed, bit_width } => parse_int(
                *bit_width,
                *signed,
                parse_type.span,
                cursor,
                &mut self.diagnostics,
            )?,
            ParseTypeKind::DynamicInteger { signed, bit_width } => {
                let bit_width_val =
                    self.eval_expr(bit_width, cursor, struct_ctx, Default::default())?;
                let bit_width = u32::try_from(bit_width_val.kind.expect_int()).map_err(|_| {
                    self.diagnostics.new_err(
                        "bit width is too large".to_string(),
                        bit_width_val.provenance,
                        bit_width.span,
                    )
                })?;

                parse_int(
                    bit_width,
                    *signed,
                    parse_type.span,
                    cursor,
                    &mut self.diagnostics,
                )?
            }
            ParseTypeKind::Repeating {
                parse_type,
                repetition_kind,
            } => match repetition_kind {
                RepeatKind::Len { count } => {
                    let count = self.eval_count(count, cursor, struct_ctx)?;

                    let mut accumulator = RepetitionAccumulator::new();
                    for _ in 0..count {
                        accumulator.push(self, cursor, struct_ctx, parse_type)?;
                    }
                    accumulator.into_value()
                }
                RepeatKind::While { condition } => {
                    let mut accumulator = RepetitionAccumulator::new();

                    while self.eval_while_condition(
                        condition,
                        cursor,
                        struct_ctx,
                        accumulator.values().last(),
                        accumulator.values().len(),
                    )? {
                        accumulator.push(self, cursor, struct_ctx, parse_type)?;
                    }

                    accumulator.into_value()
                }
                RepeatKind::Error => static_analysis_impossible(),
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
            ParseTypeKind::Error => static_analysis_impossible(),
        };

        Ok(value)
    }
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

/// Parses an integer.
fn parse_int(
    bit_width: u32,
    signed: bool,
    span: Span,
    cursor: &mut Cursor,
    diagnostics: &mut Diagnostics,
) -> Result<Value> {
    assert!(
        bit_width.is_multiple_of(8),
        "non byte aligned integers currently unimplemented"
    );
    let size_in_bytes = (bit_width / 8) as usize;

    let endianness = *cursor.endianness();
    let (parsed_bytes, provenance) = cursor.read_bytes_and_advance(
        Len::from(u64::try_from(size_in_bytes).static_analysis_expect()),
        span,
        diagnostics,
    )?;

    let num = match (endianness, signed) {
        (Endianness::Little, true) => Int::from_signed_bytes_le(&parsed_bytes),
        (Endianness::Big, true) => Int::from_signed_bytes_be(&parsed_bytes),
        (Endianness::Little, false) => Int::from_bytes_le(num_bigint::Sign::Plus, &parsed_bytes),
        (Endianness::Big, false) => Int::from_bytes_be(num_bigint::Sign::Plus, &parsed_bytes),
    };

    Ok(Value {
        kind: ValueKind::Integer(num),
        provenance,
    })
}
