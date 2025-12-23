//! Implements the parsing evaluation logic.

use std::fmt;

use crate::{
    Int, Span,
    eval::parse::diagnostics::ParseErrWithMaybePartialResult,
    ir::{
        BinOp, Declaration, ElsePart, Expr, ExprKind, File, IfChain, LetStatement, Lit, ParseType,
        ParseTypeKind, RepeatKind, StructContent, StructField, Symbol, UnOp,
    },
};

use super::{
    provenance::Provenance,
    value::{Value, ValueKind},
    view::View,
};

pub use diagnostics::{ParseErr, ParseErrId, ParseErrKind, ParseWarning};
use hexbait_common::Endianness;

mod diagnostics;

/// An offset in bytes to parse from.
#[derive(Debug, Clone, Copy)]
struct ByteOffset(u64);

impl TryFrom<&Value> for ByteOffset {
    type Error = ParseErrKind;

    fn try_from(value: &Value) -> Result<Self, Self::Error> {
        u64::try_from(value.kind.expect_int())
            .map(ByteOffset)
            .map_err(|_| ParseErrKind::OffsetTooLarge)
    }
}

/// The result of parsing.
pub struct ParseResult {
    /// The parsed value.
    pub value: Value,
    /// The errors that occurred during parsing.
    pub errors: Vec<ParseErr>,
    /// The warnings that occurred during parsing.
    pub warnings: Vec<ParseWarning>,
}

/// Evaluates the given IR on the given input.
pub fn eval_ir(file: &File, view: View<'_>, start_offset: u64) -> ParseResult {
    let mut struct_ctx = StructContext::new();
    let mut scope = Scope::new(&view);
    scope.offset = ByteOffset(start_offset);

    let mut parse_ctx = ParseContext {
        errors: Vec::new(),
        warnings: Vec::new(),
    };

    scope
        .eval_struct_content(&file.content, &mut struct_ctx, &mut parse_ctx)
        .ok();

    ParseResult {
        value: struct_ctx.into_value(),
        errors: parse_ctx.errors,
        warnings: parse_ctx.warnings,
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
    /// The errors that occurred during parsing.
    errors: Vec<ParseErr>,
    /// The warnings that occurred during parsing.
    warnings: Vec<ParseWarning>,
}

impl ParseContext {
    /// Creates a new error in the parsing context.
    fn new_err(&mut self, err: ParseErr) -> ParseErrId {
        ParseErrId::new(err, &mut self.errors)
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
    /// An error that may have occurred during parsing of this struct.
    error: Option<ParseErrId>,
    /// The offset where the parsing of this `struct` started.
    start_offset: ByteOffset,
}

impl<'parent> StructContext<'parent> {
    /// Creates a new `struct` parsing context.
    fn new() -> StructContext<'static> {
        StructContext {
            parsed_fields: Vec::new(),
            parent: None,
            recovery_strategy: RecoveryStrategy::Fallback,
            error: None,
            // will be set to the correct value when the parsing starts
            start_offset: ByteOffset(0),
        }
    }

    /// Creates the context for a child `struct`.
    fn child<'this>(&'this self) -> StructContext<'this> {
        StructContext {
            parsed_fields: Vec::new(),
            parent: Some(self),
            recovery_strategy: RecoveryStrategy::Fallback,
            error: None,
            // will be set to the correct value when the parsing starts
            start_offset: ByteOffset(0),
        }
    }

    /// Returns the `struct` context as a partially parsed `struct` value.
    fn as_value(&self) -> Value {
        let mut provenance = Provenance::empty();
        for (_, value) in &self.parsed_fields {
            provenance += &value.provenance;
        }

        Value {
            kind: ValueKind::Struct {
                fields: self.parsed_fields.clone(),
                error: self.error,
            },
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
            kind: ValueKind::Struct {
                fields: self
                    .parsed_fields
                    .into_iter()
                    .filter(|(name, _)| !name.as_str().starts_with('_'))
                    .collect(),
                error: self.error,
            },
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

    /// Reads the specified number of bytes.
    fn read_bytes(
        &mut self,
        count: u64,
        span: Span,
        parse_ctx: &mut ParseContext,
    ) -> Result<(Vec<u8>, Provenance), ParseErrId> {
        let start = self.offset.0;

        let view_len = self.view.len();
        if view_len < start.saturating_add(count) {
            return Err(parse_ctx.new_err(ParseErr {
                message: "view is too short".into(),
                kind: ParseErrKind::InputTooShort,
                provenance: self.view.provenance_from_range(start..start + 1),
                span,
            }));
        }

        let count_as_usize = usize::try_from(count).unwrap();
        let mut buf = vec![0; count_as_usize];
        let window = self.view.read_at(start, &mut buf).map_err(|err| {
            parse_ctx.new_err(ParseErr {
                message: format!("io error: {err}"),
                kind: ParseErrKind::Io(err),
                provenance: self.view.provenance_from_range(start..start + 1),
                span,
            })
        })?;
        if window.len() < buf.len() {
            return Err(parse_ctx.new_err(ParseErr {
                message: "view is too short".into(),
                kind: ParseErrKind::InputTooShort,
                provenance: self.view.provenance_from_range(start..start + 1),
                span,
            }));
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
        parse_ctx: &mut ParseContext,
        additional_ctx: AdditionalExprContext,
    ) -> Result<Value, ParseErrId> {
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
                } = self.eval_expr(operand, struct_ctx, parse_ctx, additional_ctx)?;

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
                } = self.eval_expr(lhs, struct_ctx, parse_ctx, additional_ctx)?;

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
                } = self.eval_expr(rhs, struct_ctx, parse_ctx, additional_ctx)?;
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
                                parse_ctx.new_err(ParseErr {
                                    message,
                                    kind: ParseErrKind::ArithmeticError,
                                    provenance: provenance.clone(),
                                    span: expr.span,
                                })
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
                let expr = self.eval_expr(expr, struct_ctx, parse_ctx, additional_ctx)?;

                Ok(expr
                    .kind
                    .expect_struct()
                    .iter()
                    .find_map(|(name, value)| (name == &field.inner).then(|| value.clone()))
                    .static_analysis_expect())
            }
            ExprKind::Peek { ty, offset } => {
                let offset = if let Some(offset_expr) = offset {
                    let offset =
                        self.eval_expr(offset_expr, struct_ctx, parse_ctx, additional_ctx)?;

                    if let Ok(offset) = u64::try_from(offset.kind.expect_int())
                        && offset <= self.view.len()
                    {
                        ByteOffset(offset)
                    } else {
                        return Err(parse_ctx.new_err(ParseErr {
                            message: "new offset did not fit in available space".into(),
                            kind: ParseErrKind::InputTooShort,
                            provenance: offset.provenance.clone(),
                            span: expr.span,
                        }));
                    }
                } else {
                    self.offset
                };

                let mut scope = self.child_with_view_and_offset(self.view, offset);
                scope
                    .eval_parse_type(ty, struct_ctx, parse_ctx)
                    .map_err(|err| err.parse_err)
            }
            ExprKind::Error => impossible!(),
        }
    }

    /// Evaluates the given declaration.
    fn eval_declaration(
        &mut self,
        declaration: &Declaration,
        struct_ctx: &mut StructContext,
        parse_ctx: &mut ParseContext,
    ) -> Result<(), ParseErrWithMaybePartialResult> {
        match declaration {
            Declaration::Endianness(endianness) => self.endianness = *endianness,
            Declaration::Align(expr) => {
                let value = self.eval_expr(expr, struct_ctx, parse_ctx, Default::default())?;
                let align = value.kind.expect_int();
                let align = u64::try_from(align).static_analysis_expect();

                self.offset.0 = align_up(self.offset.0, align);
            }
            Declaration::SeekBy(expr) => {
                let value = self.eval_expr(expr, struct_ctx, parse_ctx, Default::default())?;
                let offset = value.kind.expect_int();

                if let Ok(new_offset) = u64::try_from(offset + Int::from(self.offset.0))
                    && new_offset <= self.view.len()
                {
                    self.offset.0 = new_offset;
                } else {
                    return Err(parse_ctx
                        .new_err(ParseErr {
                            message: "new offset did not fit in available space".into(),
                            kind: ParseErrKind::InputTooShort,
                            provenance: value.provenance.clone(),
                            span: expr.span,
                        })
                        .into());
                }
            }
            Declaration::SeekTo(expr) => {
                let value = self.eval_expr(expr, struct_ctx, parse_ctx, Default::default())?;
                let offset = value.kind.expect_int();

                if let Ok(new_offset) = u64::try_from(offset)
                    && new_offset <= self.view.len()
                {
                    self.offset.0 = new_offset;
                } else {
                    return Err(parse_ctx
                        .new_err(ParseErr {
                            message: "new offset did not fit in available space".into(),
                            kind: ParseErrKind::InputTooShort,
                            provenance: value.provenance.clone(),
                            span: expr.span,
                        })
                        .into());
                }
            }
            Declaration::ScopeAt {
                start,
                end,
                content,
            } => {
                let start_expr =
                    self.eval_expr(start, struct_ctx, parse_ctx, Default::default())?;

                let start = if let Ok(start) = u64::try_from(start_expr.kind.expect_int())
                    && start <= self.view.len()
                {
                    start
                } else {
                    return Err(parse_ctx
                        .new_err(ParseErr {
                            message: "scope start exceeded the end of the current scope".into(),
                            kind: ParseErrKind::InputTooShort,
                            provenance: start_expr.provenance.clone(),
                            span: start.span,
                        })
                        .into());
                };

                let end = if let Some(end) = end {
                    let end_expr =
                        self.eval_expr(end, struct_ctx, parse_ctx, Default::default())?;

                    if let Ok(end) = u64::try_from(end_expr.kind.expect_int())
                        && end <= self.view.len()
                    {
                        end
                    } else {
                        return Err(parse_ctx
                            .new_err(ParseErr {
                                message: "scope end exceeded the end of the current scope".into(),
                                kind: ParseErrKind::InputTooShort,
                                provenance: end_expr.provenance.clone(),
                                span: end.span,
                            })
                            .into());
                    }
                } else {
                    self.view.len()
                };

                let view = View::Subview {
                    view: self.view,
                    valid_range: start..end,
                };
                let mut scope = self.child_with_view_and_offset(&view, ByteOffset(0));

                for single_content in content {
                    scope.eval_single_struct_content(single_content, struct_ctx, parse_ctx)?;
                }
            }
            Declaration::If(if_chain) => {
                self.eval_if_chain(if_chain, struct_ctx, parse_ctx)?;
            }
            Declaration::Assert { condition, message } => {
                let condition_value =
                    self.eval_expr(condition, struct_ctx, parse_ctx, Default::default())?;
                if !condition_value.kind.expect_bool() {
                    let message = if let Some(message) = message {
                        let message_val =
                            self.eval_expr(message, struct_ctx, parse_ctx, Default::default())?;

                        format!(
                            "assertion failed: {}",
                            std::str::from_utf8(message_val.kind.expect_bytes())
                                .static_analysis_expect()
                        )
                    } else {
                        String::from("assertion failed")
                    };

                    return Err(parse_ctx
                        .new_err(ParseErr {
                            message,
                            kind: ParseErrKind::AssertionFailure,
                            provenance: condition_value.provenance.clone(),
                            span: condition.span,
                        })
                        .into());
                }
            }
            Declaration::WarnIf { condition, message } => {
                let condition_value =
                    self.eval_expr(condition, struct_ctx, parse_ctx, Default::default())?;
                if !condition_value.kind.expect_bool() {
                    let message = if let Some(message) = message {
                        let message_val =
                            self.eval_expr(message, struct_ctx, parse_ctx, Default::default())?;
                        format!(
                            "warning triggered: {}",
                            std::str::from_utf8(message_val.kind.expect_bytes())
                                .static_analysis_expect()
                        )
                    } else {
                        String::from("warning triggered")
                    };

                    parse_ctx.warnings.push(ParseWarning {
                        message,
                        provenance: condition_value.provenance.clone(),
                        span: condition.span,
                    });
                }
            }
            Declaration::Recover { at } => {
                let offset = self.eval_expr(at, struct_ctx, parse_ctx, Default::default())?;
                if let Ok(offset) = u64::try_from(offset.kind.expect_int())
                    && let Some(offset) = offset.checked_add(struct_ctx.start_offset.0)
                    && offset <= self.view.len()
                {
                    struct_ctx.recovery_strategy = RecoveryStrategy::SkipTo {
                        offset: ByteOffset(offset),
                    };
                } else {
                    return Err(parse_ctx
                        .new_err(ParseErr {
                            message: "recovery offset exceeded the end of the current scope".into(),
                            kind: ParseErrKind::InputTooShort,
                            provenance: offset.provenance.clone(),
                            span: at.span,
                        })
                        .into());
                }
            }
        }

        Ok(())
    }

    /// Evaluates the given `if` chain.
    fn eval_if_chain(
        &mut self,
        if_chain: &IfChain,
        struct_ctx: &mut StructContext,
        parse_ctx: &mut ParseContext,
    ) -> Result<(), ParseErrWithMaybePartialResult> {
        let condition = self.eval_expr(
            &if_chain.condition,
            struct_ctx,
            parse_ctx,
            Default::default(),
        )?;

        if condition.kind.expect_bool() {
            for single_content in &if_chain.then_block {
                self.eval_single_struct_content(single_content, struct_ctx, parse_ctx)?;
            }
        } else if let Some(else_part) = &if_chain.else_part {
            match else_part {
                ElsePart::IfChain(if_chain) => {
                    self.eval_if_chain(if_chain, struct_ctx, parse_ctx)?;
                }
                ElsePart::ElseBlock(else_block) => {
                    for single_content in else_block {
                        self.eval_single_struct_content(single_content, struct_ctx, parse_ctx)?;
                    }
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
        parse_ctx: &mut ParseContext,
    ) -> Result<Value, ParseErrWithMaybePartialResult> {
        let value = match &parse_type.kind {
            ParseTypeKind::Named { name } => {
                todo!("trying to parse named `{name:?}` unimplemented")
            }
            ParseTypeKind::Bytes { repetition_kind } => match repetition_kind {
                RepeatKind::Len { count: count_expr } => {
                    let count_val =
                        self.eval_expr(count_expr, struct_ctx, parse_ctx, Default::default())?;

                    if let Ok(count) = u64::try_from(count_val.kind.expect_int()) {
                        let (bytes, provenance) =
                            self.read_bytes(count, count_expr.span, parse_ctx)?;

                        Value {
                            kind: ValueKind::Bytes(bytes),
                            provenance,
                        }
                    } else {
                        return Err(ParseErrWithMaybePartialResult {
                            parse_err: parse_ctx.new_err(ParseErr {
                                message: "count too large".into(),
                                kind: ParseErrKind::InputTooShort,
                                provenance: count_val.provenance.clone(),
                                span: count_expr.span,
                            }),
                            partial_result: None,
                        });
                    }
                }
                RepeatKind::While { condition } => {
                    todo!(
                        "while condition {condition:?} is unimplemented for bytes repetitions yet"
                    )
                }
                RepeatKind::Error => impossible!(),
            },
            ParseTypeKind::Integer { signed, .. }
            | ParseTypeKind::DynamicInteger { signed, .. } => {
                let bit_width = match &parse_type.kind {
                    ParseTypeKind::Integer { bit_width, .. } => *bit_width,
                    ParseTypeKind::DynamicInteger { bit_width, .. } => {
                        let val =
                            self.eval_expr(bit_width, struct_ctx, parse_ctx, Default::default())?;

                        u32::try_from(val.kind.expect_int()).map_err(|_| {
                            parse_ctx.new_err(ParseErr {
                                message: "bit width is too large".to_string(),
                                kind: ParseErrKind::ArithmeticError,
                                provenance: val.provenance,
                                span: bit_width.span,
                            })
                        })?
                    }
                    _ => unreachable!(),
                };
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

                let (parsed_bytes, provenance) = self.read_bytes(
                    u64::try_from(size_in_bytes).unwrap(),
                    parse_type.span,
                    parse_ctx,
                )?;

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
                    let count_val =
                        self.eval_expr(count, struct_ctx, parse_ctx, Default::default())?;

                    let mut values = Vec::new();
                    let mut provenance = Provenance::empty();

                    if let Ok(count) = u64::try_from(count_val.kind.expect_int()) {
                        for _ in 0..count {
                            match self.eval_parse_type(parse_type, struct_ctx, parse_ctx) {
                                Ok(parsed_value) => {
                                    provenance += &parsed_value.provenance;
                                    values.push(parsed_value);
                                }
                                Err(err) => {
                                    if let Some(partial_result) = err.partial_result {
                                        provenance += &partial_result.provenance;
                                        values.push(partial_result);
                                    }
                                    return Err(ParseErrWithMaybePartialResult {
                                        parse_err: err.parse_err,
                                        partial_result: Some(Value {
                                            kind: ValueKind::Array {
                                                items: values,
                                                error: Some(err.parse_err),
                                            },
                                            provenance,
                                        }),
                                    });
                                }
                            };
                        }
                    } else {
                        return Err(ParseErrWithMaybePartialResult {
                            parse_err: parse_ctx.new_err(ParseErr {
                                message: "count too large".into(),
                                kind: ParseErrKind::InputTooShort,
                                provenance: count_val.provenance.clone(),
                                span: count.span,
                            }),
                            partial_result: None,
                        });
                    }

                    Value {
                        kind: ValueKind::Array {
                            items: values,
                            error: None,
                        },
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
                            parse_ctx,
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
                        match self.eval_parse_type(parse_type, struct_ctx, parse_ctx) {
                            Ok(parsed_value) => {
                                provenance += &parsed_value.provenance;
                                values.push(parsed_value);
                            }
                            Err(err) => {
                                if let Some(partial_result) = err.partial_result {
                                    provenance += &partial_result.provenance;
                                    values.push(partial_result);
                                }
                                return Err(ParseErrWithMaybePartialResult {
                                    parse_err: err.parse_err,
                                    partial_result: Some(Value {
                                        kind: ValueKind::Array {
                                            items: values,
                                            error: Some(err.parse_err),
                                        },
                                        provenance,
                                    }),
                                });
                            }
                        };
                    }

                    Value {
                        kind: ValueKind::Array {
                            items: values,
                            error: None,
                        },
                        provenance,
                    }
                }
                crate::ir::RepeatKind::Error => impossible!(),
            },
            ParseTypeKind::Struct { content } => {
                let mut ctx = struct_ctx.child();

                match self.eval_struct_content(content, &mut ctx, parse_ctx) {
                    Ok(()) => ctx.into_value(),
                    Err(mut err) => {
                        // the partial result should have already been added at this point
                        assert!(err.partial_result.is_none());

                        err.partial_result = Some(ctx.into_value());

                        Err(err)?
                    }
                }
            }
            ParseTypeKind::Switch {
                scrutinee,
                branches,
                default,
            } => {
                let scrutinee_val =
                    self.eval_expr(scrutinee, struct_ctx, parse_ctx, Default::default())?;

                'result: {
                    for (lit, parse_type) in branches {
                        if scrutinee_val.kind == *lit {
                            break 'result self
                                .eval_parse_type(parse_type, struct_ctx, parse_ctx)?;
                        }
                    }

                    self.eval_parse_type(default, struct_ctx, parse_ctx)?
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
        parse_ctx: &mut ParseContext,
    ) -> Result<(), ParseErrWithMaybePartialResult> {
        let value = self.eval_parse_type(&field.ty, struct_ctx, parse_ctx)?;

        if let Some(expected) = &field.expected {
            let span = expected.span;
            let expected = self.eval_expr(expected, struct_ctx, parse_ctx, Default::default())?;
            if expected != value {
                return Err(ParseErrWithMaybePartialResult {
                    parse_err: parse_ctx.new_err(ParseErr {
                        message: format!(
                            "field expectation failed: {:?} != {:?}",
                            &expected.kind, &value.kind
                        ),
                        kind: ParseErrKind::ExpectationFailure,
                        provenance: expected.provenance.clone(),
                        span,
                    }),
                    partial_result: Some(value),
                });
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
        parse_ctx: &mut ParseContext,
    ) -> Result<(), ParseErrId> {
        let value = self.eval_expr(
            &let_statement.expr,
            struct_ctx,
            parse_ctx,
            Default::default(),
        )?;

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
        parse_ctx: &mut ParseContext,
    ) -> Result<(), ParseErrWithMaybePartialResult> {
        match content {
            StructContent::Field(field) => {
                match self.eval_struct_field(field, struct_ctx, parse_ctx) {
                    Ok(()) => Ok(()),
                    Err(err) => {
                        if let Some(partial_result) = err.partial_result {
                            // TODO: use resolved names here later
                            struct_ctx
                                .parsed_fields
                                .push((field.name.inner.clone(), partial_result));
                        }
                        Err(ParseErrWithMaybePartialResult {
                            parse_err: err.parse_err,
                            partial_result: None,
                        })
                    }
                }
            }
            StructContent::Declaration(declaration) => {
                Ok(self.eval_declaration(declaration, struct_ctx, parse_ctx)?)
            }
            StructContent::LetStatement(let_statement) => {
                Ok(self.eval_let_statement(let_statement, struct_ctx, parse_ctx)?)
            }
            StructContent::Error => impossible!(),
        }
    }

    /// Evaluates the content of a `struct`.
    fn eval_struct_content(
        &mut self,
        content: &[StructContent],
        struct_ctx: &mut StructContext,
        parse_ctx: &mut ParseContext,
    ) -> Result<(), ParseErrWithMaybePartialResult> {
        struct_ctx.start_offset = self.offset;

        for content in content {
            match self.eval_single_struct_content(content, struct_ctx, parse_ctx) {
                Ok(()) => (),
                Err(err) => {
                    struct_ctx.error = Some(err.parse_err);
                    match &struct_ctx.recovery_strategy {
                        RecoveryStrategy::Fallback => return Err(err),
                        RecoveryStrategy::SkipTo { offset } => {
                            self.offset = *offset;

                            return Ok(());
                        }
                    }
                }
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
