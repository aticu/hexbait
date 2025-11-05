//! Implements the parsing evaluation logic.

use std::{fmt, io};

use crate::{
    Int, Span,
    ir::{
        BinOp, Declaration, Endianness, Expr, ExprKind, File, LetStatement, Lit, ParseType,
        ParseTypeKind, RepeatKind, StructContent, StructField, Symbol, UnOp,
    },
};

use super::{
    provenance::Provenance,
    value::{Value, ValueKind},
    view::View,
};

/// An offset in bytes to parse from.
#[derive(Debug, Clone, Copy)]
struct ByteOffset(u64);

impl TryFrom<&Value> for ByteOffset {
    type Error = ParseErrKind;

    fn try_from(value: &Value) -> Result<Self, Self::Error> {
        u64::try_from(value.kind.expect_int())
            .map(|offset| ByteOffset(offset))
            .map_err(|_| ParseErrKind::OffsetTooLarge)
    }
}

/// An error that occurred during parsing.
#[derive(Debug)]
pub enum ParseErrKind {
    /// The input was shorter than expected.
    InputTooShort,
    /// A value that is meant as an offset was too large.
    OffsetTooLarge,
    /// An assertion failed.
    AssertionFailure,
    /// An assertion failed.
    ExpectationFailure,
    /// An I/O error occurred during parsing.
    Io(io::Error),
}

/// An error that occurred during parsing.
#[derive(Debug)]
pub struct ParseErr {
    /// The kind of error that occurred.
    kind: ParseErrKind,
    /// The provenance where the error occurred.
    provenance: Provenance,
    /// The span of the node where parsing failed.
    span: Span,
}

impl From<io::Error> for ParseErrKind {
    fn from(err: io::Error) -> Self {
        ParseErrKind::Io(err)
    }
}

/// Evaluates the given IR on the given input.
pub fn eval_ir(file: &File, view: View<'_>, start_offset: u64) -> Value {
    let mut ctx = StructContext::new();
    let mut scope = Scope::new(&view);
    scope.offset = ByteOffset(start_offset);

    for content in &file.content {
        scope.eval_single_struct_content(content, &mut ctx);
    }

    ctx.into_value()
}

macro_rules! impossible {
    () => {
        unreachable!("impossible because of static analysis")
    };
}

/// The different recovery strategies.
#[derive(Debug)]
enum RecoveryStrategy {
    /// Divert to the recovery strategy of the parent `struct`.
    Fallback,
    /// Skips to the given offset.
    SkipTo {
        /// The offset to skip to.
        offset: ByteOffset,
    },
}

/// The parsing context for a `struct`.
#[derive(Debug)]
struct StructContext<'parent> {
    /// The already parsed fields.
    parsed_fields: Vec<(Symbol, Value)>,
    /// The parent `struct`.
    parent: Option<&'parent StructContext<'parent>>,
    /// The recovery strategy to use if parsing fails.
    recovery_strategy: RecoveryStrategy,
}

impl<'parent> StructContext<'parent> {
    /// Creates a new `struct` parsing context.
    fn new() -> StructContext<'static> {
        StructContext {
            parsed_fields: Vec::new(),
            parent: None,
            recovery_strategy: RecoveryStrategy::Fallback,
        }
    }

    /// Creates the context for a child `struct`.
    fn child<'this>(&'this self) -> StructContext<'this> {
        StructContext {
            parsed_fields: Vec::new(),
            parent: Some(self),
            recovery_strategy: RecoveryStrategy::Fallback,
        }
    }

    /// Returns the `struct` context as a partially parsed `struct` value.
    fn as_value(&self) -> Value {
        let mut provenance = Provenance::empty();
        for (_, value) in &self.parsed_fields {
            provenance += &value.provenance;
        }

        Value {
            kind: ValueKind::Struct(self.parsed_fields.clone()),
            provenance,
        }
    }

    /// Turns the `struct` context into a fully parsed `struct`.
    fn into_value(self) -> Value {
        let mut provenance = Provenance::empty();
        for (_, value) in &self.parsed_fields {
            provenance += &value.provenance;
        }

        Value {
            kind: ValueKind::Struct(self.parsed_fields),
            provenance,
        }
    }
}

/// The parsing context for a `scope`.
#[derive(Debug)]
struct Scope<'src> {
    /// The endianness used for parsing.
    endianness: Endianness,
    /// The current offset used for parsing.
    offset: ByteOffset,
    /// The view that this scope parses from.
    view: &'src View<'src>,
}

impl<'src> Scope<'src> {
    /// Creates a new `scope` for the given `struct` context in the given view.
    fn new(view: &'src View<'src>) -> Scope<'src> {
        Scope {
            // static analysis makes sure that this is set to the correct value before parsing
            endianness: Endianness::Little,
            offset: ByteOffset(0),
            view,
        }
    }

    /// Creates a new child scope with the given view and offset.
    fn child_with_view_and_offset(
        &self,
        view: &'src View<'src>,
        offset: ByteOffset,
    ) -> Scope<'src> {
        Scope {
            endianness: self.endianness,
            view,
            offset,
        }
    }

    /// Reports the given error at the given location.
    ///
    /// This function is generic to allow for the pattern `return self.error(...)` in any function.
    fn error<T>(
        &self,
        message: impl Into<String>,
        kind: ParseErrKind,
        location: &Provenance,
        span: Span,
    ) -> Result<T, ParseErr> {
        eprintln!(
            "TODO: add proper error handling: {} at {location:?} here {span:?}",
            message.into()
        );

        Err(ParseErr {
            kind,
            provenance: location.clone(),
            span,
        })
    }

    /// Reads the specified number of bytes.
    fn read_bytes(&mut self, count: u64, span: Span) -> Result<(Vec<u8>, Provenance), ParseErr> {
        let start = self.offset.0;

        let view_len = self.view.len();
        if view_len < start.saturating_add(count) {
            return Err(ParseErr {
                kind: ParseErrKind::InputTooShort,
                provenance: self.view.provenance_from_range(start..start + 1),
                span,
            });
        }

        let count_as_usize = usize::try_from(count).unwrap();
        let mut buf = vec![0; count_as_usize];
        let window = self.view.read_at(start, &mut buf).map_err(|err| ParseErr {
            kind: ParseErrKind::Io(err),
            provenance: self.view.provenance_from_range(start..start + 1),
            span,
        })?;
        if window.len() < buf.len() {
            return Err(ParseErr {
                kind: ParseErrKind::InputTooShort,
                provenance: self.view.provenance_from_range(start..start + 1),
                span,
            });
        }

        let provenance = self.view.provenance_from_range(start..start + count);
        self.offset.0 += count;

        Ok((buf, provenance))
    }

    /// Evaluates the given expression.
    fn eval_expr(
        &self,
        expr: &Expr,
        struct_ctx: &StructContext,
        additional_ctx: AdditionalExprContext,
    ) -> Result<Value, ParseErr> {
        match &expr.kind {
            ExprKind::Lit(lit) => Ok(Value {
                kind: match lit {
                    Lit::Int(int) => ValueKind::Integer(int.clone()),
                    Lit::Bytes(bytes) => ValueKind::Bytes(bytes.clone()),
                    Lit::Bool(val) => ValueKind::Boolean(*val),
                },
                provenance: Provenance::empty(),
            }),
            ExprKind::VarUse(var) => {
                for (name, val) in &struct_ctx.parsed_fields {
                    if *name == var.inner {
                        return Ok(val.clone());
                    }
                }
                impossible!()
            }
            ExprKind::Offset => Ok(Value {
                kind: ValueKind::Integer(Int::from(self.offset.0)),
                provenance: Provenance::empty(),
            }),
            ExprKind::Parent => Ok(struct_ctx.parent.static_analysis_expect().as_value()),
            ExprKind::Last => Ok(additional_ctx.last.static_analysis_expect().clone()),
            ExprKind::Len => Ok(additional_ctx.len.static_analysis_expect().clone()),
            ExprKind::UnOp { op, operand } => {
                let Value {
                    kind: operand,
                    provenance,
                } = self.eval_expr(operand, struct_ctx, additional_ctx)?;

                Ok(match op {
                    UnOp::Neg => Value {
                        kind: ValueKind::Integer(-operand.expect_int()),
                        provenance,
                    },
                    UnOp::Plus => Value {
                        kind: operand,
                        provenance,
                    },
                    UnOp::Not => todo!(),
                })
            }
            ExprKind::BinOp { op, lhs, rhs } => {
                let Value {
                    kind: lhs,
                    mut provenance,
                } = self.eval_expr(lhs, struct_ctx, additional_ctx)?;

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
                } = self.eval_expr(rhs, struct_ctx, additional_ctx)?;
                provenance += &rhs_provenance;

                enum OpKind {
                    IntOp(fn(&Int, &Int) -> Int),
                    CmpOp(fn(&Int, &Int) -> bool),
                    Eq,
                    Neq,
                    BoolRhsIdentity,
                }

                let op_kind = match op {
                    BinOp::Add => OpKind::IntOp(|x, y| x + y),
                    BinOp::Sub => OpKind::IntOp(|x, y| x - y),
                    BinOp::Mul => OpKind::IntOp(|x, y| x * y),
                    BinOp::Div => OpKind::IntOp(|x, y| x / y),
                    BinOp::Eq => OpKind::Eq,
                    BinOp::Neq => OpKind::Neq,
                    BinOp::Gt => OpKind::CmpOp(|x, y| x > y),
                    BinOp::Geq => OpKind::CmpOp(|x, y| x >= y),
                    BinOp::Lt => OpKind::CmpOp(|x, y| x < y),
                    BinOp::Leq => OpKind::CmpOp(|x, y| x <= y),
                    BinOp::BitAnd => OpKind::IntOp(|x, y| x & y),
                    BinOp::BitOr => OpKind::IntOp(|x, y| x | y),
                    BinOp::BitXor => OpKind::IntOp(|x, y| x ^ y),
                    BinOp::LogicalAnd | BinOp::LogicalOr => OpKind::BoolRhsIdentity,
                };

                Ok(match op_kind {
                    OpKind::IntOp(func) => Value {
                        kind: ValueKind::Integer(func(lhs.expect_int(), rhs.expect_int())),
                        provenance,
                    },
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
                let expr = self.eval_expr(expr, struct_ctx, additional_ctx)?;

                Ok(expr
                    .kind
                    .expect_struct()
                    .iter()
                    .find_map(|(name, value)| (name == &field.inner).then(|| value.clone()))
                    .static_analysis_expect())
            }
            ExprKind::Peek { ty, offset } => {
                let offset = if let Some(offset_expr) = offset {
                    let offset = self.eval_expr(offset_expr, struct_ctx, additional_ctx)?;

                    if let Ok(offset) = u64::try_from(offset.kind.expect_int())
                        && offset < self.view.len()
                    {
                        ByteOffset(offset)
                    } else {
                        return self.error(
                            "new offset did not fit in available space",
                            ParseErrKind::InputTooShort,
                            &offset.provenance,
                            expr.span,
                        );
                    }
                } else {
                    self.offset
                };

                let mut scope = self.child_with_view_and_offset(self.view, offset);
                scope.eval_parse_type(ty, struct_ctx)
            }
            ExprKind::Error => impossible!(),
        }
    }

    /// Evaluates the given declaration.
    fn eval_declaration(
        &mut self,
        declaration: &Declaration,
        struct_ctx: &mut StructContext,
    ) -> Result<(), ParseErr> {
        match declaration {
            Declaration::Endianness(endianness) => self.endianness = *endianness,
            Declaration::Align(expr) => {
                let value = self.eval_expr(&expr, struct_ctx, Default::default())?;
                let align = value.kind.expect_int();
                let align = u64::try_from(align).static_analysis_expect();

                self.offset.0 = align_up(self.offset.0, align);
            }
            Declaration::SeekBy(expr) => {
                let value = self.eval_expr(&expr, struct_ctx, Default::default())?;
                let offset = value.kind.expect_int();

                if let Ok(new_offset) = u64::try_from(offset + Int::from(self.offset.0))
                    && new_offset < self.view.len()
                {
                    self.offset.0 = new_offset;
                } else {
                    return self.error(
                        "new offset did not fit in available space",
                        ParseErrKind::InputTooShort,
                        &value.provenance,
                        expr.span,
                    );
                }
            }
            Declaration::SeekTo(expr) => {
                let value = self.eval_expr(&expr, struct_ctx, Default::default())?;
                let offset = value.kind.expect_int();

                if let Ok(new_offset) = u64::try_from(offset)
                    && new_offset < self.view.len()
                {
                    self.offset.0 = new_offset;
                } else {
                    return self.error(
                        "new offset did not fit in available space",
                        ParseErrKind::InputTooShort,
                        &value.provenance,
                        expr.span,
                    );
                }
            }
            Declaration::ScopeAt {
                start,
                end,
                content,
            } => {
                let start_expr = self.eval_expr(start, struct_ctx, Default::default())?;

                let start = if let Ok(start) = u64::try_from(start_expr.kind.expect_int())
                    && start < self.view.len()
                {
                    start
                } else {
                    return self.error(
                        "scope start exceeded the end of the current scope",
                        ParseErrKind::InputTooShort,
                        &start_expr.provenance,
                        start.span,
                    );
                };

                let end = if let Some(end) = end {
                    let end_expr = self.eval_expr(end, struct_ctx, Default::default())?;

                    if let Ok(end) = u64::try_from(end_expr.kind.expect_int())
                        && end < self.view.len()
                    {
                        end
                    } else {
                        return self.error(
                            "scope end exceeded the end of the current scope",
                            ParseErrKind::InputTooShort,
                            &end_expr.provenance,
                            end.span,
                        );
                    }
                } else {
                    self.view.len()
                };

                let view = View::Subview {
                    view: &self.view,
                    valid_range: start..end,
                };
                let mut scope = self.child_with_view_and_offset(&view, ByteOffset(0));

                for single_content in content {
                    scope.eval_single_struct_content(single_content, struct_ctx);
                }
            }
            Declaration::Assert { condition, message } => {
                let condition_value = self.eval_expr(condition, struct_ctx, Default::default())?;
                if !condition_value.kind.expect_bool() {
                    let message = if let Some(message) = message {
                        let message_val =
                            self.eval_expr(message, struct_ctx, Default::default())?;

                        format!(
                            "assertion failed: {}",
                            std::str::from_utf8(message_val.kind.expect_bytes())
                                .static_analysis_expect()
                        )
                    } else {
                        String::from("assertion failed")
                    };

                    return self.error(
                        message,
                        ParseErrKind::AssertionFailure,
                        &condition_value.provenance,
                        condition.span,
                    );
                }
            }
            Declaration::WarnIf { condition, message } => {
                let condition_value = self.eval_expr(condition, struct_ctx, Default::default())?;
                if !condition_value.kind.expect_bool() {
                    let message = if let Some(message) = message {
                        let message_val =
                            self.eval_expr(message, struct_ctx, Default::default())?;
                        format!(
                            "warning triggered: {}",
                            std::str::from_utf8(message_val.kind.expect_bytes())
                                .static_analysis_expect()
                        )
                    } else {
                        String::from("warning triggered")
                    };

                    eprintln!("TODO: fix warning printing: {message}");
                }
            }
            Declaration::Recover { at } => {
                let offset = self.eval_expr(at, struct_ctx, Default::default())?;
                if let Ok(offset) = u64::try_from(offset.kind.expect_int())
                    && offset < self.view.len()
                {
                    struct_ctx.recovery_strategy = RecoveryStrategy::SkipTo {
                        offset: ByteOffset(offset),
                    };
                } else {
                    return self.error(
                        "recovery offset exceeded the end of the current scope",
                        ParseErrKind::InputTooShort,
                        &offset.provenance,
                        at.span,
                    );
                }
            }
        }

        Ok(())
    }

    /// Evaluates the given parsing type.
    fn eval_parse_type(
        &mut self,
        parse_type: &ParseType,
        struct_ctx: &StructContext,
    ) -> Result<Value, ParseErr> {
        let value = match &parse_type.kind {
            ParseTypeKind::Named { name } => {
                todo!("trying to parse named `{name:?}` unimplemented")
            }
            ParseTypeKind::Bytes { repetition_kind } => match repetition_kind {
                RepeatKind::Len { count: count_expr } => {
                    let count_val = self.eval_expr(&count_expr, struct_ctx, Default::default())?;

                    if let Ok(count) = u64::try_from(count_val.kind.expect_int()) {
                        let (bytes, provenance) = self.read_bytes(count, count_expr.span)?;

                        Value {
                            kind: ValueKind::Bytes(bytes),
                            provenance,
                        }
                    } else {
                        return self.error(
                            "count too large",
                            ParseErrKind::InputTooShort,
                            &count_val.provenance,
                            count_expr.span,
                        );
                    }
                }
                RepeatKind::While { condition } => {
                    todo!(
                        "while condition {condition:?} is unimplemented for bytes repetitions yet"
                    )
                }
                RepeatKind::Error => impossible!(),
            },
            ParseTypeKind::Integer { bit_width, signed } => {
                let bit_width = *bit_width;
                let signed = *signed;

                assert!(
                    bit_width <= 64,
                    "larger than 64-bit integers currently unimplemented"
                );
                assert!(bit_width > 0, "zero-width integers unsupported");
                assert!(
                    bit_width % 8 == 0,
                    "non byte aligned integers currently unimplemented"
                );
                let size_in_bytes = (bit_width / 8) as usize;

                let (parsed_bytes, provenance) =
                    self.read_bytes(u64::try_from(size_in_bytes).unwrap(), parse_type.span)?;

                let mut parse_buf = [0; 8];

                let (copy_start, msb) = match self.endianness {
                    Endianness::Little => (0, parsed_bytes[size_in_bytes - 1]),
                    Endianness::Big => (8 - size_in_bytes, parsed_bytes[0]),
                };

                if signed && msb & 0x80 != 0 {
                    // sign extend so the result is negative
                    parse_buf = [0xff; 8];
                }

                parse_buf[copy_start..copy_start + size_in_bytes].copy_from_slice(&parsed_bytes);
                let num = match self.endianness {
                    Endianness::Little => i64::from_le_bytes(parse_buf),
                    Endianness::Big => i64::from_be_bytes(parse_buf),
                };

                let as_int = if !signed && num < 0 {
                    (num as u64).into()
                } else {
                    num.into()
                };

                Value {
                    kind: ValueKind::Integer(as_int),
                    provenance,
                }
            }
            ParseTypeKind::Repeating {
                parse_type,
                repetition_kind,
            } => match repetition_kind {
                crate::ir::RepeatKind::Len { count } => {
                    let count_val = self.eval_expr(&count, struct_ctx, Default::default())?;

                    let mut values = Vec::new();
                    let mut provenance = Provenance::empty();

                    if let Ok(count) = u64::try_from(count_val.kind.expect_int()) {
                        for _ in 0..count {
                            let parsed_value = self.eval_parse_type(&*parse_type, struct_ctx)?;
                            provenance += &parsed_value.provenance;
                            values.push(parsed_value);
                        }
                    } else {
                        return self.error(
                            "count too large",
                            ParseErrKind::InputTooShort,
                            &count_val.provenance,
                            count.span,
                        );
                    }

                    Value {
                        kind: ValueKind::Array(values),
                        provenance,
                    }
                }
                crate::ir::RepeatKind::While { condition } => {
                    let mut values = Vec::new();
                    let mut provenance = Provenance::empty();

                    while self
                        .eval_expr(
                            condition,
                            struct_ctx,
                            AdditionalExprContext {
                                last: values.last(),
                                len: Some(&Value {
                                    kind: ValueKind::Integer(Int::from(values.len())),
                                    provenance: Provenance::empty(),
                                }),
                            },
                        )?
                        .kind
                        .expect_bool()
                    {
                        let parsed_value = self.eval_parse_type(&*parse_type, struct_ctx)?;
                        provenance += &parsed_value.provenance;
                        values.push(parsed_value);
                    }

                    Value {
                        kind: ValueKind::Array(values),
                        provenance,
                    }
                }
                crate::ir::RepeatKind::Error => impossible!(),
            },
            ParseTypeKind::Struct { content } => {
                let mut ctx = struct_ctx.child();

                for single_content in content {
                    self.eval_single_struct_content(single_content, &mut ctx);
                }

                ctx.into_value()
            }
            ParseTypeKind::Switch {
                scrutinee,
                branches,
                default,
            } => {
                let scrutinee_val = self.eval_expr(scrutinee, struct_ctx, Default::default())?;

                'result: {
                    for (lit, parse_type) in branches {
                        if scrutinee_val.kind == *lit {
                            break 'result self.eval_parse_type(parse_type, struct_ctx)?;
                        }
                    }

                    self.eval_parse_type(default, struct_ctx)?
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
        struct_ctx: &mut StructContext,
    ) -> Result<(), ParseErr> {
        let Ok(value) = self.eval_parse_type(&field.ty, struct_ctx) else {
            todo!("proper error handling")
        };

        if let Some(expected) = &field.expected {
            let span = expected.span;
            let Ok(expected) = self.eval_expr(&expected, struct_ctx, Default::default()) else {
                todo!("error")
            };
            if expected != value {
                return self.error(
                    format!(
                        "field expectation failed: {:?} != {:?}",
                        &expected.kind, &value.kind
                    ),
                    ParseErrKind::ExpectationFailure,
                    &expected.provenance,
                    span,
                );
            }
        }

        // TODO: use resolved names here later
        struct_ctx
            .parsed_fields
            .push((field.name.inner.clone(), value));

        Ok(())
    }

    /// Evaluates the given `let` statement.
    fn eval_let_statement(
        &mut self,
        let_statement: &LetStatement,
        struct_ctx: &mut StructContext,
    ) -> Result<(), ParseErr> {
        let value = self.eval_expr(&let_statement.expr, struct_ctx, Default::default())?;

        // TODO: use resolved names here later
        struct_ctx
            .parsed_fields
            .push((let_statement.name.inner.clone(), value));

        Ok(())
    }

    /// Evaluates the given single `struct` content.
    fn eval_single_struct_content(
        &mut self,
        content: &StructContent,
        struct_ctx: &mut StructContext,
    ) -> Result<(), ParseErr> {
        match content {
            StructContent::Field(field) => self.eval_struct_field(field, struct_ctx),
            StructContent::Declaration(declaration) => {
                self.eval_declaration(declaration, struct_ctx)
            }
            StructContent::LetStatement(let_statement) => {
                self.eval_let_statement(let_statement, struct_ctx)
            }
            StructContent::Error => impossible!(),
        }
    }

    /// Evaluates the content of a `struct`.
    fn eval_struct_content(
        &mut self,
        content: &[StructContent],
        struct_ctx: &mut StructContext,
    ) -> Result<(), ParseErr> {
        for content in content {
            self.eval_single_struct_content(content, struct_ctx);
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

/// Aligns the given number towards the maximum value.
///
/// `align` must be a power of two.
const fn align_up(num: u64, align: u64) -> u64 {
    align_down(num + (align - 1), align)
}

/// Aligns the given number towards zero.
///
/// `align` must be a power of two.
const fn align_down(num: u64, align: u64) -> u64 {
    num & !(align - 1)
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
