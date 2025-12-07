//! Implements the primitives that are used in the parser implementation.

use crate::{
    NodeKind,
    lexer::{Token, TokenKind},
    span::Span,
};

use super::ParseError;

/// A marker for a started node.
#[derive(Clone, Copy)]
pub(crate) struct Marker {
    /// The index into the event stream where the node was started.
    idx: usize,
}

/// A marker for a completed node.
///
/// Can be used to precede earlier started nodes.
#[derive(Clone, Copy)]
pub(crate) struct CompletedMarker {
    /// The index into the event stream where the node was started.
    idx: usize,
}

impl CompletedMarker {
    /// Make a new parent that will wrap this node.
    pub(crate) fn precede(self, p: &mut Parser) -> Marker {
        // Start a new parent node
        let parent_start = p.events.len();
        p.events.push(Event::Start {
            kind: None,
            forward_parent: None,
            is_forward_parent: true,
        });

        // Link the old node to the current parent
        match &mut p.events[self.idx] {
            Event::Start { forward_parent, .. } => *forward_parent = Some(parent_start),
            _ => unreachable!(),
        }

        Marker { idx: parent_start }
    }
}

/// A parser event.
///
/// Events are recorded in a first pass and later replayed to get a parse tree.
pub(crate) enum Event {
    /// Starts a node of the given kind.
    Start {
        /// The kind of node that was started.
        ///
        /// This is set when the node is completed.
        kind: Option<NodeKind>,
        /// The index of the parent node of this node.
        forward_parent: Option<usize>,
        /// Indicates whether the given node is a parent of some previous node.
        ///
        /// These nodes do not need to be started again when encountered during replay.
        is_forward_parent: bool,
    },
    /// Consumes a single token.
    Token,
    /// Records a parsing error.
    Error(ParseError),
    /// Finishes the previously started node.
    Finish,
}

/// Bumps trivia in the given parser if dropped.
///
/// Allows to complete parent nodes before bumping trivia.
pub(crate) struct TriviaBumper<'parser, 'src> {
    /// The parser in which trivia is to be dumped.
    parser: &'parser mut Parser<'src>,
}

impl TriviaBumper<'_, '_> {
    /// Bumps the trivia in the parser.
    pub(crate) fn bump(self) {}

    /// Handles trivia_bumping manually.
    pub(crate) fn handle_manually(self) {
        std::mem::forget(self)
    }
}

impl Drop for TriviaBumper<'_, '_> {
    fn drop(&mut self) {
        self.parser.bump_past_trivia();
    }
}

/// A completed node.
pub(crate) struct Completed<'parser, 'src> {
    /// The trivia bumper after the node.
    ///
    /// Allows to complete parent nodes before bumping trivia.
    trivia_bumper: TriviaBumper<'parser, 'src>,
    /// The completed marker of the finished node.
    completed_marker: CompletedMarker,
}

impl Completed<'_, '_> {
    /// Completes the given marker with the given node kind in the underlying parser.
    pub(crate) fn and_complete(self, m: Marker, kind: NodeKind) -> Self {
        let completed_marker = self.trivia_bumper.parser.complete(m, kind);

        Completed {
            completed_marker,
            ..self
        }
    }

    /// Returns the completed marker and handles trivia manually.
    pub(crate) fn handle_trivia_manually(self) -> CompletedMarker {
        self.trivia_bumper.handle_manually();
        self.completed_marker
    }
}

/// Contains the driving state for the parser.
pub(crate) struct Parser<'src> {
    /// The source that is being parsed.
    src: &'src str,
    /// The tokenized representation of the source.
    tokens: &'src [Token],
    /// The current offset into the token stream.
    pos: usize,
    /// The current parsing events that the parser already produced.
    events: Vec<Event>,
}

impl<'src> Parser<'src> {
    /// Creates a new parser for the given tokens.
    pub(crate) fn new(src: &'src str, tokens: &'src [Token]) -> Parser<'src> {
        Parser {
            src,
            tokens,
            pos: 0,
            events: Vec::with_capacity(tokens.len() * 2),
        }
    }

    /// Peeks all upcoming non-trivia tokens, returning their kind and whether they were preceded
    /// by trivia.
    ///
    /// Returns the token kind along with the index of the token.
    /// The current token is included.
    pub(crate) fn peek(&self) -> impl Iterator<Item = (usize, TokenKind)> {
        self.tokens
            .get(self.pos..)
            .unwrap_or(&[])
            .iter()
            .enumerate()
            .filter(|(_, t)| !t.kind.is_trivia())
            .map(|(i, t)| (self.pos + i, t.kind))
    }

    /// Returns the current token.
    pub(crate) fn cur(&self) -> Option<TokenKind> {
        self.tokens.get(self.pos).map(|t| t.kind)
    }

    /// Returns the text of the token at the given token index.
    pub(crate) fn text_at(&self, index: usize) -> Option<&str> {
        self.tokens
            .get(index)
            .map(|t| &self.src[t.span.start..t.span.end])
    }

    /// Returns the text of the current token.
    pub(crate) fn cur_text(&self) -> Option<&str> {
        self.text_at(self.pos)
    }

    /// Checks if the parser is currently at the given token.
    pub(crate) fn at(&self, kind: TokenKind) -> bool {
        self.cur() == Some(kind)
    }

    /// Checks if the parser is currently at the given contextual keyword.
    pub(crate) fn at_contextual_kw(&self, kw: &str) -> bool {
        self.at(TokenKind::Identifier) && self.cur_text() == Some(kw)
    }

    /// Expects a contextual keyword, returning the text of the keyword.
    pub(crate) fn expect_contextual_kw(&mut self) -> Option<&str> {
        if self.at(TokenKind::Identifier) {
            let span = self.tokens[self.pos].span;

            self.bump();

            Some(&self.src[span.start..span.end])
        } else {
            todo!("better error message here")
        }
    }

    /// Expects the given token next.
    pub(crate) fn expect(&mut self, kind: TokenKind) {
        if self.cur() == Some(kind) {
            self.bump();
        } else {
            self.expect_error(vec![kind.name()]);
        }
    }

    /// Bumps the parser forward to the next non-trivia token.
    pub(crate) fn bump(&mut self) {
        self.bump_raw();
        self.bump_past_trivia();
    }

    /// Bumps while the current token is a trivia token.
    fn bump_past_trivia(&mut self) {
        while let Some(t) = self.cur()
            && t.is_trivia()
        {
            self.bump_raw();
        }
    }

    /// Bumps the parser exactly one token forward.
    fn bump_raw(&mut self) {
        self.events.push(Event::Token);
        self.pos += 1;
    }

    /// Starts a new node.
    pub(crate) fn start(&mut self) -> Marker {
        let idx = self.events.len();
        self.events.push(Event::Start {
            kind: None,
            forward_parent: None,
            is_forward_parent: false,
        });
        Marker { idx }
    }

    /// Completes the given node.
    pub(crate) fn complete(&mut self, m: Marker, kind: NodeKind) -> CompletedMarker {
        match &mut self.events[m.idx] {
            Event::Start {
                kind: kind_to_set, ..
            } => *kind_to_set = Some(kind),
            _ => unreachable!("markers should only point at started nodes"),
        }
        self.events.push(Event::Finish);

        CompletedMarker { idx: m.idx }
    }

    /// Completes the given node after the given expected token.
    ///
    /// The trivia will need to be bumped manually afterwards, in order to
    pub(crate) fn complete_after<'this>(
        &'this mut self,
        m: Marker,
        kind: NodeKind,
        expected: TokenKind,
    ) -> Completed<'this, 'src> {
        if self.cur() == Some(expected) {
            self.bump_raw();
        } else {
            self.expect_error(vec![expected.name()]);
        }

        let completed_marker = self.complete(m, kind);
        self.completed_from_marker(completed_marker)
    }

    /// Returns a completed node from the given completed marker.
    pub(crate) fn completed_from_marker<'this>(
        &'this mut self,
        completed_marker: CompletedMarker,
    ) -> Completed<'this, 'src> {
        Completed {
            trivia_bumper: self.trivia_bumper(),
            completed_marker,
        }
    }

    /// Returns a trivia bumper for the current state.
    pub(crate) fn trivia_bumper<'this>(&'this mut self) -> TriviaBumper<'this, 'src> {
        TriviaBumper { parser: self }
    }

    /// Creates an error.
    pub(crate) fn expect_error(&mut self, expected: Vec<&'static str>) {
        let span = self.tokens.get(self.pos).map(|t| t.span).unwrap_or(Span {
            start: self.src.len(),
            end: self.src.len(),
        });

        self.events.push(Event::Error(ParseError {
            message: "expected something else".into(),
            span,
            expected,
        }));

        self.recover(&[TokenKind::Semicolon]);
    }

    /// Recovers from a previous error, by looking for one of the given sync tokens.
    fn recover(&mut self, sync: &[TokenKind]) {
        while let Some(token) = self.cur()
            && !sync.contains(&token)
        {
            self.bump();
        }
    }

    /// Returns a reference to the events of the parser.
    pub(crate) fn events(&self) -> &[Event] {
        &self.events
    }

    /// Print debug information about the current parser state.
    #[allow(unused)]
    pub(crate) fn dbg(&self) {
        eprintln!("DEBUG:");
        eprintln!("  token_position = {}", self.pos);
        eprintln!("  token = {:?}", self.cur());
        eprintln!(
            "  token_ctx = {:#?}",
            &self.tokens
                [self.pos.saturating_sub(2)..std::cmp::min(self.pos + 3, self.tokens.len() - 1)]
        );
        eprintln!(
            "  rest_text = {:?}",
            self.tokens
                .get(self.pos)
                .map(|t| &self.src[t.span.start..])
                .unwrap_or("")
        );
    }
}
