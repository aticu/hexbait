//! Implements finding of magic values in a struct.

use crate::compile::ir::{
    Block, DeclarationKind, Expr, ExprKind, File, Lit, ParseType, ParseTypeKind, RepeatKind,
    Repetition, StructContent, StructContentKind,
};

/// A magic value in a format definition.
#[derive(Debug)]
pub struct MagicValue<'file> {
    /// The value of the magic bytes.
    value: &'file [u8],
    /// The offset into the structure where the magic bytes are located.
    offset: usize,
}

impl<'file> MagicValue<'file> {
    /// The value of the magic bytes.
    pub fn value(&self) -> &'file [u8] {
        self.value
    }

    /// The offset into the structure where the magic bytes are located.
    pub fn offset(&self) -> usize {
        self.offset
    }
}

impl File {
    /// Finds the magic bytes for the format described by the `File`.
    pub fn magic(&self) -> Option<MagicValue<'_>> {
        let mut offset = 0;

        for content in &self.content {
            if let Some(magic) = content.magic() {
                return Some(MagicValue {
                    value: magic.value(),
                    offset: magic.offset + offset,
                });
            } else {
                offset += content.fixed_size()?;
            }
        }

        None
    }
}

impl StructContent {
    /// Finds the magic bytes for the struct content.
    fn magic(&self) -> Option<MagicValue<'_>> {
        match &self.kind {
            StructContentKind::Field(struct_field)
                if matches!(
                    struct_field.ty.kind,
                    ParseTypeKind::Bytes {
                        repetition: Repetition {
                            kind: RepeatKind::Len { count: _ },
                            span: _
                        }
                    }
                ) && let Some(expected) = &struct_field.expected
                    && let ExprKind::Lit(Lit::Bytes(bytes_lit)) = &expected.kind =>
            {
                Some(MagicValue {
                    value: bytes_lit,
                    offset: 0,
                })
            }
            StructContentKind::Field(field) => match &field.ty.kind {
                ParseTypeKind::Named { .. }
                | ParseTypeKind::Integer { .. }
                | ParseTypeKind::DynamicInteger { .. }
                | ParseTypeKind::Bytes { .. }
                | ParseTypeKind::Repeating { .. }
                | ParseTypeKind::Switch { .. }
                | ParseTypeKind::Enum { .. }
                | ParseTypeKind::Flags { .. }
                | ParseTypeKind::Error => None,
                ParseTypeKind::Struct { block } => block.magic(),
            },
            StructContentKind::Declaration(_)
            | StructContentKind::LetStatement(_)
            | StructContentKind::IfChain(_) => None,
            StructContentKind::Scope(scope) => scope.block.magic(),
            StructContentKind::Error => None,
        }
    }

    /// Returns the fixed size occupied by this struct content.
    fn fixed_size(&self) -> Option<usize> {
        match &self.kind {
            StructContentKind::Field(struct_field) => struct_field.ty.fixed_size(),
            StructContentKind::Declaration(declaration) => match &declaration.kind {
                DeclarationKind::Align(_) => None, // here we could be smarter by passing the current offset
                DeclarationKind::SeekBy(expr) => expr.fixed_int_val(),
                DeclarationKind::SeekTo(_) => None, // here we could be smarter by passing the current offset
                DeclarationKind::Endianness(_)
                | DeclarationKind::Assert { .. }
                | DeclarationKind::WarnIf { .. }
                | DeclarationKind::Recover { .. } => Some(0),
            },
            StructContentKind::LetStatement(_) => Some(0),
            StructContentKind::IfChain(if_chain) => {
                let size = if_chain.if_blocks[0].block.fixed_size()?;
                for branch in &if_chain.if_blocks[1..] {
                    if branch.block.fixed_size()? != size {
                        return None;
                    }
                }
                if let Some(else_branch) = &if_chain.else_block
                    && else_branch.fixed_size()? != size
                {
                    return None;
                }

                Some(size)
            }
            StructContentKind::Scope(scope) => scope.block.fixed_size(),
            StructContentKind::Error => None,
        }
    }
}

impl Block {
    /// Returns the magic value of this block.
    fn magic(&self) -> Option<MagicValue<'_>> {
        let mut offset = 0;

        for content in &self.content {
            if let Some(magic) = content.magic() {
                return Some(MagicValue {
                    value: magic.value(),
                    offset: magic.offset + offset,
                });
            } else {
                offset += content.fixed_size()?;
            }
        }

        None
    }

    /// Returns the fixed size for this block.
    fn fixed_size(&self) -> Option<usize> {
        let mut size = 0;
        for content in &self.content {
            size += content.fixed_size()?;
        }
        Some(size)
    }
}

impl ParseType {
    /// Returns the fixed size for this parse type.
    fn fixed_size(&self) -> Option<usize> {
        match &self.kind {
            ParseTypeKind::Named { .. } => unimplemented!(),
            ParseTypeKind::Integer {
                bit_width,
                signed: _,
            } => {
                if bit_width.is_multiple_of(8) {
                    Some(*bit_width as usize / 8)
                } else {
                    None
                }
            }
            ParseTypeKind::DynamicInteger { .. } => None,
            ParseTypeKind::Bytes { repetition } => repetition.fixed_count(),
            ParseTypeKind::Repeating {
                parse_type,
                repetition,
            } => Some(parse_type.fixed_size()? * repetition.fixed_count()?),
            ParseTypeKind::Struct { block } => block.fixed_size(),
            ParseTypeKind::Switch {
                scrutinee: _,
                branches,
                default,
            } => {
                let size = default.fixed_size()?;
                for (_, _, branch_ty) in branches {
                    if branch_ty.fixed_size()? != size {
                        return None;
                    }
                }
                Some(size)
            }
            ParseTypeKind::Enum {
                backing_type,
                enum_info: _,
            }
            | ParseTypeKind::Flags {
                backing_type,
                flags_info: _,
            } => backing_type.fixed_size(),
            ParseTypeKind::Error => None,
        }
    }
}

impl Repetition {
    /// Returns the fixed repetition count of this repetition.
    fn fixed_count(&self) -> Option<usize> {
        match &self.kind {
            RepeatKind::Len { count } => count.fixed_int_val(),
            RepeatKind::While { .. } => None,
            RepeatKind::Error => None,
        }
    }
}

impl Expr {
    /// Returns the fixed integer value of the expression.
    fn fixed_int_val(&self) -> Option<usize> {
        match &self.kind {
            ExprKind::Lit(Lit::Int(count)) => usize::try_from(count).ok(),
            _ => None,
        }
    }
}
