//! Implements a test harness for the hexbait-lang static analysis.

use std::path::Path;

use hexbait_lang::compile::{CompileResult, ast::AstNode as _, compile_file, parser::parse_file};

/// Goes through all analysis test cases and snapshots their parse trees.
#[test]
fn cases() {
    insta::glob!("analysis-cases/**.hbl", |path| {
        snapshot_file_diagnostics(path);
    });
}

/// Records a snapshot of the diagnostics for the given file.
fn snapshot_file_diagnostics(path: &Path) {
    let name = path.file_name().unwrap().to_str().unwrap();
    let content = std::fs::read_to_string(path).unwrap();

    let parse = parse_file(&content);

    // use the diagnostics of later stages too
    let diagnostics = match compile_file(name, &content) {
        CompileResult::NoDiagnostics { ir: _ } => None,
        CompileResult::WithWarnings { ir: _, diagnostics }
        | CompileResult::Failure { diagnostics } => Some(diagnostics),
    };

    let mut result = String::new();
    if let Some(diagnostics) = diagnostics {
        for diagnostic in &diagnostics {
            result.push_str(&diagnostic.emit_to_str(name, &content).unwrap());
            result.push('\n');
        }
    } else {
        result.push_str("no diagnostics");
    }

    insta::assert_snapshot!(name, result, &content);

    assert_eq!(
        parse.ast.syntax().to_string(),
        content,
        "parsing should be lossless"
    );
}
