//! Implements common string based operations.

use std::borrow::Cow;

/// Converts the given string literal content to bytes.
///
/// This function does not expect any surrounding `"` bytes.
pub fn str_lit_content_to_bytes(
    content: &str,
    out: &mut Vec<u8>,
) -> Result<(), (Cow<'static, str>, usize)> {
    /// The different states that can be encountered during the parsing of string
    /// literals.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum State {
        /// No special conditions apply to the current state.
        Normal,
        /// The previous character was an escape character.
        Escaped,
        /// Expecting the first hex digit for a hex escape.
        Hex1,
        /// Expecting the second hex digit for a hex escape.
        Hex2,
    }

    use State::*;
    let mut state = Normal;
    let mut hex_state = 0;
    for (i, c) in content.char_indices() {
        state = match (c, state) {
            ('\\', Normal) => Escaped,
            (_, Normal) => {
                out.extend_from_slice(c.encode_utf8(&mut [0; 4]).as_bytes());
                Normal
            }
            ('0', Escaped) => {
                out.push(b'\0');
                Normal
            }
            ('n', Escaped) => {
                out.push(b'\n');
                Normal
            }
            ('r', Escaped) => {
                out.push(b'\r');
                Normal
            }
            ('t', Escaped) => {
                out.push(b'\t');
                Normal
            }
            ('\\', Escaped) => {
                out.push(b'\\');
                Normal
            }
            ('"', Escaped) => {
                out.push(b'"');
                Normal
            }
            ('x', Escaped) => Hex1,
            (_, Escaped) => return Err((Cow::Owned(format!("unknown escape sequence: {c}")), i)),
            (_, Hex1 | Hex2) => {
                let Some(val) = c.to_digit(16) else {
                    return Err((Cow::Borrowed("expected two hex characters after `\\x`"), i));
                };
                let val = u8::try_from(val).expect("a single hex digit cannot exceed a u8");

                match state {
                    Hex1 => {
                        hex_state = val;
                        Hex2
                    }
                    Hex2 => {
                        out.push(hex_state << 4 | val);
                        Normal
                    }
                    _ => unreachable!(),
                }
            }
        };
    }

    if state == Normal {
        Ok(())
    } else {
        Err((
            Cow::Borrowed("unfinished escape sequence at the end of the string"),
            content.len(),
        ))
    }
}
