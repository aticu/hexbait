//! Implements errors and warnings for parsing.

use std::io;

use crate::{Span, Value, eval::provenance::Provenance};

/// An error that occurred during parsing.
#[derive(Debug)]
pub enum ParseErrKind {
    /// The input was shorter than expected.
    InputTooShort,
    /// A value that is meant as an offset was too large.
    OffsetTooLarge,
    /// An arithmetic error occurred while evaluating an expression.
    ArithmeticError,
    /// An assertion failed.
    AssertionFailure,
    /// An assertion failed.
    ExpectationFailure,
    /// An I/O error occurred during parsing.
    Io(io::Error),
}

impl From<io::Error> for ParseErrKind {
    fn from(err: io::Error) -> Self {
        ParseErrKind::Io(err)
    }
}

/// An error that occurred during parsing.
#[derive(Debug)]
pub struct ParseErr {
    /// The error message.
    pub message: String,
    /// The kind of error that occurred.
    pub kind: ParseErrKind,
    /// The provenance where the error occurred.
    pub provenance: Provenance,
    /// The span of the node where parsing failed.
    pub span: Span,
}

/// An ID referencing a specific parsing error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseErrId {
    /// The index into the error array at which the error occurs.
    idx: usize,
}

impl ParseErrId {
    /// Created a new parse error ID, but inserting it into the list of parsing errors.
    pub(crate) fn new(err: ParseErr, vec: &mut Vec<ParseErr>) -> ParseErrId {
        let idx = vec.len();
        vec.push(err);
        ParseErrId { idx }
    }

    /// Returns the raw index into the errors.
    pub fn raw_idx(self) -> usize {
        self.idx
    }
}

/// A parse error that may or may not contain partial results.
#[derive(Debug)]
pub(crate) struct ParseErrWithMaybePartialResult {
    /// The parse error.
    pub(crate) parse_err: ParseErrId,
    /// A partial result that was parsed despite the error.
    pub(crate) partial_result: Option<Value>,
}

impl From<ParseErrId> for ParseErrWithMaybePartialResult {
    fn from(parse_err: ParseErrId) -> Self {
        ParseErrWithMaybePartialResult {
            parse_err,
            partial_result: None,
        }
    }
}

/// A warning that occurred during parsing.
#[derive(Debug)]
pub struct ParseWarning {
    /// The warning message.
    pub message: String,
    /// The provenance where the warning occurred.
    pub provenance: Provenance,
    /// The span of the node that triggered the warning.
    pub span: Span,
}
