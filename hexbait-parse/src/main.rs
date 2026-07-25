//! A rudimentary interface for a standalone parser binary.
//!
//! This also serves as a testing ground for an eventual integration into hexbait itself.

use std::path::PathBuf;

use clap::Parser;
use hexbait_builtin_parsers::built_in_format_descriptions;
use hexbait_common::{Input, RelativeOffset};
use hexbait_lang::{View, eval_ir, ir::lower_file, parse};
use hexbait_parse::result_to_json;

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
    /// Whether to show more detailed output
    #[arg(short, long)]
    detailed: bool,
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

    let result = eval_ir(&parser, view, RelativeOffset::ZERO);
    let as_json = result_to_json(&result, config.detailed);

    println!("{as_json}");

    Ok(())
}
