//! Implements the hexbait format description language.

#![forbid(unsafe_code)]

pub mod ast;
mod eval;
pub mod ir;
mod lexer;
mod parser;
mod span;
mod syntax;

pub use {
    eval::*,
    ir::check_ir,
    parser::parse,
    span::Span,
    syntax::{Language, NodeKind, SyntaxKind, SyntaxNode, SyntaxToken},
};

/// The integer type used for arbitrary precision integers.
pub type Int = num_bigint::BigInt;

/// Parses the given string into an integer of the given base.
fn int_from_str(base: u32, s: &str) -> Option<Int> {
    <Int as num_traits::Num>::from_str_radix(s, base).ok()
}

// TODO: add optional field to reflect max counts for count parsing -> or implement max function
// TODO: implement display options (enum that name certain values)
// TODO: implement bitwise fields
// TODO: implement custom data streams
// TODO: implement classification of parsed values (offset, integer?, string?)
// TODO: improve display of the parsed values in the GUI
// TODO: figure out a way to cleverly incorporate colors
// TODO: implement a new concept of "scopes" in the file to reset endianness (and others) at the end of `!scope` and `struct`s
