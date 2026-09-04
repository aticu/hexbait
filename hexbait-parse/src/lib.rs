//! The library to the CLI frontend of the hexbait language.

use std::str::FromStr as _;

use hexbait_lang::eval::{DiagnosticLevel, ParseResult, Provenance, StructContent, ValueKind};
use serde_json::{Map, Number, Value};

/// Converts the given parse result to JSON.
pub fn result_to_json(result: &ParseResult, detailed: bool) -> Value {
    let as_json = value_to_json(&result.value, result, detailed);

    if detailed {
        let mut object = Map::new();

        object.insert("value".to_string(), as_json);
        object.insert(
            "errors".to_string(),
            Value::Array(Vec::from_iter(
                result
                    .diagnostics
                    .iter()
                    .filter(|diagnostic| diagnostic.level == DiagnosticLevel::Fail)
                    .map(|err| {
                        let mut err_object = Map::new();

                        err_object.insert(
                            "message".to_string(),
                            Value::String(err.message.to_string()),
                        );
                        err_object.insert(
                            "provenance".to_string(),
                            Value::String(format_provenance(&err.provenance)),
                        );

                        Value::Object(err_object)
                    }),
            )),
        );
        object.insert(
            "warnings".to_string(),
            Value::Array(Vec::from_iter(
                result
                    .diagnostics
                    .iter()
                    .filter(|diagnostic| diagnostic.level == DiagnosticLevel::Warn)
                    .map(|warning| {
                        let mut warning_object = Map::new();

                        warning_object.insert(
                            "message".to_string(),
                            Value::String(warning.message.to_string()),
                        );
                        warning_object.insert(
                            "provenance".to_string(),
                            Value::String(format_provenance(&warning.provenance)),
                        );

                        Value::Object(warning_object)
                    }),
            )),
        );

        Value::Object(object)
    } else {
        as_json
    }
}

/// Converts the given parsed value to JSON.
fn value_to_json(value: &hexbait_lang::eval::Value, result: &ParseResult, detailed: bool) -> Value {
    let mut err = None;

    let val = match &value.kind {
        ValueKind::Boolean(val) => Value::Bool(*val),
        ValueKind::Integer(val) => {
            let num = if let Ok(num) = u128::try_from(val) {
                Number::from_u128(num)
            } else if let Ok(num) = i128::try_from(val) {
                Number::from_i128(num)
            } else {
                Number::from_str(&val.to_string()).ok()
            };
            num.map(Value::Number).unwrap_or(Value::Null)
        }
        ValueKind::Float(val) => Number::from_f64(*val)
            .map(Value::Number)
            .unwrap_or_else(|| Value::String(val.to_string())),
        ValueKind::Bytes(val) => {
            let mut as_str = String::new();
            for byte in &*val.value().unwrap() {
                for bit in (0..8).step_by(4).rev() {
                    let nibble = (byte >> bit) & 0xf;
                    let c = char::from_digit(nibble as u32, 16).unwrap();
                    as_str.push(c);
                }
            }
            Value::String(as_str)
        }
        ValueKind::Struct { content } => {
            let mut object = Map::new();
            for content in content {
                match content {
                    StructContent::Field { name, value } => {
                        object.insert(
                            name.as_str().to_string(),
                            value_to_json(value, result, detailed),
                        );
                    }
                    StructContent::Diagnostic(diagnostic_id) => {
                        let diagnostic = &result.diagnostics[diagnostic_id.raw_idx()];
                        let level = match diagnostic.level {
                            DiagnosticLevel::Fail => "failure",
                            DiagnosticLevel::Warn => "warning",
                        };
                        object.insert(
                            format!("_{level}{}", diagnostic_id.raw_idx()),
                            Value::String(diagnostic.message.to_string()),
                        );
                    }
                }
            }

            Value::Object(object)
        }
        ValueKind::Array { items, error } => {
            err = error.map(|err| err.raw_idx());

            Value::Array(
                items
                    .iter()
                    .map(|val| value_to_json(val, result, detailed))
                    .collect(),
            )
        }
    };

    if detailed {
        let mut object = Map::new();

        object.insert("value".to_string(), val);
        object.insert(
            "provenance".to_string(),
            Value::String(format_provenance(&value.provenance)),
        );
        if let Some(err) = err {
            object.insert(
                "error".to_string(),
                Value::String(result.diagnostics[err].message.to_string()),
            );
        }

        Value::Object(object)
    } else {
        val
    }
}

/// Formats a provenance as a string.
fn format_provenance(provenance: &Provenance) -> String {
    use std::fmt::Write as _;

    let mut out = String::new();

    let mut needs_sep = false;
    for range in provenance.byte_ranges() {
        if needs_sep {
            write!(&mut out, ", ").unwrap();
        }
        write!(&mut out, "{}..={}", range.start(), range.end()).unwrap();
        needs_sep = true;
    }

    out
}
