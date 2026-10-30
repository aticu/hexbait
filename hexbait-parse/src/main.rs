//! A rudimentary interface for a standalone parser binary.
//!
//! This also serves as a testing ground for an eventual integration into hexbait itself.

#![forbid(unsafe_code)]

use std::{
    io::{self, Write as _},
    path::PathBuf,
};

use clap::Parser;
use hexbait_builtin_parsers::built_in_format_descriptions;
use hexbait_common::{Input, RelativeOffset};
use hexbait_lang::{
    compile::{CompileResult, compile_file},
    eval::{DiagnosticLevel, View, eval_ir},
};
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
    /// Show more detailed output
    #[arg(short, long)]
    detailed: bool,
    /// Fail for parse errors
    #[arg(short, long)]
    fail_for_errors: bool,
}

/// The entry point for the application.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::parse();

    let mut builtin = built_in_format_descriptions();

    if config.list {
        let mut as_str = String::new();
        for name in builtin.keys() {
            as_str.push_str(name);
            as_str.push('\n');
        }
        as_str.push('\n');
        as_str.push_str(
            "if the `--custom` (or `-c`) argument is used, the format definition at the supplied path will be used instead"
        );

        return match writeln!(io::stdout().lock(), "{as_str}") {
            Err(e) if e.kind() == io::ErrorKind::BrokenPipe => Ok(()),
            result => Ok(result?),
        };
    }

    let parser = match (config.custom, config.parse_as) {
        (Some(path), _) => {
            let content = std::fs::read_to_string(&path)?;
            let source_name = path.display().to_string();

            match compile_file(&source_name, &content) {
                CompileResult::NoDiagnostics { ir } => ir,
                CompileResult::WithWarnings { ir, diagnostics } => {
                    diagnostics.emit_to_stderr();
                    ir
                }
                CompileResult::Failure { diagnostics } => {
                    diagnostics.emit_to_stderr();
                    std::process::exit(1);
                }
            }
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

    match writeln!(io::stdout().lock(), "{as_json}") {
        Err(e) if e.kind() == io::ErrorKind::BrokenPipe => (),
        result => result?,
    };

    if config.fail_for_errors
        && result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.level == DiagnosticLevel::Fail)
    {
        std::process::exit(1);
    }

    Ok(())
}
