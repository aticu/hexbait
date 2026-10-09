//! Implements an intermediate representation the hexbait language.

use std::{fmt, sync::Arc};

use hexbait_common::Endianness;
use smol_str::SmolStr;

use crate::{
    Int,
    compile::{Span, syntax::SyntaxToken},
};

pub use analysis::{check_expr, check_file};
pub use expr::*;
pub use lowering::{lower_expr, lower_file};
pub use str::str_lit_content_to_bytes;

mod analysis;
mod expr;
mod lowering;
pub mod path;
mod str;

/// A name in the language.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Symbol(SmolStr);

impl fmt::Debug for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<SyntaxToken> for Symbol {
    fn from(token: SyntaxToken) -> Self {
        Symbol(token.text().into())
    }
}

impl Symbol {
    /// Returns the text of this symbol as a string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A symbol in the language along with a span.
pub struct Spanned<T> {
    /// The text of the symbol.
    pub inner: T,
    /// The span of the symbol.
    pub span: Span,
}

impl<T: fmt::Debug> fmt::Debug for Spanned<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}@{:?}", self.inner, self.span)
    }
}

impl<T: From<SyntaxToken>> From<SyntaxToken> for Spanned<T> {
    fn from(token: SyntaxToken) -> Self {
        let span = Span::from(token.text_range());
        Spanned {
            inner: T::from(token),
            span,
        }
    }
}

/// A single file in the hexbait language.
#[derive(Debug)]
pub struct File {
    /// The content that makes up the file.
    pub content: Vec<StructContent>,
}

/// The content of a struct.
#[derive(Debug)]
pub struct StructContent {
    /// The kind of the content.
    pub kind: StructContentKind,
    /// The span of the content.
    pub span: Span,
}

/// The possible kind of a `struct` content in the hexbait language.
#[derive(Debug)]
pub enum StructContentKind {
    /// A field of the `struct`.
    Field(StructField),
    /// A declaration in the `struct`.
    Declaration(Declaration),
    /// A `let` statement.
    LetStatement(LetStatement),
    /// A `struct` content that contained an error during parsing.
    Error,
}

/// A single block with struct content.
#[derive(Debug)]
pub struct Block {
    /// The content of the block.
    pub content: Vec<StructContent>,
    /// The span of the block.
    pub span: Span,
}

/// A field of a `struct`.
#[derive(Debug)]
pub struct StructField {
    /// The name of the `struct` field.
    pub name: Spanned<Symbol>,
    /// The type of the `struct` field without any modifiers applied to it.
    pub ty: ParseType,
    /// The expected value for this field, if one exists.
    pub expected: Option<Expr>,
}

/// A `let` statement.
#[derive(Debug)]
pub struct LetStatement {
    /// The name of the computed value.
    pub name: Spanned<Symbol>,
    /// The expression that computes the value.
    pub expr: Expr,
}

/// A `scope` kind.
#[derive(Debug)]
pub enum ScopeKind {
    /// Defines a scope by a start and an optional end offset in the current scope.
    At {
        /// The start offset of the scope relative to the current parent.
        start: Expr,
        /// The end offset of the scope relative to the current parent.
        end: Option<Expr>,
    },
    /// Defines a scope by the bytes which make up the content of the scope.
    In {
        /// The bytes used for parsing.
        bytes: Expr,
    },
}

/// A declaration found in a `struct`.
#[derive(Debug)]
pub struct Declaration {
    /// The kind of the declaration.
    pub kind: DeclarationKind,
    /// The span of the declaration.
    pub span: Span,
}

/// The kind of a declaration found in a `struct`.
#[derive(Debug)]
pub enum DeclarationKind {
    /// Declares the endianness.
    Endianness(Endianness),
    /// Aligns to a certain number of bytes.
    Align(Expr),
    /// Seeks by a specified amount.
    SeekBy(Expr),
    /// Seeks to a specified position.
    SeekTo(Expr),
    /// Parses the contained fields in a separate scope.
    Scope {
        /// The kind of the scope.
        kind: ScopeKind,
        /// The content block of the scope.
        block: Block,
    },
    If(IfChain),
    /// Asserts that the given expression is true.
    Assert {
        /// The condition that needs to hold.
        condition: Expr,
        /// The message to display if the condition is false.
        message: Option<Expr>,
    },
    /// Warns if the given expression is true.
    WarnIf {
        /// The condition that will trigger a warning.
        condition: Expr,
        /// The message to display if the condition is true.
        message: Option<Expr>,
    },
    /// Specifies an offset to recover at in case of errors.
    Recover {
        /// The offset at which to recover.
        at: Expr,
    },
}

/// A chain of `if` statements.
#[derive(Debug)]
pub struct IfChain {
    /// The `if` blocks.
    ///
    /// These are executed in order and execution stops once the first is discovered.
    pub if_blocks: Vec<IfBlock>,
    /// The else part of the if chain.
    pub else_block: Option<Block>,
}

/// A conditionally executed block.
#[derive(Debug)]
pub struct IfBlock {
    /// The expression that determines if the block is executed.
    pub condition: Expr,
    /// The block that is executed if the condition is `true`.
    pub block: Block,
}

/// A description of a parsing type.
#[derive(Debug)]
pub struct ParseType {
    /// The kind of parsing type.
    pub kind: ParseTypeKind,
    /// The span of the parsing type.
    pub span: Span,
}

/// The different types that can be parsed.
#[derive(Debug)]
pub enum ParseTypeKind {
    /// Parses a type of the given name.
    Named {
        /// The name of the type to parse.
        name: Spanned<Symbol>,
    },
    /// Parses an integer with a given bit width from the input.
    Integer {
        /// The bit width to use.
        bit_width: u32,
        /// Whether the integer is signed.
        signed: bool,
    },
    /// Parses an integer of dynamic size.
    DynamicInteger {
        /// The bit width to use.
        bit_width: Expr,
        /// Whether the integer is signed.
        signed: bool,
    },
    /// Parses an array of contiguous bytes.
    Bytes {
        /// The repetition that determines the number of bytes to parse.
        repetition: Repetition,
    },
    /// Parses another parse type repeatedly with a given repetition kind.
    Repeating {
        /// The parse type to parse.
        parse_type: Box<ParseType>,
        /// The repetition.
        repetition: Repetition,
    },
    /// Parses an anonymous `struct` declaration.
    Struct {
        /// The content of the `struct`.
        block: Block,
    },
    /// Parses one of multiple other parse types depending on the value of `scrutinee`.
    Switch {
        /// The value determining which branch to take.
        scrutinee: Expr,
        /// The branches of the `switch` parse type.
        branches: Vec<(Lit, Span, ParseType)>,
        /// The default branch if no other branch matches.
        default: Box<ParseType>,
    },
    /// Parses an integer and maps it to variants.
    Enum {
        /// The backing integer type of the enum.
        backing_type: Box<ParseType>,
        /// The info that is attached to the parsed integer value.
        ///
        /// This is an [`Arc`] to allow values to cheaply clone it and reference it too.
        enum_info: Arc<EnumInfo>,
    },
    /// A parse type that contained an error during parsing.
    Error,
}

/// The information on the fields of an enum.
#[derive(Debug)]
pub struct EnumInfo {
    /// The variants of the enum.
    pub variants: Vec<EnumVariant>,
}

impl EnumInfo {
    /// Resolves the value to a possible variant.
    pub fn resolve(&self, value: &Int) -> Option<&EnumVariant> {
        self.variants
            .iter()
            .find(|variant| match &variant.kind.inner {
                EnumVariantKind::SingleValue(val) => val == value,
                EnumVariantKind::Range { start, end } => (start..=end).contains(&value),
            })
    }
}

/// A single variant of an enum.
#[derive(Debug)]
pub struct EnumVariant {
    /// The name of the variant.
    pub name: Spanned<Symbol>,
    /// The kind of the variant.
    pub kind: Spanned<EnumVariantKind>,
}

/// The kind of an enum variant.
#[derive(Debug)]
pub enum EnumVariantKind {
    /// Only a single value maps to the variant.
    SingleValue(Int),
    /// Any value in the range maps to the variant.
    Range {
        /// The start of the range (inclusive).
        start: Int,
        /// The end of the range (inclusive).
        end: Int,
    },
}

/// The type of a repetition of a repeating parse type.
#[derive(Debug)]
pub struct Repetition {
    /// The kind of the repetition.
    pub kind: RepeatKind,
    /// The span of the repetition declaration.
    pub span: Span,
}

/// The kind of a repetition.
#[derive(Debug)]
pub enum RepeatKind {
    /// Repeats a fixed number of times.
    Len {
        /// The number of times to repeat.
        count: Expr,
    },
    /// Repeats while the condition is true.
    While {
        /// The condition that determines whether another instance is parsed.
        condition: Expr,
    },
    /// A repeat kind that contained an error during parsing.
    Error,
}
