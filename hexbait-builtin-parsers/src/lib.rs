//! Provides the built-in format descriptions.

use std::collections::BTreeMap;

use hexbait_lang::compile::{CompileResult, compile_file, ir::File};

include!(concat!(env!("OUT_DIR"), "/built_in.gen.rs"));

/// Returns the built-in format definitions.
pub fn built_in_format_descriptions() -> BTreeMap<&'static str, File> {
    BUILT_IN_DEFINITIONS_RAW
        .iter()
        .map(|&(name, content)| {
            let name = name.strip_suffix(".hbl").unwrap_or(name);

            let ir = match compile_file(name, content) {
                CompileResult::NoDiagnostics { ir } => ir,
                CompileResult::WithWarnings { ir: _, diagnostics }
                | CompileResult::Failure { diagnostics } => {
                    diagnostics.emit_to_stderr();
                    panic!("built-in format description contained diagnostics");
                }
            };

            (name, ir)
        })
        .collect()
}
