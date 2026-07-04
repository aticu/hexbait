//! Implements the state for the hexbait parser.

use std::{borrow::Cow, collections::BTreeMap, path::PathBuf};

use hexbait_builtin_parsers::built_in_format_descriptions;

/// The type of parser to use.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum ParseType {
    /// Use no parser.
    None,
    /// Use a built-in parser.
    Builtin(&'static str),
    /// Use a custom parser.
    Custom(PathBuf),
}

impl ParseType {
    /// Returns a string representation of the parse type.
    pub fn as_str(&self) -> Cow<'_, str> {
        match self {
            ParseType::None => Cow::Borrowed("none"),
            ParseType::Builtin(name) => Cow::Borrowed(name),
            ParseType::Custom(path_buf) => {
                if let Some(file_name) = path_buf.file_name() {
                    match file_name.to_string_lossy() {
                        Cow::Borrowed(name) => {
                            Cow::Borrowed(name.strip_suffix(".hbl").unwrap_or(name))
                        }
                        Cow::Owned(name) => {
                            if let Some(name) = name.strip_suffix(".hbl") {
                                Cow::Owned(String::from(name))
                            } else {
                                Cow::Owned(name)
                            }
                        }
                    }
                } else {
                    path_buf.to_string_lossy()
                }
            }
        }
    }
}

/// The state of the hexbait parser.
pub struct ParseState {
    /// The name of the type that should be parsed.
    pub parse_type: ParseType,
    /// The offset at which to parse.
    pub parse_offset: String,
    /// Whether the parse offset should be synced to the start of the selection.
    pub sync_parse_offset_to_selection_start: bool,
    /// The built-in format description.
    pub built_in_format_descriptions: BTreeMap<&'static str, hexbait_lang::ir::File>,
    /// The path to the custom parser definitions.
    pub custom_parsers: Vec<PathBuf>,
}

impl ParseState {
    /// Creates a new parse state.
    pub fn new(custom_parsers: Vec<PathBuf>) -> ParseState {
        ParseState {
            parse_type: ParseType::None,
            parse_offset: String::from("0"),
            sync_parse_offset_to_selection_start: true,
            built_in_format_descriptions: built_in_format_descriptions(),
            custom_parsers,
        }
    }
}
