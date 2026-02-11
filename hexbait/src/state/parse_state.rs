//! Implements the state for the hexbait parser.

use std::{collections::BTreeMap, path::PathBuf};

use hexbait_builtin_parsers::built_in_format_descriptions;

/// The state of the hexbait parser.
pub struct ParseState {
    /// The name of the type that should be parsed.
    pub parse_type: &'static str,
    /// The offset at which to parse.
    pub parse_offset: String,
    /// Whether the parse offset should be synced to the start of the selection.
    pub sync_parse_offset_to_selection_start: bool,
    /// The built-in format description.
    pub built_in_format_descriptions: BTreeMap<&'static str, hexbait_lang::ir::File>,
    /// The path to the custom parser definition.
    pub custom_parser: Option<PathBuf>,
}

impl ParseState {
    /// Creates a new parse state.
    pub fn new(custom_parser: Option<PathBuf>) -> ParseState {
        ParseState {
            parse_type: "none",
            parse_offset: String::from("0"),
            sync_parse_offset_to_selection_start: true,
            built_in_format_descriptions: built_in_format_descriptions(),
            custom_parser,
        }
    }
}
