//! Implements the parser for the hexbait language.

use crate::compile::{
    Diagnostic,
    ast::{AstNode, Expr, File},
    lexer::{Token, lex},
    syntax::{NodeKind, SyntaxKind},
};
use infrastructure::{Event, Parser};
use rowan::GreenNodeBuilder;

mod implementation;
mod infrastructure;

/// The result of parsing.
#[derive(Debug)]
pub struct Parse<Result> {
    /// The parsed result.
    pub ast: Result,
    /// The diagnostics that occurred during parsing.
    pub diagnostics: Vec<Diagnostic>,
}

/// Parses the given file content.
pub fn parse_file(source: &str) -> Parse<File> {
    parse(source, implementation::root)
}

/// Parses the given expression.
pub fn parse_expr(source: &str) -> Parse<Expr> {
    parse(source, |p| {
        implementation::expr(p);
    })
}

/// Parses the given text.
fn parse<Result: AstNode>(source: &str, parse_fn: impl FnOnce(&mut Parser)) -> Parse<Result> {
    let tokens = lex(source);
    let mut p = Parser::new(source, &tokens);
    parse_fn(&mut p);

    let token = |t: &Token, builder: &mut GreenNodeBuilder| {
        builder.token(
            <crate::compile::syntax::Language as rowan::Language>::kind_to_raw(SyntaxKind::from(
                t.kind,
            )),
            &source[t.span.start..t.span.end],
        );
    };
    let start_node = |kind: Option<NodeKind>, builder: &mut GreenNodeBuilder| {
        let kind = kind.expect("nodes should always be finished in the parser");
        builder.start_node(
            <crate::compile::syntax::Language as rowan::Language>::kind_to_raw(SyntaxKind::from(
                kind,
            )),
        )
    };
    let flush_trivia = |tok_idx: &mut usize, builder: &mut GreenNodeBuilder| {
        while *tok_idx < tokens.len() && tokens[*tok_idx].kind.is_trivia() {
            let t = &tokens[*tok_idx];
            *tok_idx += 1;

            token(t, builder);
        }
    };

    let mut builder = GreenNodeBuilder::new();
    let mut tok_idx = 0;
    let mut depth = 0;
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

                // don't try to put trivia before the root node
                if depth > 0 {
                    flush_trivia(&mut tok_idx, &mut builder);
                }

                // reverse parents so the last preceding node is started first
                for parent_kind in parents.iter().rev() {
                    start_node(**parent_kind, &mut builder);
                    depth += 1;
                }

                start_node(*kind, &mut builder);
                depth += 1;
            }
            Event::Token => {
                flush_trivia(&mut tok_idx, &mut builder);

                let t = &tokens[tok_idx];
                tok_idx += 1;
                token(t, &mut builder);
            }
            Event::Finish => {
                depth -= 1;
                if depth == 0 {
                    // flush trailing trivia into the root
                    flush_trivia(&mut tok_idx, &mut builder);
                }
                builder.finish_node();
            }
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

    let syntax_node = rowan::SyntaxNode::<crate::compile::syntax::Language>::new_root(green);
    Parse {
        ast: Result::cast(syntax_node).expect("root node is always `File`"),
        diagnostics,
    }
}
