//! Implements an intermediate representation the hexbait language.

use std::fmt;

use hexbait_common::Endianness;
use smol_str::SmolStr;

use crate::{SyntaxToken, span::Span};

pub use analysis::check_ir;
pub use expr::*;
pub use lowering::lower_file;
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

/// The possible content of a `struct` in the hexbait language.
#[derive(Debug)]
pub enum StructContent {
    /// A field of the `struct`.
    Field(StructField),
    /// A declaration in the `struct`.
    Declaration(Declaration),
    /// A `let` statement.
    LetStatement(LetStatement),
    /// A `struct` content that contained an error during parsing.
    Error,
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

/// A declaration found in a `struct`.
#[derive(Debug)]
pub enum Declaration {
    /// Declares the endianness.
    Endianness(Endianness),
    /// Aligns to a certain number of bytes.
    Align(Expr),
    /// Seeks by a specified amount.
    SeekBy(Expr),
    /// Seeks to a specified position.
    SeekTo(Expr),
    /// Parses the contained fields in a separate scope.
    ScopeAt {
        /// The start offset of the scope relative to the current parent.
        start: Expr,
        /// The end offset of the scope relative to the current parent.
        end: Option<Expr>,
        /// The content of the scope.
        content: Vec<StructContent>,
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
    /// The condition that decides which branch to take.
    pub condition: Expr,
    /// The content to parse if the condition is true.
    pub then_block: Vec<StructContent>,
    /// The else part of the if chain.
    pub else_part: Option<ElsePart>,
}

/// The `else` part of an if chain.
#[derive(Debug)]
pub enum ElsePart {
    /// An else block that is the end of the chain.
    ElseBlock(Vec<StructContent>),
    /// Another nested if chain.
    IfChain(Box<IfChain>),
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
    /// Parses an array of contiguous bytes.
    Bytes {
        /// The repetition that determines the number of bytes to parse.
        repetition_kind: RepeatKind,
    },
    /// Parses another parse type repeatedly with a given repetition kind.
    Repeating {
        /// The parse type to parse.
        parse_type: Box<ParseType>,
        /// The repetition kind.
        repetition_kind: RepeatKind,
    },
    /// Parses an anonymous `struct` declaration.
    Struct {
        /// The content of the `struct`.
        content: Vec<StructContent>,
    },
    /// Parses one of multiple other parse types depending on the value of `scrutinee`.
    Switch {
        /// The value determining which branch to take.
        scrutinee: Expr,
        /// The branches of the `switch` parse type.
        branches: Vec<(Lit, ParseType)>,
        /// The default branch if no other branch matches.
        default: Box<ParseType>,
    },
    /// A parse type that contained an error during parsing.
    Error,
}

/// The type of repetition of a repeating parse type.
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
