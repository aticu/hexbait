//! Provides the built-in format descriptions.

use std::collections::BTreeMap;

use hexbait_lang::{
    check_ir,
    ir::{File, lower_file},
    parse,
};

include!(concat!(env!("OUT_DIR"), "/built_in.gen.rs"));

/// Returns the built-in format definitions.
pub fn built_in_format_descriptions() -> BTreeMap<&'static str, File> {
    BUILT_IN_DEFINITIONS_RAW
        .iter()
        .map(|&(name, content)| {
            let name = name.strip_suffix(".hbl").unwrap_or(name);

            let parse = parse(content);
            // TODO: handle errors better here
            assert!(parse.errors.is_empty());
            let ir = lower_file(parse.ast);
            // TODO: use these
            let _resolved_names = check_ir(&ir).unwrap();

            (name, ir)
        })
        .collect()
}
