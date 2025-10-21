//! Implements the parser for the hexbait language.

use crate::{
    ast::{AstNode as _, File},
    lexer::lex,
    span::Span,
    syntax::SyntaxKind,
};
use infrastructure::{Event, Parser};
use rowan::GreenNodeBuilder;

mod implementation;
mod infrastructure;

/// The result of parsing.
#[derive(Debug)]
pub struct Parse {
    /// The parsed [`File`].
    pub ast: File,
    /// The errors that occurred during parsing.
    pub errors: Vec<ParseError>,
}

/// A single parsing error.
#[derive(Debug, Clone)]
pub struct ParseError {
    /// The error message.
    pub message: String,
    /// The [`Span`] at which parsing failed.
    pub span: Span,
    /// The tokens that were expected instead of the found token.
    pub expected: Vec<&'static str>,
}

/// Parses the given text.
pub fn parse(src: &str) -> Parse {
    let tokens = lex(src);
    let mut p = Parser::new(src, &tokens);
    implementation::root(&mut p);

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
                    &src[t.span.start..t.span.end],
                );
            }
            Event::Finish => builder.finish_node(),
            Event::Error(_) => (),
        }
    }
    let green = builder.finish();
    let errors = p
        .events()
        .iter()
        .filter_map(|e| match e {
            Event::Error(e) => Some(e.clone()),
            _ => None,
        })
        .collect();

    let syntax_node = rowan::SyntaxNode::<crate::syntax::Language>::new_root(green);
    Parse {
        ast: File::cast(syntax_node).expect("root node is always `File`"),
        errors,
    }
}
