//! Implements evaluation of the parser.

pub(crate) mod parse;
mod provenance;
mod value;
pub(crate) mod view;

pub use value::{Value, ValueKind};
