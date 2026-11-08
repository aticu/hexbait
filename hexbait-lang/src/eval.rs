//! Implements evaluation of the parser.

pub(crate) mod parse;
mod provenance;
mod value;
pub(crate) mod view;

pub use parse::{
    Diagnostic, DiagnosticId, DiagnosticLevel, ParseResult, eval_repl_statement, eval_struct,
};
pub use provenance::Provenance;
pub use value::{BytesValue, StructContent, Value, ValueKind};
pub use view::View;
