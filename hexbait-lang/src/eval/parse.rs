//! Implements the parsing evaluation logic.

use std::{fmt, io};

use crate::{
    Int, Span,
    ir::{
        BinOp, Declaration, Endianness, Expr, ExprKind, File, Lit, ParseType, StructContent,
        StructField, Symbol, UnOp,
    },
};

use super::{
    provenance::Provenance,
    value::{Value, ValueKind},
    view::View,
};

/// An offset in bytes to parse from.
#[derive(Debug)]
struct ByteOffset(u64);

/// An error that occurred during parsing.
#[derive(Debug)]
pub enum ParseErr {
    /// The input was shorter than expected.
    InputTooShort,
    /// An I/O error occurred during parsing.
    Io(io::Error),
}

impl From<io::Error> for ParseErr {
    fn from(err: io::Error) -> Self {
        ParseErr::Io(err)
    }
}

/// Evaluates the given IR on the given input.
pub fn eval_ir(file: &File, view: View<'_>) -> Value {
    let mut ctx = StructContext::new();
    let mut scope = Scope::new(&mut ctx, view);

    for content in &file.content {
        scope.eval_struct_content(content);
    }

    let mut provenance = Provenance::empty();
    for (_, value) in &ctx.parsed_fields {
        provenance += &value.provenance;
    }

    Value {
        kind: ValueKind::Struct(ctx.parsed_fields),
        provenance,
    }
}

macro_rules! impossible {
    () => {
        unreachable!("impossible because of static analysis")
    };
}

/// The parsing context for a `struct`.
#[derive(Debug)]
struct StructContext {
    /// The endianness used for parsing.
    endianness: Endianness,
    /// The already parsed fields.
    parsed_fields: Vec<(Symbol, Value)>,
}

impl StructContext {
    /// Creates a new `struct` parsing context.
    fn new() -> StructContext {
        StructContext {
            // static analysis makes sure that this is set to the correct value before parsing
            endianness: Endianness::Little,
            parsed_fields: Vec::new(),
        }
    }

    /// Creates a child context from the current context.
    fn child(&self) -> StructContext {
        StructContext {
            endianness: self.endianness,
            parsed_fields: Vec::new(),
        }
    }
}

/// The parsing context for a `scope`.
#[derive(Debug)]
struct Scope<'src, 'struct_ctx> {
    /// The `struct` context of this scope.
    struct_ctx: &'struct_ctx mut StructContext,
    /// The current offset used for parsing.
    offset: ByteOffset,
    /// The view that this scope parses from.
    view: View<'src>,
}

impl<'src, 'struct_ctx> Scope<'src, 'struct_ctx> {
    /// Creates a new `scope` for the given `struct` context in the given view.
    fn new(
        struct_ctx: &'struct_ctx mut StructContext,
        view: View<'src>,
    ) -> Scope<'src, 'struct_ctx> {
        Scope {
            struct_ctx,
            offset: ByteOffset(0),
            view,
        }
    }

    /// Reports the given error at the given location.
    fn error(&mut self, message: impl Into<String>, location: &Provenance, span: Span) {
        eprintln!(
            "TODO: add proper error handling: {} at {location:?} here {span:?}",
            message.into()
        )
    }

    /// Returns the view for the current scope.
    fn current_view(&mut self) -> &mut View<'src> {
        &mut self.view
    }

    /// Reads the specified number of bytes.
    fn read_bytes(&mut self, count: u64) -> Result<(Vec<u8>, Provenance), ParseErr> {
        let start = self.offset.0;
        let view = self.current_view();

        let view_len = view.len()?;
        if view_len < start.saturating_add(count) {
            return Err(ParseErr::InputTooShort);
        }

        let count_as_usize = usize::try_from(count).unwrap();
        let mut buf = vec![0; count_as_usize];
        let window = view.read_at(start, &mut buf)?;
        if window.len() < buf.len() {
            return Err(ParseErr::InputTooShort);
        }

        let provenance = view.provenance_from_range(start..start + count);
        self.offset.0 += count;

        Ok((buf, provenance))
    }

    /// Evaluates the given expression.
    fn eval_expr(&self, expr: &Expr) -> Value {
        match &expr.kind {
            ExprKind::Lit(lit) => Value {
                kind: match lit {
                    Lit::Int(int) => ValueKind::Integer(int.clone()),
                    Lit::Bytes(bytes) => ValueKind::Bytes(bytes.clone()),
                },
                provenance: Provenance::empty(),
            },
            ExprKind::VarUse(var) => {
                for (name, val) in &self.struct_ctx.parsed_fields {
                    if *name == var.inner {
                        return val.clone();
                    }
                }
                impossible!()
            }
            ExprKind::UnOp { op, operand } => {
                let Value {
                    kind: operand,
                    provenance,
                } = self.eval_expr(operand);

                match op {
                    UnOp::Neg => Value {
                        kind: ValueKind::Integer(-operand.expect_int()),
                        provenance,
                    },
                    UnOp::Plus => Value {
                        kind: operand,
                        provenance,
                    },
                    UnOp::Not => todo!(),
                }
            }
            ExprKind::BinOp { op, lhs, rhs } => {
                let Value {
                    kind: lhs,
                    mut provenance,
                } = self.eval_expr(lhs);
                let Value {
                    kind: rhs,
                    provenance: rhs_provenance,
                } = self.eval_expr(rhs);
                provenance += &rhs_provenance;

                enum OpKind {
                    IntOp(fn(&Int, &Int) -> Int),
                    CmpOp(fn(&Int, &Int) -> bool),
                }

                let op_kind = match op {
                    BinOp::Add => OpKind::IntOp(|x, y| x + y),
                    BinOp::Sub => OpKind::IntOp(|x, y| x - y),
                    BinOp::Mul => OpKind::IntOp(|x, y| x * y),
                    BinOp::Div => OpKind::IntOp(|x, y| x / y),
                    BinOp::Eq => OpKind::CmpOp(|x, y| x == y),
                    BinOp::Neq => OpKind::CmpOp(|x, y| x != y),
                    BinOp::Gt => OpKind::CmpOp(|x, y| x > y),
                    BinOp::Geq => OpKind::CmpOp(|x, y| x >= y),
                    BinOp::Lt => OpKind::CmpOp(|x, y| x < y),
                    BinOp::Leq => OpKind::CmpOp(|x, y| x <= y),
                };

                match op_kind {
                    OpKind::IntOp(func) => Value {
                        kind: ValueKind::Integer(func(lhs.expect_int(), rhs.expect_int())),
                        provenance,
                    },
                    OpKind::CmpOp(func) => Value {
                        kind: ValueKind::Boolean(func(lhs.expect_int(), rhs.expect_int())),
                        provenance,
                    },
                }
            }
            ExprKind::Error => impossible!(),
        }
    }

    /// Evaluates the given declaration.
    fn eval_declaration(&mut self, declaration: &Declaration) -> Result<(), ParseErr> {
        match declaration {
            Declaration::Endianness(endianness) => self.struct_ctx.endianness = *endianness,
            Declaration::Align(expr) => {
                let value = self.eval_expr(&expr);
                let align = value.kind.expect_int();
                let align = u64::try_from(align).static_analysis_expect();

                self.offset.0 = align_up(self.offset.0, align);
            }
            Declaration::SeekBy(expr) => {
                let value = self.eval_expr(&expr);
                let offset = value.kind.expect_int();

                if let Ok(new_offset) = u64::try_from(offset + Int::from(self.offset.0))
                    && new_offset < self.current_view().len()?
                {
                    self.offset.0 = new_offset;
                } else {
                    self.error(
                        "new offset did not fit in available space",
                        &value.provenance,
                        expr.span,
                    );
                }
            }
            Declaration::SeekTo(expr) => {
                let value = self.eval_expr(&expr);
                let offset = value.kind.expect_int();

                if let Ok(new_offset) = u64::try_from(offset)
                    && new_offset < self.current_view().len()?
                {
                    self.offset.0 = new_offset;
                } else {
                    self.error(
                        "new offset did not fit in available space",
                        &value.provenance,
                        expr.span,
                    );
                }
            }
            Declaration::ScopeAt { start, content } => {
                let start_expr = self.eval_expr(start);
                let len = self.current_view().len()?;

                let start = if let Ok(start) = u64::try_from(start_expr.kind.expect_int())
                    && start < len
                {
                    start
                } else {
                    self.error(
                        "scope start exceeded the end of the current scope",
                        &start_expr.provenance,
                        start.span,
                    );
                    return Ok(());
                    // TODO: check what to return here
                };

                let view = View::Subview {
                    view: &self.view,
                    valid_range: start..len,
                };
                let mut scope = Scope::new(self.struct_ctx, view);

                for single_content in content {
                    scope.eval_struct_content(single_content);
                }
            }
        }

        Ok(())
    }

    /// Evaluates the given parsing type.
    fn eval_parse_type(&mut self, parse_type: &ParseType) -> Result<Value, ParseErr> {
        let value = match parse_type {
            ParseType::Named { name } => todo!(),
            ParseType::Bytes { repetition_kind } => match repetition_kind {
                crate::ir::RepeatKind::Len { count } => {
                    let count_val = self.eval_expr(&count);
                    let count = count_val.kind.expect_int();

                    if let Ok(count) = u64::try_from(count) {
                        let (bytes, provenance) = self.read_bytes(count)?;

                        Value {
                            kind: ValueKind::Bytes(bytes),
                            provenance,
                        }
                    } else {
                        self.error(
                            "count too large",
                            &count_val.provenance,
                            todo!("add spans here"),
                        );
                        return Ok(Value {
                            kind: ValueKind::Err,
                            provenance: count_val.provenance,
                        });
                    }
                }
                crate::ir::RepeatKind::While { condition } => todo!(),
                crate::ir::RepeatKind::Error => impossible!(),
            },
            ParseType::Integer { bit_width, signed } => {
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
                    self.read_bytes(u64::try_from(size_in_bytes).unwrap())?;

                let mut parse_buf = [0; 8];

                let (copy_start, msb) = match self.struct_ctx.endianness {
                    Endianness::Little => (0, parsed_bytes[size_in_bytes - 1]),
                    Endianness::Big => (8 - size_in_bytes, parsed_bytes[0]),
                };

                if signed && msb & 0x80 != 0 {
                    // sign extend so the result is negative
                    parse_buf = [0xff; 8];
                }

                parse_buf[copy_start..copy_start + size_in_bytes].copy_from_slice(&parsed_bytes);
                let num = match self.struct_ctx.endianness {
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
            ParseType::Repeating {
                parse_type,
                repetition_kind,
            } => match repetition_kind {
                crate::ir::RepeatKind::Len { count } => {
                    let count_val = self.eval_expr(&count);
                    let count = count_val.kind.expect_int();

                    let mut values = Vec::new();
                    let mut provenance = Provenance::empty();

                    if let Ok(count) = u64::try_from(count) {
                        for _ in 0..count {
                            let parsed_value = self.eval_parse_type(&*parse_type)?;
                            provenance += &parsed_value.provenance;
                            values.push(parsed_value);
                        }
                    } else {
                        self.error(
                            "count too large",
                            &count_val.provenance,
                            todo!("add spans here"),
                        );
                        return Ok(Value {
                            kind: ValueKind::Err,
                            provenance,
                        });
                    }

                    Value {
                        kind: ValueKind::Array(values),
                        provenance,
                    }
                }
                crate::ir::RepeatKind::While { condition } => todo!(),
                crate::ir::RepeatKind::Error => impossible!(),
            },
            ParseType::Struct { content } => todo!(),
            ParseType::Error => impossible!(),
        };

        Ok(value)
    }

    /// Evaluates the given `struct` field.
    fn eval_struct_field(&mut self, field: &StructField) {
        let Ok(value) = self.eval_parse_type(&field.ty) else {
            todo!("proper error handling")
        };

        if let Some(expected) = &field.expected {
            let span = expected.span;
            let expected = self.eval_expr(&expected);
            if expected != value {
                self.error(
                    format!(
                        "field expectation failed: {:?} != {:?}",
                        &expected.kind, &value.kind
                    ),
                    &expected.provenance,
                    span,
                );
            }
        }

        // TODO: use resolved names here later
        self.struct_ctx
            .parsed_fields
            .push((field.name.inner.clone(), value));
    }

    /// Evaluates the given `struct` content.
    fn eval_struct_content(&mut self, content: &StructContent) {
        match content {
            StructContent::Field(field) => self.eval_struct_field(field),
            StructContent::Declaration(declaration) => {
                if let Err(err) = self.eval_declaration(declaration) {
                    todo!("handle err: {err:?}");
                }
            }
            StructContent::Error => impossible!(),
        }
    }
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

    fn static_analysis_expect(self) -> Self::Target {
        self.expect("impossible because of static analysis")
    }
}

impl<T, E: fmt::Debug> StaticAnalysisImpossible for Result<T, E> {
    type Target = T;

    fn static_analysis_expect(self) -> Self::Target {
        self.expect("impossible because of static analysis")
    }
}
