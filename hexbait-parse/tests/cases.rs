//! Implements a test harness for the parser output.

use std::ffi::OsStr;

use hexbait_common::{Input, RelativeOffset};
use hexbait_lang::{View, eval_ir, ir::lower_file};
use hexbait_parse::result_to_json;

/// Goes through all test cases.
#[test]
fn cases() {
    insta::glob!("cases/**/spec.hbl", |spec_path| {
        let dir = spec_path.parent().unwrap();
        let spec = std::fs::read_to_string(spec_path).unwrap();

        let mut tests_run = 0;
        for entry in std::fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();

            if path.extension() != Some(OsStr::new("hex")) {
                continue;
            }

            let name = path.file_name().unwrap().to_string_lossy().to_string();
            let hex_text = std::fs::read_to_string(&path).unwrap();
            let input =
                parse_hex(&hex_text).unwrap_or_else(|err| panic!("{}: {err}", path.display()));
            let debug_info = format!("\n=== Spec ===\n{spec}\n\n=== Hex ===\n{hex_text}");
            insta::assert_snapshot!(name.clone(), render(&name, &spec, &input), &debug_info);

            tests_run += 1;
        }

        if tests_run == 0 {
            panic!("no tests found in {}", dir.display());
        }
    });
}

/// Renders the parsed JSON for later diffing.
fn render(name: &str, spec: &str, input: &[u8]) -> String {
    let parse = hexbait_lang::parse_file(name, spec);
    parse.emit_diagnostics_to_stderr();
    if !parse.diagnostics.is_empty() {
        panic!("test case did not compile correctly");
    }

    let ir = lower_file(parse.ast);
    let view = View::from_input(Input::from_bytes(input));
    let result = eval_ir(&ir, view, RelativeOffset::ZERO);

    serde_json::to_string_pretty(&result_to_json(&result, true)).unwrap()
}

/// Parses a textual hex fixture into bytes.
///
/// The format is whitespace-separated runs of hex digits, each of which must
/// contain an even number of digits. `#` begins a comment that runs to the end
/// of the line.
fn parse_hex(source: &str) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();

    for (line_idx, line) in source.lines().enumerate() {
        let line_no = line_idx + 1;
        let code = line.split_once('#').map_or(line, |(before, _)| before);

        for token in code.split_whitespace() {
            // `token` is a subslice of `line`, so the difference gives its column.
            let col = token.as_ptr() as usize - line.as_ptr() as usize + 1;

            if !token.len().is_multiple_of(2) {
                return Err(format!(
                    "{line_no}:{col}: `{token}` has {} hex digits, expected an even number",
                    token.len()
                ));
            }

            for (i, pair) in token.as_bytes().as_chunks::<2>().0.iter().enumerate() {
                let hi = nibble(pair[0]).ok_or_else(|| bad_digit(line_no, col + 2 * i, pair[0]))?;
                let lo =
                    nibble(pair[1]).ok_or_else(|| bad_digit(line_no, col + 2 * i + 1, pair[1]))?;
                bytes.push((hi << 4) | lo);
            }
        }
    }

    Ok(bytes)
}

/// Converts a single hex digit to its value.
fn nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

/// Builds the error message for a byte that is not a hex digit.
fn bad_digit(line: usize, col: usize, byte: u8) -> String {
    format!("{line}:{col}: `{}` is not a hex digit", byte.escape_ascii())
}
