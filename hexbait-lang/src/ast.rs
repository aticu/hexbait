//! Implements the abstract syntax tree.

use crate::{
    lexer::TokenKind::{self, *},
    span::Span,
    syntax::{NodeKind, SyntaxKind, SyntaxNode, SyntaxToken},
};

include!(concat!(env!("OUT_DIR"), "/ast.gen.rs"));

/// Defines operations common to all AST nodes.
pub trait AstNode: Sized {
    /// Casts the given syntax node into the AST node.
    fn cast(n: SyntaxNode) -> Option<Self>;

    /// Returns the underlying syntax node.
    fn syntax(&self) -> &SyntaxNode;

    /// Returns the span of the AST node.
    fn span(&self) -> Span {
        Span::from(self.syntax().text_range())
    }

    /// Returns all token children.
    fn tokens(&self) -> impl Iterator<Item = SyntaxToken> {
        self.syntax()
            .children_with_tokens()
            .filter_map(|it| it.into_token())
    }

    /// Returns the underlying text of the node.
    fn text(&self) -> rowan::SyntaxText {
        self.syntax().text()
    }
}

/// Returns an iterator over all children of the given type from the syntax node.
fn children<'node, N: AstNode + 'node>(n: &'node SyntaxNode) -> impl Iterator<Item = N> + 'node {
    n.children().filter_map(N::cast)
}

/// Returns the first token of the given kind.
fn tokens(n: &SyntaxNode, k: TokenKind) -> impl Iterator<Item = SyntaxToken> {
    n.children_with_tokens()
        .filter_map(|it| it.into_token())
        .filter(move |t| t.kind() == k.into())
}
