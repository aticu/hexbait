//! Provides the built-in format descriptions.

use std::collections::BTreeMap;

use hexbait_lang::{
    check_ir,
    ir::{File, lower_file},
    parse_file,
};

include!(concat!(env!("OUT_DIR"), "/built_in.gen.rs"));

/// Returns the built-in format definitions.
pub fn built_in_format_descriptions() -> BTreeMap<&'static str, File> {
    BUILT_IN_DEFINITIONS_RAW
        .iter()
        .map(|&(name, content)| {
            let name = name.strip_suffix(".hbl").unwrap_or(name);

            let parse = parse_file(name, content);
            parse.emit_diagnostics_to_stderr();
            if !parse.diagnostics.is_empty() {
                std::process::exit(1);
            }
            let ir = lower_file(parse.ast);
            // TODO: use these
            let _resolved_names = check_ir(&ir).unwrap();

            (name, ir)
        })
        .collect()
}
