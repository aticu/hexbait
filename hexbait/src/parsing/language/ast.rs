//! Implements the AST in the parser description language.

use std::fmt;

pub use expr::*;

mod expr;

/// A node to be parsed.
#[derive(Clone)]
pub struct Node {
    /// The kind of node.
    pub kind: NodeKind,
    /// Parse this at the given offset instead of after the next field.
    pub offset: Option<Expr>,
}

impl fmt::Debug for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(offset) = &self.offset {
            write!(f, "{:?} @ {:?}", self.kind, offset)
        } else {
            write!(f, "{:?}", self.kind)
        }
    }
}

/// The kind of a node that can be parsed.
#[derive(Clone)]
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
    /// Parses the given named node.
    NamedNode {
        /// The name of the node to parse.
        name: Symbol,
    },
    /// Parses the given node at a different location without updating the offset.
    Elsewhere {
        /// The node to parse.
        node: Box<Node>,
    },
    /// A composite node consisting of multiple named subnodes.
    Struct {
        /// The nodes that make up the struct.
        nodes: Vec<(Symbol, Node)>,
    },
    /// Repeats a node `count` times.
    RepeatCount {
        /// The node to parse.
        node: Box<Node>,
        /// The number of times to parse the node.
        count: Expr,
    },
    /// Repeats while the provided condition is true.
    RepeatWhile {
        /// The node to parse.
        node: Box<Node>,
        /// The condition that is checked.
        condition: Expr,
    },
    /// Parses different nodes depending on a condition.
    ParseIf {
        /// The condition that determines which node is parsed.
        condition: Expr,
        /// The node to parse if the condition is true.
        true_node: Box<Node>,
        /// The node to parse if the condition is false.
        false_node: Box<Node>,
    },
    /// Parses different values depending on a condition.
    Switch {
        /// The expression to evaluate for the switch.
        scrutinee: Expr,
        /// The branches of this switch with their expressions and the corresponding nodes.
        branches: Vec<(Expr, Node)>,
        /// The node to parse in case no branch matches.
        default: Box<Node>,
    },
}

impl fmt::Debug for NodeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FixedBytes { expected } => {
                write!(
                    f,
                    "bytes len {} = {:?}",
                    match &expected.kind {
                        ExprKind::ConstantBytes { value } => value.len(),
                        _ => unreachable!(),
                    },
                    expected
                )
            }
            Self::FixedLength { length } => {
                write!(f, "bytes len {:?}", length)
            }
            Self::Integer { bit_width, signed } => {
                write!(f, "{}{}", if *signed { "i" } else { "u" }, bit_width)
            }
            Self::Float { bit_width } => f
                .debug_struct("Float")
                .field("bit_width", bit_width)
                .finish(),
            Self::NamedNode { name } => f.debug_struct("NamedNode").field("name", name).finish(),
            Self::Elsewhere { node } => f.debug_struct("Elsewhere").field("node", node).finish(),
            Self::Struct { nodes } => {
                writeln!(f, "struct {{")?;
                for (name, node) in nodes {
                    writeln!(f, "    {name:?} {node:?};")?;
                }
                writeln!(f, "}}")
            }
            Self::RepeatCount { node, count } => f
                .debug_struct("RepeatCount")
                .field("node", node)
                .field("count", count)
                .finish(),
            Self::RepeatWhile { node, condition } => f
                .debug_struct("RepeatWhile")
                .field("node", node)
                .field("condition", condition)
                .finish(),
            Self::ParseIf {
                condition,
                true_node,
                false_node,
            } => f
                .debug_struct("ParseIf")
                .field("condition", condition)
                .field("true_node", true_node)
                .field("false_node", false_node)
                .finish(),
            Self::Switch {
                scrutinee,
                branches,
                default,
            } => f
                .debug_struct("Switch")
                .field("scrutinee", scrutinee)
                .field("branches", branches)
                .field("default", default)
                .finish(),
        }
    }
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
        write!(f, "{}", self.name)
    }
}
