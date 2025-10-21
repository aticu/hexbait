//! Implements support for [`Spans`] that mark locations in source code.

use std::fmt;

use rowan::TextRange;

/// A location in source code.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    /// The start offset of the location, inclusive.
    pub(crate) start: usize,
    /// The end offset of the location, exclusive.
    pub(crate) end: usize,
}

impl fmt::Debug for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}..{}", self.start, self.end)
    }
}

impl From<TextRange> for Span {
    fn from(text_range: TextRange) -> Self {
        Span {
            start: usize::try_from(text_range.start()).expect("content fits a usize"),
            end: usize::try_from(text_range.end()).expect("content fits a usize"),
        }
    }
}
