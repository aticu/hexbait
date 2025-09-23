//! Implements the language used to describe parsers of binary formats.

pub mod ast;

/// The type used for integers in the language.
pub type Int = malachite_nz::integer::Integer;
