//! Implements the lexer.

use std::fmt;

use crate::span::Span;

/// Describes all kinds of possible tokens.
#[derive(
    logos::Logos,
    num_enum::TryFromPrimitive,
    Debug,
    PartialEq,
    Eq,
    Clone,
    Copy,
    PartialOrd,
    Ord,
    Hash,
)]
#[repr(u16)]
pub enum TokenKind {
    // Trivia
    /// A comment on a single line.
    #[regex("//[^\n]*\n?")]
    LineComment,
    /// A block comment.
    // Note that this only matches the start in logos. The rest of the parsing is implemented
    // manually.
    #[token("/*")]
    BlockComment,
    /// Arbitrary amounts of white space.
    #[regex("\\s+")]
    Whitespace,

    // Expression contents
    /// A binary integer literal.
    #[regex("0b[01][01]*")]
    BinaryIntegerLiteral,
    /// An octal integer literal.
    #[regex("0o[0-7][0-7]*")]
    OctalIntegerLiteral,
    /// A hexadecimal integer literal.
    #[regex("0x[0-9a-fA-F][0-9a-fA-F]*")]
    HexadecimalIntegerLiteral,
    /// A decimal integer literal.
    #[regex("[0-9][0-9]*", priority = 10)]
    DecimalIntegerLiteral,
    /// A byte literal.
    // This has a low priority so that valid decimal integer literals are parsed as such.
    // A consequence of this is that decimal integer literals may be valid byte literals.
    // This needs to be handled correctly by later stages.
    #[regex("[0-9a-fA-F]{2}", priority = 0)]
    ByteLiteral,
    /// A string literal.
    #[regex("\"(?:[^\\\\\"]|\\\\.)*\"")]
    StringLiteral,
    /// An identifier to name something.
    #[regex("[a-zA-Z_][a-zA-Z0-9_]*")]
    Identifier,

    // Symbols
    /// An exclamation mark: `!`.
    #[token("!")]
    ExclamationMark,
    /// The underscore symbol: `_`.
    #[token("_", priority = 10)]
    Underscore,
    /// The ampersand symbol: `&`.
    #[token("&")]
    Ampersand,
    /// The vertical line symbol: `|`.
    #[token("|")]
    VerticalLine,
    /// The plus symbol: `+`.
    #[token("+")]
    Plus,
    /// The minus symbol: `-`.
    #[token("-")]
    Minus,
    /// The star symbol: `*`.
    #[token("*")]
    Star,
    /// The slash symbol: `/`.
    #[token("/")]
    Slash,
    /// The percent symbol: `%`.
    #[token("%")]
    Percent,
    /// The equals symbol: `=`.
    #[token("=")]
    Equals,
    /// The caret symbol: `^`.
    #[token("^")]
    Caret,
    /// The colon symbol: `:`.
    #[token(":")]
    Colon,
    /// The semicolon symbol: `;`.
    #[token(";")]
    Semicolon,
    /// The comma symbol: `,`.
    #[token(",")]
    Comma,
    /// The dot symbol: `.`.
    #[token(".")]
    Dot,
    /// The dollar symbol: `$`.
    #[token("$")]
    Dollar,
    /// The hash symbol: `#`.
    #[token("#")]
    Hash,
    /// The left angle symbol: `<`.
    #[token("<")]
    LAngle,
    /// The right angle symbol: `>`.
    #[token(">")]
    RAngle,

    // Parentheses
    /// The left parenthesis: `(`.
    #[token("(")]
    LParen,
    /// The right parenthesis: `)`.
    #[token(")")]
    RParen,
    /// The left brace: `{`.
    #[token("{")]
    LBrace,
    /// The right brace: `}`.
    #[token("}")]
    RBrace,
    /// The left bracket: `[`.
    #[token("[")]
    LBracket,
    /// The right bracket: `]`.
    #[token("]")]
    RBracket,

    // Keywords
    /// The `bytes` keyword.
    #[token("bytes")]
    BytesKw,
    /// The `struct` keyword.
    #[token("struct")]
    StructKw,
    /// The `let` keyword.
    #[token("let")]
    LetKw,
    /// The `parse` keyword.
    #[token("peek")]
    PeekKw,
    /// The `switch` keyword.
    #[token("switch")]
    SwitchKw,
    /// The `true` keyword.
    #[token("true")]
    TrueKw,
    /// The `false` keyword.
    #[token("false")]
    FalseKw,

    /// Represents any kind of error in the input stream.
    Error,

    /// Guaranteed to be the last variant.
    _Last,
}

impl TokenKind {
    /// The human-readable name of this token kind.
    pub fn name(&self) -> &'static str {
        match self {
            TokenKind::LineComment => "line comment",
            TokenKind::BlockComment => "block comment",
            TokenKind::Whitespace => "whitespace",
            TokenKind::BinaryIntegerLiteral => "binary integer",
            TokenKind::OctalIntegerLiteral => "octal integer",
            TokenKind::HexadecimalIntegerLiteral => "hexadecimal integer",
            TokenKind::DecimalIntegerLiteral => "decimal integer",
            TokenKind::ByteLiteral => "byte literal",
            TokenKind::StringLiteral => "string literal",
            TokenKind::Identifier => "identifier",
            TokenKind::ExclamationMark => "!",
            TokenKind::Underscore => "`_`",
            TokenKind::Ampersand => "`&`",
            TokenKind::VerticalLine => "`|`",
            TokenKind::Plus => "`+`",
            TokenKind::Minus => "`-`",
            TokenKind::Star => "`*`",
            TokenKind::Slash => "`/`",
            TokenKind::Percent => "`%`",
            TokenKind::Equals => "`=`",
            TokenKind::Caret => "`^`",
            TokenKind::Colon => "`:`",
            TokenKind::Semicolon => "`;`",
            TokenKind::Comma => "`,`",
            TokenKind::Dot => "`.`",
            TokenKind::Dollar => "`$`",
            TokenKind::Hash => "`#`",
            TokenKind::LAngle => "`<`",
            TokenKind::RAngle => "`>`",
            TokenKind::LParen => "`(`",
            TokenKind::RParen => "`)`",
            TokenKind::LBrace => "`{`",
            TokenKind::RBrace => "`}`",
            TokenKind::LBracket => "`[`",
            TokenKind::RBracket => "`]`",
            TokenKind::BytesKw => "`bytes`",
            TokenKind::StructKw => "`struct`",
            TokenKind::LetKw => "`let`",
            TokenKind::PeekKw => "`peek`",
            TokenKind::SwitchKw => "`switch`",
            TokenKind::TrueKw => "`true`",
            TokenKind::FalseKw => "`false`",
            TokenKind::Error => "an unrecognized token",
            TokenKind::_Last => unreachable!("the last variant should never be constructed"),
        }
    }

    /// Returns `true` if the `TokenKind` is trivia.
    ///
    /// A token is trivia if it carries no semantic value other than as a possible separator for
    /// other tokens.
    pub fn is_trivia(&self) -> bool {
        match self {
            TokenKind::LineComment | TokenKind::BlockComment | TokenKind::Whitespace => true,
            TokenKind::BinaryIntegerLiteral
            | TokenKind::OctalIntegerLiteral
            | TokenKind::HexadecimalIntegerLiteral
            | TokenKind::DecimalIntegerLiteral
            | TokenKind::ByteLiteral
            | TokenKind::StringLiteral
            | TokenKind::Identifier
            | TokenKind::ExclamationMark
            | TokenKind::Underscore
            | TokenKind::Ampersand
            | TokenKind::VerticalLine
            | TokenKind::Plus
            | TokenKind::Minus
            | TokenKind::Star
            | TokenKind::Slash
            | TokenKind::Percent
            | TokenKind::Equals
            | TokenKind::Caret
            | TokenKind::Colon
            | TokenKind::Semicolon
            | TokenKind::Comma
            | TokenKind::Dot
            | TokenKind::Dollar
            | TokenKind::Hash
            | TokenKind::LAngle
            | TokenKind::RAngle
            | TokenKind::LParen
            | TokenKind::RParen
            | TokenKind::LBrace
            | TokenKind::RBrace
            | TokenKind::LBracket
            | TokenKind::RBracket
            | TokenKind::BytesKw
            | TokenKind::StructKw
            | TokenKind::LetKw
            | TokenKind::PeekKw
            | TokenKind::SwitchKw
            | TokenKind::TrueKw
            | TokenKind::FalseKw
            | TokenKind::Error
            | TokenKind::_Last => false,
        }
    }
}

impl fmt::Display for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Represents a single token produced by the lexer.
pub(crate) struct Token {
    /// The kind of the token.
    pub(crate) kind: TokenKind,
    /// The span of the token.
    pub(crate) span: Span,
}

impl fmt::Debug for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} @ {:?}", self.kind, self.span)
    }
}

/// Lexes the given input into tokens.
pub fn lex(input: &str) -> Vec<Token> {
    let mut lexer = logos::Lexer::new(input);
    let mut tokens = Vec::new();

    while let Some(kind) = lexer.next() {
        let kind = kind.unwrap_or(TokenKind::Error);
        let start = lexer.span().start;
        let end = lexer.span().end;

        match kind {
            TokenKind::BlockComment => {
                let mut level = 1;

                loop {
                    if lexer.remainder().starts_with("*/") {
                        level -= 1;
                        lexer.bump(2);
                        if level == 0 {
                            break;
                        }
                    }
                    match lexer.next() {
                        Some(Ok(TokenKind::BlockComment)) => level += 1,
                        Some(_) => (),
                        None => break,
                    }
                }
                let end = lexer.span().end;

                tokens.push(Token {
                    kind: TokenKind::BlockComment,
                    span: Span { start, end },
                });
            }
            _ => {
                tokens.push(Token {
                    kind,
                    span: Span { start, end },
                });
            }
        }
    }

    tokens
}
