//! Implements the structures storing the search state.

use std::borrow::Cow;

use hexbait_common::Input;
use hexbait_lang::ir::str_lit_content_to_bytes;

use crate::search::Searcher;

/// The search state.
pub struct SearchState {
    /// The searcher to perform searches.
    pub searcher: Searcher,
    /// The text to search for.
    pub search_text: String,
    /// Whether to search case insensitive (ASCII only).
    pub search_ascii_case_insensitive: bool,
    /// Whether to search for a UTF-16 version of the input.
    pub search_utf16: bool,
}

impl SearchState {
    /// Creates a new search state.
    pub fn new(input: &Input) -> SearchState {
        SearchState {
            searcher: Searcher::new(input),
            search_text: String::new(),
            search_ascii_case_insensitive: true,
            search_utf16: true,
        }
    }

    /// Returns the bytes to search for or an error message.
    pub fn search_bytes(&self) -> Result<Vec<u8>, Cow<'static, str>> {
        let mut search_bytes = Vec::new();

        match str_lit_content_to_bytes(&self.search_text, &mut search_bytes) {
            Ok(()) => Ok(search_bytes),
            Err((msg, _)) => Err(msg),
        }
    }
}
