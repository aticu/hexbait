//! Implements a test harness for the hexbait-lang parser.

use std::path::Path;

use hexbait_lang::compile::{ast::AstNode as _, parser::parse_file};

/// Goes through all parser test cases and snapshots their parse trees.
#[test]
fn cases() {
    insta::glob!("parser-cases/**.hbl", |path| {
        snapshot_file_parse_tree(path);
    });
}

/// Goes through all built-in foramat definitions and snapshots their parse trees.
#[test]
fn format_definitions() {
    insta::glob!("../..", "format_descriptions/**.hbl", |path| {
        snapshot_file_parse_tree(path);
    });
}

/// Records a snapshot for the parse tree of the given file.
fn snapshot_file_parse_tree(path: &Path) {
    let name = path.file_name().unwrap().to_str().unwrap();
    let content = std::fs::read_to_string(path).unwrap();

    let parse = parse_file(&content);

    let mut result = format!("{:#?}\n--- diagnostics ---\n", parse.ast.syntax());

    if parse.diagnostics.is_empty() {
        result.push_str("no diagnostics");
    } else {
        for diagnostic in parse.diagnostics {
            result.push_str(&diagnostic.emit_to_str(name, &content).unwrap());
            result.push('\n');
        }
    }

    insta::assert_snapshot!(name, result);
}
