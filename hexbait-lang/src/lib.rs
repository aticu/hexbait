//! Implements the hexbait format description language.

pub mod ast;
mod eval;
pub mod ir;
mod lexer;
mod parser;
mod span;
mod syntax;

pub use {
    eval::{parse::eval_ir, view::View},
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

// TODO: add optional field to reflect max counts for count parsing
