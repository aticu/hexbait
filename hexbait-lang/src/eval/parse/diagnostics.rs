//! Implements errors and warnings for parsing.

use crate::{Span, Value, eval::provenance::Provenance};

/// The result of a parsing operation.
pub type Result<T, E = ParseErr> = std::result::Result<T, E>;

/// The level of a diagnostic.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum DiagnosticLevel {
    /// The diagnostic is a parsing failure.
    ///
    /// This indicates that parsing can only continue after a `recover` declaration.
    Fail,
    /// The diagnostic is a warning.
    ///
    /// This should be used for format mismatches that aren't fatal.
    Warn,
}

/// A diagnostic that occurred during parsing.
#[derive(Debug)]
pub struct Diagnostic {
    /// The diagnostic message.
    pub message: String,
    /// The level of the diagnostic.
    pub level: DiagnosticLevel,
    /// The provenance where the diagnostic occurred.
    pub provenance: Provenance,
    /// The span of the node that produced the diagnostic.
    pub span: Span,
}

/// An ID referencing a specific diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiagnosticId {
    /// The index into the diagnostics `Vec` at which the diagnostic is stored.
    idx: usize,
}

impl DiagnosticId {
    /// Created a new diagnostic ID, by inserting it into the list of diagnostics.
    pub(crate) fn new(err: Diagnostic, vec: &mut Vec<Diagnostic>) -> DiagnosticId {
        let idx = vec.len();
        vec.push(err);
        DiagnosticId { idx }
    }

    /// Returns the raw index into the errors.
    pub fn raw_idx(self) -> usize {
        self.idx
    }
}

/// A parse error that may or may not contain partial results.
#[derive(Debug)]
pub struct ParseErr {
    /// The diagnostic that caused parsing to fail.
    error: DiagnosticId,
    /// A partial result that was parsed despite the error.
    partial_result: Option<Value>,
}

impl ParseErr {
    /// Creates a new parse error.
    pub fn new(error: DiagnosticId) -> ParseErr {
        ParseErr {
            error,
            partial_result: None,
        }
    }

    /// The diagnostics ID that caused this error.
    pub fn id(&self) -> DiagnosticId {
        self.error
    }

    /// Adds a partial result to the parse error.
    pub fn with_partial_result(self, value: Value) -> ParseErr {
        ParseErr {
            error: self.error,
            partial_result: Some(value),
        }
    }

    /// Takes the stored partial result if there is any.
    pub fn take_partial_result(&mut self) -> Option<Value> {
        self.partial_result.take()
    }
}
