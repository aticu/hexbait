//! Implements general GUI state.

use hexbait_lang::{compile::Diagnostics, eval::ParseResult};

/// General GUI state.
///
/// This contains things that are almost always visible and don't belong to one system in particular.
pub struct GeneralGuiState {
    /// The content of the `Jump to` field.
    pub jump_to_expr: String,
    /// The error to show for a `Jump to` expression.
    pub jump_to_error: Option<JumpToError>,
}

impl GeneralGuiState {
    /// Creates a new general GUI state.
    pub fn new() -> GeneralGuiState {
        GeneralGuiState {
            jump_to_expr: String::new(),
            jump_to_error: None,
        }
    }
}

impl Default for GeneralGuiState {
    fn default() -> Self {
        GeneralGuiState::new()
    }
}

/// The different errors that can occur while evaluation a `Jump to` expression.
pub enum JumpToError {
    /// A compilation error for the expression.
    Compile(Diagnostics),
    /// An error that occurred while evaluating the expression.
    Eval(ParseResult),
    /// A generic other error.
    Message(String),
}
