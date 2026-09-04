//! Implements the state for the hexbait console.

use hexbait_lang::{compile::Diagnostics, eval::ParseResult};

/// The state of the hexbait console.
#[derive(Debug)]
pub struct ConsoleState {
    /// The currently entered text in the console.
    current_text: String,
    /// The history of console entries.
    history: Vec<ConsoleEntry>,
}

impl ConsoleState {
    /// Creates a new console state.
    pub fn new() -> ConsoleState {
        ConsoleState {
            current_text: String::new(),
            history: Vec::new(),
        }
    }

    /// Returns a mutable reference to the current text.
    pub fn current_text(&mut self) -> &mut String {
        &mut self.current_text
    }

    /// Adds a console history entry.
    pub fn add_history_entry(&mut self, entry: ConsoleEntry) {
        self.history.push(entry);
    }

    /// Returns the console history.
    pub fn history(&self) -> &[ConsoleEntry] {
        &self.history
    }
}

impl Default for ConsoleState {
    fn default() -> Self {
        ConsoleState::new()
    }
}

/// An entry in the hexbait console history.
#[derive(Debug, Clone)]
pub struct ConsoleEntry {
    /// The query that produced this entry.
    pub query: String,
    /// The diagnostics encountered during compilation.
    pub diagnostics: Option<Diagnostics>,
    /// The result of executing the query.
    pub result: Option<ParseResult>,
}
