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
    ir::{check_ir, lower_expr, lower_file},
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

    if diagnostics.contains_errors() {
        return CompileResult::Failure { diagnostics };
    }

    let ir = lower_file(parse.ast);
    // TODO: use these
    let _resolved_names = check_ir(&ir).unwrap();

    if diagnostics.is_empty() {
        CompileResult::NoDiagnostics { ir }
    } else {
        CompileResult::WithWarnings { ir, diagnostics }
    }
}

/// Compiles an expression to IR.
pub fn compile_expr(name: &str, content: &str) -> CompileResult<ir::Expr> {
    let mut diagnostics = Diagnostics::new(name, content);

    let parse = parse_expr(content);
    diagnostics.add_diagnostics(parse.diagnostics);

    if diagnostics.contains_errors() {
        return CompileResult::Failure { diagnostics };
    }

    let ir = lower_expr(parse.ast);

    if diagnostics.is_empty() {
        CompileResult::NoDiagnostics { ir }
    } else {
        CompileResult::WithWarnings { ir, diagnostics }
    }
}
