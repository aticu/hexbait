//! A rudimentary interface for a standalone parser binary.
//!
//! This also serves as a testing ground for an eventual integration into hexbait itself.

use std::{char, path::PathBuf, str::FromStr};

use clap::Parser;
use hexbait_builtin_parsers::built_in_format_descriptions;
use hexbait_common::{Input, RelativeOffset};
use hexbait_lang::{Value, View, eval_ir, ir::lower_file, parse};
use serde_json::Number;

/// hexbait-parser - parses bytes to json according to .hbl-definitions
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Config {
    /// The file to parse, stdin if omitted
    file: Option<PathBuf>,
    /// Lists possible definitions
    #[arg(short, long)]
    list: bool,
    /// What to parse in the input
    #[arg(short, long)]
    parse_as: Option<String>,
    /// A custom parser to use
    #[arg(short, long)]
    custom: Option<PathBuf>,
}

/// The entry point for the application.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::parse();

    let mut builtin = built_in_format_descriptions();

    if config.list {
        for name in builtin.keys() {
            println!("{name}");
        }
        println!();
        println!(
            "if the `--custom` (or `-c`) argument is used, the format definition at the supplied path will be used instead"
        );
        std::process::exit(0);
    }

    let parser = match (config.custom, config.parse_as) {
        (Some(path), _) => {
            let content = std::fs::read_to_string(path)?;

            let parse = parse(&content);
            // TODO: handle errors better here
            assert!(parse.errors.is_empty());

            lower_file(parse.ast)
        }
        (None, Some(name)) => {
            if let Some(parser) = builtin.remove(&*name) {
                parser
            } else {
                eprintln!("unknown definition name: {name}, exiting...");
                std::process::exit(1);
            }
        }
        (None, None) => {
            eprintln!("no definition to parse as specified, exiting...");
            std::process::exit(1);
        }
    };

    let input = match config.file {
        Some(path) => Input::from_path(path)?,
        None => Input::from_stdin()?,
    };
    let view = View::from_input(input);

    let result = eval_ir(&parser, view, RelativeOffset::ZERO).value;
    let as_json = value_to_json(&result);

    println!("{}", as_json);

    Ok(())
}

/// Converts the given parsed value to JSON.
fn value_to_json(value: &Value) -> serde_json::Value {
    match &value.kind {
        hexbait_lang::ValueKind::Boolean(val) => serde_json::Value::Bool(*val),
        hexbait_lang::ValueKind::Integer(val) => {
            let num = if let Ok(num) = u128::try_from(val) {
                Number::from_u128(num)
            } else if let Ok(num) = i128::try_from(val) {
                Number::from_i128(num)
            } else {
                Number::from_str(&val.to_string()).ok()
            };
            num.map(serde_json::Value::Number)
                .unwrap_or(serde_json::Value::Null)
        }
        hexbait_lang::ValueKind::Float(val) => serde_json::Number::from_f64(*val)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        hexbait_lang::ValueKind::Bytes(val) => {
            let mut as_str = String::new();
            for byte in &val.as_vec().unwrap() {
                for bit in (0..8).step_by(4).rev() {
                    let nibble = (byte >> bit) & 0xf;
                    let c = char::from_digit(nibble as u32, 16).unwrap();
                    as_str.push(c);
                }
            }
            serde_json::Value::String(as_str)
        }
        hexbait_lang::ValueKind::Struct { fields, .. } => {
            let mut object = serde_json::Map::new();

            for (name, val) in fields {
                object.insert(name.as_str().to_string(), value_to_json(val));
            }

            serde_json::Value::Object(object)
        }
        hexbait_lang::ValueKind::Array { items, .. } => {
            serde_json::Value::Array(items.iter().map(value_to_json).collect())
        }
    }
}
