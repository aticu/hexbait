//! Implements the AST in the parser description language.

use std::fmt;

pub use expr::*;

mod expr;

/// A node to be parsed.
pub struct Node {
    /// The name of the node.
    pub name: Symbol,
    /// The kind of node.
    pub kind: NodeKind,
    /// Parse this at the given offset instead of after the next field.
    pub offset: Option<Expr>,
}

/// The kind of a node that can be parsed.
pub enum NodeKind {
    /// Fixed bytes are expected.
    FixedBytes {
        /// The bytes to expect.
        expected: Expr,
    },
    /// Bytes of a fixed length are parsed.
    FixedLength {
        /// The number of bytes to be parsed.
        length: Expr,
    },
    /// Parses an integer with a given bit width from the input.
    Integer {
        /// The bit width to use.
        bit_width: u32,
        /// Whether the integer is signed.
        signed: bool,
    },
    /// Parses a float with a given bit width from the input.
    Float {
        /// The bit width to use.
        bit_width: u32,
    },
    /// A composite node consisting of multiple named subnodes.
    Struct {
        /// The nodes that make up the struct.
        nodes: Vec<Node>,
    },
    /// Repeats a node `count` times.
    RepeatCount {
        /// The node to parse.
        node: Box<Node>,
        /// The number of times to parse the node.
        count: Expr,
    },
}

/// References a name in the language.
#[derive(PartialEq, Eq, Hash, Clone)]
pub struct Symbol {
    /// The name being referenced.
    name: String,
}

impl From<&str> for Symbol {
    fn from(value: &str) -> Self {
        Symbol {
            name: String::from(value),
        }
    }
}

impl fmt::Debug for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.name.fmt(f)
    }
}
