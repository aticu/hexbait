//! Implements the primitives that are used in the parser implementation.

use crate::compile::{
    diagnostics::Label,
    lexer::{Token, TokenKind},
    parser::Diagnostic,
    span::Span,
    syntax::NodeKind,
};

/// A marker for a started node.
struct Marker {
    /// The index into the event stream where the node was started.
    idx: usize,
}

/// A marker for a completed node.
///
/// Can be used to precede earlier started nodes.
pub(crate) struct CompletedMarker {
    /// The index into the event stream where the node was started.
    idx: usize,
}

impl CompletedMarker {
    /// Make a new parent that will wrap this node.
    fn precede(self, p: &mut Parser) -> Marker {
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
    Error(Diagnostic),
    /// Finishes the previously started node.
    Finish,
}

/// A peeked token.
pub(crate) struct PeekedToken<'src> {
    /// The kind of the token.
    pub(crate) kind: TokenKind,
    /// The source text of the token.
    pub(crate) text: &'src str,
    /// Whether this token was preceeded by trivia.
    pub(crate) preceeded_by_trivia: bool,
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
        let mut parser = Parser {
            src,
            tokens,
            pos: 0,
            events: Vec::with_capacity(tokens.len() * 2),
        };

        // skip initial trivia
        while let Some(t) = parser.cur()
            && t.is_trivia()
        {
            parser.pos += 1;
        }

        parser
    }

    /// Peeks all upcoming non-trivia tokens.
    pub(crate) fn peek(&self) -> impl Iterator<Item = PeekedToken<'src>> {
        (0..self.tokens.len())
            .skip(self.pos)
            .filter(|&i| !self.tokens[i].kind.is_trivia())
            .map(|i| {
                let preceeded_by_trivia = i
                    .checked_sub(1)
                    .map(|prev_idx| self.tokens[prev_idx].kind.is_trivia())
                    .unwrap_or(false);
                let token = &self.tokens[i];

                PeekedToken {
                    kind: token.kind,
                    text: &self.src[token.span.start..token.span.end],
                    preceeded_by_trivia,
                }
            })
    }

    /// The current token kind.
    pub(crate) fn cur(&self) -> Option<TokenKind> {
        self.tokens.get(self.pos).map(|t| t.kind)
    }

    /// Checks if the parser is currently at the given token.
    pub(crate) fn at(&self, kind: TokenKind) -> bool {
        self.cur() == Some(kind)
    }

    /// Checks if the parser is currently at the given contextual keyword.
    pub(crate) fn at_contextual_kw(&self, kw: &str) -> bool {
        self.at(TokenKind::Identifier) && self.peek().next().map(|t| t.text) == Some(kw)
    }

    /// Expects a contextual keyword, returning the text of the keyword.
    #[track_caller]
    pub(crate) fn expect_peek_contextual_kw(&mut self) -> Option<&str> {
        if self.at(TokenKind::Identifier) {
            let span = self.tokens[self.pos].span;

            Some(&self.src[span.start..span.end])
        } else {
            self.expect_error(&["contextual keyword"]);

            None
        }
    }

    /// Expects a contextual keyword, returning the text of the keyword.
    #[track_caller]
    pub(crate) fn expect_and_bump_contextual_kw(&mut self) -> Option<&str> {
        if self.at(TokenKind::Identifier) {
            let span = self.tokens[self.pos].span;

            self.bump();

            Some(&self.src[span.start..span.end])
        } else {
            self.expect_error(&["contextual keyword"]);

            None
        }
    }

    /// Expects the given token next.
    pub(crate) fn expect(&mut self, kind: TokenKind) {
        if self.cur() == Some(kind) {
            self.bump();
        } else {
            self.expect_error(&[kind.name()]);
        }
    }

    /// Bumps the parser forward to the next non-trivia token.
    pub(crate) fn bump(&mut self) {
        self.events.push(Event::Token);
        self.pos += 1;

        while let Some(t) = self.cur()
            && t.is_trivia()
        {
            self.pos += 1;
        }
    }

    /// Parses a node.
    pub(crate) fn node(
        &mut self,
        parse_node: impl FnOnce(&mut Self) -> NodeKind,
    ) -> CompletedMarker {
        let idx = self.events.len();
        self.events.push(Event::Start {
            kind: None,
            forward_parent: None,
            is_forward_parent: false,
        });
        let m = Marker { idx };

        let kind = parse_node(self);

        self.complete(m, kind)
    }

    /// Wraps the given completed node in a new parent node.
    pub(crate) fn precede_with(
        &mut self,
        completed_node: CompletedMarker,
        parse_parent: impl FnOnce(&mut Self) -> NodeKind,
    ) -> CompletedMarker {
        let m = completed_node.precede(self);

        let kind = parse_parent(self);

        self.complete(m, kind)
    }

    /// Completes the given node.
    fn complete(&mut self, m: Marker, kind: NodeKind) -> CompletedMarker {
        match &mut self.events[m.idx] {
            Event::Start {
                kind: kind_to_set, ..
            } => *kind_to_set = Some(kind),
            _ => unreachable!("markers should only point at started nodes"),
        }
        self.events.push(Event::Finish);

        CompletedMarker { idx: m.idx }
    }

    /// Creates an error.
    pub(crate) fn expect_error(&mut self, expected: &[&'static str]) {
        let span = self.tokens.get(self.pos).map(|t| t.span).unwrap_or(Span {
            start: self.src.len(),
            end: self.src.len(),
        });

        let message = match expected.len() {
            0 => unreachable!(),
            1 => format!("expected {}", expected[0]),
            _ => format!(
                "expected {} or {}",
                expected[..expected.len() - 1].join(", "),
                expected.last().unwrap()
            ),
        };

        self.events.push(Event::Error(Diagnostic::error(
            message,
            Label::new(
                format!(
                    "found unexpected token {}",
                    self.tokens
                        .get(self.pos)
                        .map(|token| format!("{}", token.kind))
                        .unwrap_or_else(|| String::from("end of file"))
                ),
                span,
            ),
        )));

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
