//! Implements the compiler-side of the language.

pub mod ast;
mod diagnostics;
pub mod ir;
mod lexer;
pub mod parser;
mod span;
mod syntax;

pub use diagnostics::{Diagnostic, DiagnosticEmitter, Diagnostics, RgbColor, Style};
pub use span::Span;

use crate::compile::{
    ir::{check_file, check_repl_statement, lower_file, lower_repl_statement},
    parser::{parse_expr, parse_file},
};

/// The result of a successful compilation.
pub enum CompileResult<Ir> {
    /// The compilation succeeded without diagnostics.
    NoDiagnostics {
        /// The resulting IR.
        ir: Ir,
    },
    /// The compilation succeeded with warnings.
    WithWarnings {
        /// The resulting IR.
        ir: Ir,
        /// The diagnostics containing the warnings.
        diagnostics: Diagnostics,
    },
    ///  The compilations failed.
    Failure {
        /// The resulting diagnostics.
        diagnostics: Diagnostics,
    },
}

/// Compiles a file to IR.
pub fn compile_file(name: &str, content: &str) -> CompileResult<ir::File> {
    let mut diagnostics = Diagnostics::new(name, content);

    let parse = parse_file(content);
    diagnostics.add_diagnostics(parse.diagnostics);
    let ir = lower_file(parse.ast, &mut diagnostics);
    check_file(&ir, &mut diagnostics);

    if diagnostics.contains_errors() {
        CompileResult::Failure { diagnostics }
    } else if diagnostics.is_empty() {
        CompileResult::NoDiagnostics { ir }
    } else {
        CompileResult::WithWarnings { ir, diagnostics }
    }
}

/// Compiles a REPL statement to IR.
pub fn compile_repl_statement(name: &str, content: &str) -> CompileResult<ir::ReplStatement> {
    let mut diagnostics = Diagnostics::new(name, content);

    let parse = parse_expr(content);
    diagnostics.add_diagnostics(parse.diagnostics);
    let ir = lower_repl_statement(parse.ast, &mut diagnostics);
    check_repl_statement(&ir, &mut diagnostics);

    if diagnostics.contains_errors() {
        CompileResult::Failure { diagnostics }
    } else if diagnostics.is_empty() {
        CompileResult::NoDiagnostics { ir }
    } else {
        CompileResult::WithWarnings { ir, diagnostics }
    }
}
