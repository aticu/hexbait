//! Implements a test harness for the hexbait-lang parser.

use hexbait_lang::compile::{ast::AstNode as _, parser::parse_file};

/// Goes through all test cases.
#[test]
fn cases() {
    insta::glob!("parser-cases/**.hbl", |path| {
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
    });
}
