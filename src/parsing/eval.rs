//! Implements evaluation of the language.

mod error;
mod path;
mod provenance;
mod value;

pub use error::ParseErr;
pub use path::{Path, PathComponent};
pub use provenance::Provenance;
pub use value::{Value, ValueKind};

use crate::{data::DataSource, model::Endianness, window::Window};

use super::language::ast::{self, ExprKind, Node, NodeKind, Symbol};

/// Represents an offset into a data stream.
#[derive(Debug, Clone)]
pub struct Offset {
    /// The offset to the first byte that will need to be read next.
    in_bytes: u64,
}

impl Offset {
    /// Returns the next whole byte to be read.
    fn next_whole_byte(&self) -> u64 {
        self.in_bytes
    }

    /// Advances the cursor by `bytes` bytes.
    fn advance_bytes(&mut self, bytes: u64) {
        // TODO: decide how this affects bit-level offsets
        self.in_bytes += bytes;
    }
}

impl From<u64> for Offset {
    fn from(value: u64) -> Self {
        Offset { in_bytes: value }
    }
}

/// The parsing context.
#[derive(Debug)]
pub struct ParseContext {
    /// The endianness used for parsing.
    endianness: Endianness,
    /// The current offset used for parsing.
    offset: Offset,
    /// The offset at which parsing started.
    start_offset: Offset,
    /// The parsed values.
    parsed_values: Vec<(Symbol, Value)>,
}

impl ParseContext {
    /// Creates a new empty parsing context.
    pub fn with_offset(offset: Offset) -> ParseContext {
        let start_offset = offset.clone();
        ParseContext {
            endianness: Endianness::Little,
            offset,
            start_offset,
            parsed_values: Vec::new(),
        }
    }

    /// Creates a child parsing context.
    pub fn child(&self) -> ParseContext {
        ParseContext {
            endianness: self.endianness,
            offset: self.offset.clone(),
            start_offset: self.start_offset.clone(),
            parsed_values: Vec::new(),
        }
    }

    /// Evaluates the given expression.
    pub fn eval_expr<Source: DataSource>(
        &self,
        expr: &ast::Expr,
        source: &mut Source,
    ) -> Result<Value, ParseErr<Source::Error>> {
        let result = match &expr.kind {
            ExprKind::ConstantInt { value } => Value {
                kind: ValueKind::Integer(value.clone()),
                provenance: Provenance::empty(),
            },
            ExprKind::ConstantBytes { value } => Value {
                kind: ValueKind::Bytes(value.clone()),
                provenance: Provenance::empty(),
            },
            ExprKind::Offset => Value {
                kind: ValueKind::Integer(self.offset.next_whole_byte().into()),
                provenance: Provenance::empty(),
            },
            ExprKind::UnOp { operand, op } => {
                let operand = self.eval_expr(operand, source)?;

                match op {
                    ast::UnOp::Not => Value {
                        kind: ValueKind::Boolean(!operand.expect_bool()),
                        provenance: operand.provenance,
                    },
                }
            }
            ExprKind::BinOp { left, right, op } => {
                let left = self.eval_expr(left, source)?;
                let right = self.eval_expr(right, source)?;
                let combined_provenance = &left.provenance + &right.provenance;

                match op {
                    ast::BinOp::Eq => Value {
                        kind: ValueKind::Boolean(left.kind == right.kind),
                        provenance: combined_provenance,
                    },
                    ast::BinOp::Add => Value {
                        kind: ValueKind::Integer(left.expect_int() + right.expect_int()),
                        provenance: combined_provenance,
                    },
                    ast::BinOp::Sub => Value {
                        kind: ValueKind::Integer(left.expect_int() - right.expect_int()),
                        provenance: combined_provenance,
                    },
                }
            }
            ExprKind::VariableUse { var } => {
                for (name, value) in &self.parsed_values {
                    if name == var {
                        return Ok(value.clone());
                    }
                }

                unreachable!("this should be impossible because of static analysis")
            }
            ExprKind::FieldAccess {
                field_holder,
                field,
            } => {
                let field_holder = self.eval_expr(field_holder, source)?;

                for (name, value) in field_holder.expect_struct() {
                    if name == field {
                        return Ok(value.clone());
                    }
                }

                unreachable!("this should be impossible because of static analysis")
            }
            ExprKind::ParseAt { node } => self.child().parse(node, source)?,
        };

        Ok(result)
    }

    /// Parses the given node into the current node context.
    pub fn parse<Source: DataSource>(
        &mut self,
        node: &Node,
        source: &mut Source,
    ) -> Result<Value, ParseErr<Source::Error>> {
        if let Some(offset) = &node.offset {
            let offset_val = self.eval_expr(offset, source)?;
            let offset = offset_val.expect_int();

            if let Ok(offset) = u64::try_from(offset) {
                // add the start offset here, since the nodes cannot possibly know about that
                self.offset.in_bytes = offset + self.start_offset.next_whole_byte();
            } else {
                // TODO: handle expectation failures
            }
        }

        let value = match &node.kind {
            NodeKind::FixedBytes { expected } => {
                let expected = self.eval_expr(expected, source)?;
                let bytes = expected.expect_bytes();

                let (window, parsed_bytes) = read_bytes(
                    source,
                    &mut self.offset,
                    u64::try_from(bytes.len()).unwrap(),
                )?;

                if parsed_bytes != bytes {
                    // TODO: handle expectation failures
                }

                Value {
                    kind: ValueKind::Bytes(parsed_bytes),
                    provenance: Provenance::from_window(window),
                }
            }
            NodeKind::FixedLength { length } => {
                let length_val = self.eval_expr(length, source)?;
                let length = length_val.expect_int();

                let (window, parsed_bytes) =
                    read_bytes(source, &mut self.offset, u64::try_from(length).unwrap())?;

                Value {
                    kind: ValueKind::Bytes(parsed_bytes),
                    provenance: Provenance::from_window(window),
                }
            }
            NodeKind::Integer { bit_width, signed } => {
                assert!(
                    *bit_width <= 64,
                    "larger than 64-bit integers currently unimplemented"
                );
                assert!(*bit_width > 0, "zero-width integers unsupported");
                assert!(
                    bit_width % 8 == 0,
                    "non byte aligned integers currently unimplemented"
                );
                let size_in_bytes = (bit_width / 8) as usize;

                let (window, parsed_bytes) = read_bytes(
                    source,
                    &mut self.offset,
                    u64::try_from(size_in_bytes).unwrap(),
                )?;

                let mut parse_buf = [0; 8];

                let (copy_start, msb) = match self.endianness {
                    Endianness::Little => (0, parsed_bytes[size_in_bytes - 1]),
                    Endianness::Big => (8 - size_in_bytes, parsed_bytes[0]),
                };

                if *signed && msb & 0x80 != 0 {
                    // sign extend so the result is negative
                    parse_buf = [0xff; 8];
                }

                parse_buf[copy_start..copy_start + size_in_bytes].copy_from_slice(&parsed_bytes);
                let num = self.endianness.i64_from_bytes()(parse_buf);

                let as_int = if !signed && num < 0 {
                    (num as u64).into()
                } else {
                    num.into()
                };

                Value {
                    kind: ValueKind::Integer(as_int),
                    provenance: Provenance::from_window(window),
                }
            }
            NodeKind::Float { bit_width } => match *bit_width {
                32 => {
                    let (window, parsed_bytes) = read_bytes(source, &mut self.offset, 4)?;
                    Value {
                        kind: ValueKind::Float(self.endianness.f32_from_bytes()(
                            parsed_bytes.try_into().unwrap(),
                        ) as f64),
                        provenance: Provenance::from_window(window),
                    }
                }
                64 => {
                    let (window, parsed_bytes) = read_bytes(source, &mut self.offset, 4)?;
                    Value {
                        kind: ValueKind::Float(self.endianness.f64_from_bytes()(
                            parsed_bytes.try_into().unwrap(),
                        )),
                        provenance: Provenance::from_window(window),
                    }
                }
                _ => unreachable!("only 32-bit and 64-bit floats are supported"),
            },
            NodeKind::Struct { nodes } => {
                let mut child_ctx = self.child();

                let mut provenance = Provenance::empty();
                for node in nodes {
                    let value = child_ctx.parse(node, source)?;
                    provenance += &value.provenance;
                    child_ctx.parsed_values.push((node.name.clone(), value));
                }

                let parsed_values = child_ctx.parsed_values;
                self.offset = child_ctx.offset;

                Value {
                    kind: ValueKind::Struct(parsed_values),
                    provenance,
                }
            }
            NodeKind::RepeatCount { node, count } => {
                let count_val = self.eval_expr(count, source)?;
                let count = count_val.expect_int();
                let mut nodes = Vec::new();

                let mut provenance = Provenance::empty();

                if let Ok(count) = usize::try_from(count) {
                    for _ in 0..count {
                        let parsed_node = self.parse(node, source)?;
                        provenance += &parsed_node.provenance;
                        nodes.push(parsed_node);
                    }
                } else {
                    // TODO: handle expectation failures here (count is larger than fits into
                    // memory)
                }

                Value {
                    kind: ValueKind::Array(nodes),
                    provenance,
                }
            }
            NodeKind::RepeatWhile { node, condition } => {
                let mut nodes = Vec::new();

                let mut provenance = Provenance::empty();

                while self.eval_expr(condition, source)?.expect_bool() {
                    let parsed_node = self.parse(node, source)?;
                    provenance += &parsed_node.provenance;
                    nodes.push(parsed_node);
                }

                Value {
                    kind: ValueKind::Array(nodes),
                    provenance,
                }
            }
        };

        Ok(value)
    }
}

/// Reads `count` bytes from `source`, using and adjusting the `offset` accordingly.
fn read_bytes<Source: DataSource>(
    source: &mut Source,
    offset: &mut Offset,
    count: u64,
) -> Result<(Window, Vec<u8>), ParseErr<Source::Error>> {
    let count_as_usize = usize::try_from(count).unwrap();
    let mut buf = vec![0; count_as_usize];
    let start = offset.next_whole_byte();
    let window = source.window_at(start, &mut buf)?;
    if window.len() < count_as_usize {
        return Err(ParseErr::InputTooShort);
    }
    offset.advance_bytes(count);

    let window = Window::from_start_len(start, count);

    Ok((window, buf))
}
