use std::fmt;

use crate::lexer::TokenKind;

/// A syntax element.
///
/// This may be either a single token or a group of tokens.
#[derive(PartialEq, Eq, Clone, Copy, PartialOrd, Ord, Hash)]
pub enum SyntaxKind {
    /// The syntax element is a token.
    Token {
        /// The kind of the token.
        kind: TokenKind,
    },
    /// The syntax element is a node.
    Node {
        /// The kind of the node.
        kind: NodeKind,
    },
}

impl fmt::Debug for SyntaxKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Token { kind } => kind.fmt(f),
            Self::Node { kind } => kind.fmt(f),
        }
    }
}

impl From<TokenKind> for SyntaxKind {
    fn from(value: TokenKind) -> Self {
        SyntaxKind::Token { kind: value }
    }
}

impl From<NodeKind> for SyntaxKind {
    fn from(value: NodeKind) -> Self {
        SyntaxKind::Node { kind: value }
    }
}

impl SyntaxKind {
    /// Expects the `SyntaxKind` to be a `Token`, panicking if it isn't.
    #[track_caller]
    pub fn expect_token(self) -> TokenKind {
        match self {
            SyntaxKind::Token { kind } => kind,
            SyntaxKind::Node { .. } => {
                panic!("expected syntax kind to be a token, but it was a node")
            }
        }
    }

    /// Expects the `SyntaxKind` to be a `Node`, panicking if it isn't.
    #[track_caller]
    pub fn expect_node(self) -> NodeKind {
        match self {
            SyntaxKind::Token { .. } => {
                panic!("expected syntax kind to be a node, but it was a token")
            }
            SyntaxKind::Node { kind } => kind,
        }
    }
}

/// Ensures that there is enough room for all `SyntaxKind`s.
const _: () = {
    assert!(
        (TokenKind::_Last as u16)
            .checked_add(NodeKind::_Last as u16)
            .is_some()
    );
};

/// The kind of an inner node in the syntax tree.
#[derive(Debug, PartialEq, Eq, Clone, Copy, PartialOrd, Ord, Hash)]
#[repr(u16)]
pub enum NodeKind {
    /// A file.
    ///
    /// This is the root node of a parse tree.
    File,

    // Definitions
    /// Defines a named struct.
    Struct,
    /// A field of a struct.
    StructField,

    // Parse types
    /// A parse type that is a single identifier.
    NamedParseType,
    /// A parse type that parses bytes until a condition is met.
    BytesParseType,
    /// A parse type that is a repetition of a fixed element.
    RepeatParseType,
    /// A parse type that parses an anonymous `struct`.
    AnonymousStructParseType,

    // Repeating types
    /// A repetition of a fixed number of elements.
    RepeatLenDecl,
    /// A repetition until a condition is met.
    RepeatWhileDecl,

    // Declarations
    /// A declaration of endianness like `!endian le`.
    EndiannessDeclaration,
    /// A declaration to align the parsing offset like `!align 4`.
    AlignDeclaration,
    /// A declaration to seek to a specified offset like `!seek to 64`.
    SeekToDeclaration,
    /// A declaration to seek by a specified amount like `!seek by +64`.
    SeekByDeclaration,
    /// A declaration that parsing should continue in another scope that starts at a given offset.
    ScopeAtDeclaration,

    // Expressions
    /// An atomic expression.
    Atom,
    /// A byte concatenation expression.
    ByteConcat,
    /// An operator used in expressions.
    Op,
    /// A parenthesized expression.
    ParenExpr,
    /// A binary expression with an infix operator.
    InfixExpr,
    /// An expression with a prefix operator.
    PrefixExpr,

    /// Contains unrecognized syntax.
    Error,

    /// Guaranteed to be the last variant.
    _Last,
}

/// The type that `rowan::Language` is implemented for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Language {}

impl rowan::Language for Language {
    type Kind = SyntaxKind;

    fn kind_from_raw(raw: rowan::SyntaxKind) -> Self::Kind {
        let num = raw.0;

        if num < TokenKind::_Last as u16 {
            SyntaxKind::Token {
                kind: unsafe {
                    // SAFETY: `TokenKind` is repr(u16) and the variant is before `_Last`, so it's a
                    // valid variant
                    std::mem::transmute::<u16, TokenKind>(num)
                },
            }
        } else {
            let num = num - TokenKind::_Last as u16;
            assert!(num < NodeKind::_Last as u16);
            SyntaxKind::Node {
                kind: unsafe {
                    // SAFETY: `NodeKind` is repr(u16) and the variant is before `_Last`, so it's a
                    // valid variant
                    std::mem::transmute::<u16, NodeKind>(num)
                },
            }
        }
    }

    fn kind_to_raw(kind: Self::Kind) -> rowan::SyntaxKind {
        let num = match kind {
            SyntaxKind::Token { kind } => kind as u16,
            SyntaxKind::Node { kind } => kind as u16 + TokenKind::_Last as u16,
        };

        rowan::SyntaxKind(num)
    }
}

/// A node in a concrete syntax tree based on `rowan`.
pub type SyntaxNode = rowan::SyntaxNode<Language>;

/// A token in a concrete syntax tree based on `rowan`.
pub type SyntaxToken = rowan::SyntaxToken<Language>;
