//! Provides the errors that can occur during parsing.

use std::fmt;

/// Represents the possible parsing errors.
#[derive(Debug)]
pub enum ParseErr<SourceErr> {
    /// The input was too short to finish parsing.
    InputTooShort,
    /// An input source specific error occurred.
    SourceErr(SourceErr),
}

impl<SourceErr> From<SourceErr> for ParseErr<SourceErr> {
    fn from(value: SourceErr) -> Self {
        ParseErr::SourceErr(value)
    }
}

impl<SourceErr: fmt::Display> fmt::Display for ParseErr<SourceErr> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseErr::InputTooShort => f.write_str("input too short"),
            ParseErr::SourceErr(err) => err.fmt(f),
        }
    }
}
