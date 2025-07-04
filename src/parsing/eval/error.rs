//! Provides the errors that can occur during parsing.

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
