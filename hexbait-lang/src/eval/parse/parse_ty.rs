//! Implements evaluation of a parse type.

use hexbait_common::{Endianness, Len};

use crate::{
    BytesValue, Int, Provenance, Span, Value, ValueKind,
    ir::{ParseType, ParseTypeKind, RepeatKind},
    parse::{
        ParseContext,
        cursor::Cursor,
        diagnostics::{Result, SeekError},
        expr::AdditionalExprContext,
        static_analysis_impossible,
        struct_context::StructContext,
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
                RepeatKind::Error => static_analysis_impossible(),
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
                crate::ir::RepeatKind::Error => static_analysis_impossible(),
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
