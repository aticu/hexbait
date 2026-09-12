//! Implements support for [`Spans`] that mark locations in source code.

use std::{fmt, ops::Range};

use rowan::TextRange;

/// A location in source code.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    /// The start offset of the location, inclusive.
    start: usize,
    /// The end offset of the location, exclusive.
    end: usize,
}

impl Span {
    /// Creates a `Span` from a start and an end.
    ///
    /// # Panics
    /// This function panics if `start > end`.
    pub fn from_start_end(start: usize, end: usize) -> Span {
        assert!(start <= end);
        Span { start, end }
    }

    /// Computes whether `self` contains `other`.
    pub fn contains(self, other: Span) -> bool {
        // inclusive because of zero-width spans
        self.start <= other.start && other.end <= self.end
    }

    /// The range of text this span covers.
    pub fn range(self) -> Range<usize> {
        self.start..self.end
    }
}

impl fmt::Debug for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}..{}", self.start, self.end)
    }
}

impl From<TextRange> for Span {
    fn from(text_range: TextRange) -> Self {
        assert!(text_range.start() <= text_range.end());
        Span {
            start: usize::from(text_range.start()),
            end: usize::from(text_range.end()),
        }
    }
}
