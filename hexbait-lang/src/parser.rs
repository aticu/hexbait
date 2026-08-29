//! Implements the parser for the hexbait language.

use crate::{
    ast::{AstNode, Expr, File},
    lexer::lex,
    syntax::SyntaxKind,
};
use infrastructure::{Event, Parser};
use rowan::GreenNodeBuilder;

mod diagnostics;
mod implementation;
mod infrastructure;

pub use diagnostics::Diagnostic;

/// The result of parsing.
#[derive(Debug)]
pub struct Parse<'src, Result> {
    /// The name of the source.
    pub source_name: &'src str,
    /// The source that was parsed from.
    pub source: &'src str,
    /// The parsed result.
    pub ast: Result,
    /// The diagnostics that occurred during parsing.
    pub diagnostics: Vec<Diagnostic>,
}

impl<Result> Parse<'_, Result> {
    /// Emits the diagnostics to stderr.
    pub fn emit_diagnostics_to_stderr(&self) {
        for diagnostic in &self.diagnostics {
            diagnostic
                .emit_to_stderr(self.source_name, self.source)
                .unwrap();
        }
    }
}

/// Parses the given file content.
pub fn parse_file<'src>(source_name: &'src str, source: &'src str) -> Parse<'src, File> {
    parse(source_name, source, implementation::root)
}

/// Parses the given expression.
pub fn parse_expr<'src>(source_name: &'src str, source: &'src str) -> Parse<'src, Expr> {
    parse(source_name, source, |p| {
        implementation::expr(p);
    })
}

/// Parses the given text.
fn parse<'src, Result: AstNode>(
    source_name: &'src str,
    source: &'src str,
    parse_fn: impl FnOnce(&mut Parser),
) -> Parse<'src, Result> {
    let tokens = lex(source);
    let mut p = Parser::new(source, &tokens);
    parse_fn(&mut p);

    let mut builder = GreenNodeBuilder::new();
    let mut tok_idx = 0;
    for ev in p.events() {
        match ev {
            Event::Start {
                kind,
                forward_parent,
                is_forward_parent,
            } => {
                if *is_forward_parent {
                    // forward parents where already handled by their children
                    continue;
                }

                let mut forward_parent = forward_parent;
                let mut parents = Vec::new();
                while let Some(parent_idx) = forward_parent
                    && let Event::Start {
                        kind,
                        forward_parent: new_forward_parent,
                        is_forward_parent: true,
                    } = &p.events()[*parent_idx]
                {
                    parents.push(kind);
                    forward_parent = new_forward_parent;
                }

                // reverse parents so the last preceding node is started first
                for parent_kind in parents.iter().rev() {
                    let kind = parent_kind.expect("nodes should always be finished in the parser");
                    builder.start_node(<crate::syntax::Language as rowan::Language>::kind_to_raw(
                        SyntaxKind::from(kind),
                    ))
                }

                let kind = kind.expect("nodes should always be finished in the parser");
                builder.start_node(<crate::syntax::Language as rowan::Language>::kind_to_raw(
                    SyntaxKind::from(kind),
                ))
            }
            Event::Token => {
                let t = &tokens[tok_idx];
                tok_idx += 1;
                builder.token(
                    <crate::syntax::Language as rowan::Language>::kind_to_raw(SyntaxKind::from(
                        t.kind,
                    )),
                    &source[t.span.start..t.span.end],
                );
            }
            Event::Finish => builder.finish_node(),
            Event::Error(_) => (),
        }
    }
    let green = builder.finish();
    let diagnostics = p
        .events()
        .iter()
        .filter_map(|e| match e {
            Event::Error(e) => Some(e.clone()),
            _ => None,
        })
        .collect();

    let syntax_node = rowan::SyntaxNode::<crate::syntax::Language>::new_root(green);
    Parse {
        source_name,
        source,
        ast: Result::cast(syntax_node).expect("root node is always `File`"),
        diagnostics,
    }
}
