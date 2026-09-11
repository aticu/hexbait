//! Implements parsing of expressions.

use crate::compile::{
    lexer::TokenKind,
    parser::infrastructure::{CompletedMarker, Parser},
    syntax::NodeKind,
};

use super::nested_parse_type;

/// Parses an atomic expression.
fn atom(p: &mut Parser) -> CompletedMarker {
    p.node(|p| {
        match p.cur() {
            Some(
                TokenKind::Identifier
                | TokenKind::BinaryIntegerLiteral
                | TokenKind::OctalIntegerLiteral
                | TokenKind::DecimalIntegerLiteral
                | TokenKind::HexadecimalIntegerLiteral
                | TokenKind::TrueKw
                | TokenKind::FalseKw
                | TokenKind::StringLiteral,
            ) => {
                p.bump();
                NodeKind::Atom
            }
            Some(TokenKind::Dollar) => {
                p.expect(TokenKind::Dollar);
                p.expect(TokenKind::Identifier);
                NodeKind::Metavar
            }
            Some(TokenKind::PeekKw) => {
                p.expect(TokenKind::PeekKw);
                p.expect(TokenKind::LParen);
                p.with_consuming_recovery(TokenKind::RParen, |p| {
                    nested_parse_type(p);

                    if p.at_contextual_kw("at") {
                        p.bump();
                        expr(p);
                    }
                });

                NodeKind::PeekExpr
            }
            Some(TokenKind::ConcatKw) => {
                p.expect(TokenKind::ConcatKw);
                p.expect(TokenKind::LParen);
                p.with_consuming_recovery(TokenKind::RParen, |p| {
                    let mut needs_comma = false;
                    loop {
                        if needs_comma {
                            match p.cur() {
                                Some(TokenKind::Comma) => {
                                    p.expect(TokenKind::Comma);
                                }
                                Some(TokenKind::RParen) => break,
                                _ => {
                                    p.expect_error(&["`,`", "`)`"]);
                                    break;
                                }
                            }
                        }

                        match p.cur() {
                            Some(TokenKind::RParen) => break,
                            Some(TokenKind::Dot) => {
                                p.node(|p| {
                                    p.expect(TokenKind::Dot);
                                    p.expect(TokenKind::Dot);

                                    expr(p);
                                    NodeKind::ConcatArgExpanding
                                });
                            }
                            _ => {
                                p.node(|p| {
                                    expr(p);
                                    NodeKind::ConcatArgDirect
                                });
                            }
                        }

                        needs_comma = true;
                    }
                });

                NodeKind::ConcatExpr
            }
            Some(TokenKind::LAngle) => {
                p.expect(TokenKind::LAngle);
                p.with_consuming_recovery(TokenKind::RAngle, |p| {
                    loop {
                        match p.cur() {
                            Some(TokenKind::RAngle) => break,
                            Some(
                                TokenKind::StringLiteral
                                | TokenKind::ByteLiteral // for things like 1a
                                | TokenKind::DecimalIntegerLiteral // for things like 10
                                | TokenKind::Identifier // for things like a1
                            ) => p.bump(),
                            _ => {
                                p.expect_error(&["a two character hex-literal like `00`, `0a` or `a0`"]);
                                break;
                            }
                        }
                    }
                });
                NodeKind::ByteConcat
            }
            Some(TokenKind::LParen) => {
                p.expect(TokenKind::LParen);
                p.with_consuming_recovery(TokenKind::RParen, |p| {
                    expr(p);
                });
                NodeKind::ParenExpr
            }
            _ => {
                p.expect_error(&[
                    "identifier",
                    "literal",
                    "`peek`",
                    "`concat`",
                    "`$`",
                    "`<`",
                    "`(`",
                ]);
                // this is deliberately still an atom, so that `Error` does not need be an expression kind in grammar.ungram
                NodeKind::Atom
            }
        }
    })
}

/// Parses an expression.
pub(crate) fn expr(p: &mut Parser) {
    expr_bp(p, 0);
}

/// Parses an expression using a Pratt parser with the given minimum binding power.
fn expr_bp(p: &mut Parser, min_bp: u8) -> CompletedMarker {
    // parse prefix and first atom
    let mut lhs = if let Some(op) = PrefixOp::peek(p) {
        p.node(|p| {
            let (_l_bp, r_bp) = op.binding_power();
            op.parse(p);

            expr_bp(p, r_bp);

            NodeKind::PrefixExpr
        })
    } else {
        atom(p)
    };

    // postfix loop
    loop {
        let next_token = p.peek().map(|t| t.kind).next();
        match next_token {
            Some(TokenKind::Dot) => {
                lhs = p.precede_with(lhs, |p| {
                    p.expect(TokenKind::Dot);
                    p.expect(TokenKind::Identifier);
                    NodeKind::FieldAccess
                });
            }
            _ => break,
        };
    }

    // infix loop
    while let Some(op) = InfixOp::peek(p) {
        let (l_bp, r_bp) = op.binding_power();
        if l_bp < min_bp {
            break;
        }

        lhs = p.precede_with(lhs, |p| {
            op.parse(p);
            expr_bp(p, r_bp);
            NodeKind::InfixExpr
        });
    }

    lhs
}

/// A prefix operator.
#[derive(Debug)]
enum PrefixOp {
    /// `-`
    Neg,
    /// `+`
    ///
    /// A no-op
    Plus,
    /// `!`
    Not,
}

impl PrefixOp {
    /// Returns an upcoming prefix operator, if it is present.
    fn peek(p: &Parser) -> Option<PrefixOp> {
        match p.cur() {
            Some(TokenKind::Minus) => Some(PrefixOp::Neg),
            Some(TokenKind::Plus) => Some(PrefixOp::Plus),
            Some(TokenKind::ExclamationMark) => Some(PrefixOp::Not),
            _ => None,
        }
    }

    /// Parses this operator.
    fn parse(self, p: &mut Parser) {
        p.node(|p| {
            let final_token = match self {
                PrefixOp::Neg => TokenKind::Minus,
                PrefixOp::Plus => TokenKind::Plus,
                PrefixOp::Not => TokenKind::ExclamationMark,
            };
            p.expect(final_token);

            NodeKind::Op
        });
    }

    /// Returns the binding powers of this operator.
    fn binding_power(&self) -> ((), u8) {
        match self {
            PrefixOp::Neg | PrefixOp::Plus | PrefixOp::Not => ((), 19),
        }
    }
}

/// An infix operator.
#[derive(Debug)]
enum InfixOp {
    /// `+`
    Add,
    /// `-`
    Sub,
    /// `*`
    Mul,
    /// `/`
    Div,
    /// `%`
    Mod,
    /// `==`
    Eq,
    /// `!=`
    Neq,
    /// `>`
    Gt,
    /// `>=`
    Geq,
    /// `<`
    Lt,
    /// `<=`
    Leq,
    /// `&&`
    LogicalAnd,
    /// `||`
    LogicalOr,
    /// `&`
    BitAnd,
    /// `|`
    BitOr,
    /// `^`
    BitXor,
    /// `<<`
    ShiftLeft,
    /// `>>`
    ShiftRight,
}

impl InfixOp {
    /// Returns an upcoming infix operator, if it is present.
    fn peek(p: &Parser) -> Option<InfixOp> {
        let mut peek = p.peek();

        match (
            peek.next().map(|t| t.kind),
            peek.next().map(|t| (t.kind, t.preceeded_by_trivia)),
        ) {
            // two character operators
            (Some(TokenKind::Equals), Some((TokenKind::Equals, false))) => Some(InfixOp::Eq),
            (Some(TokenKind::ExclamationMark), Some((TokenKind::Equals, false))) => {
                Some(InfixOp::Neq)
            }
            (Some(TokenKind::RAngle), Some((TokenKind::Equals, false))) => Some(InfixOp::Geq),
            (Some(TokenKind::LAngle), Some((TokenKind::Equals, false))) => Some(InfixOp::Leq),
            (Some(TokenKind::Ampersand), Some((TokenKind::Ampersand, false))) => {
                Some(InfixOp::LogicalAnd)
            }
            (Some(TokenKind::VerticalLine), Some((TokenKind::VerticalLine, false))) => {
                Some(InfixOp::LogicalOr)
            }
            (Some(TokenKind::LAngle), Some((TokenKind::LAngle, false))) => Some(InfixOp::ShiftLeft),
            (Some(TokenKind::RAngle), Some((TokenKind::RAngle, false))) => {
                Some(InfixOp::ShiftRight)
            }

            // single character operators
            (Some(TokenKind::Plus), _) => Some(InfixOp::Add),
            (Some(TokenKind::Minus), _) => Some(InfixOp::Sub),
            (Some(TokenKind::Star), _) => Some(InfixOp::Mul),
            (Some(TokenKind::Slash), _) => Some(InfixOp::Div),
            (Some(TokenKind::Percent), _) => Some(InfixOp::Mod),
            (Some(TokenKind::RAngle), _) => Some(InfixOp::Gt),
            (Some(TokenKind::LAngle), _) => Some(InfixOp::Lt),
            (Some(TokenKind::Ampersand), _) => Some(InfixOp::BitAnd),
            (Some(TokenKind::VerticalLine), _) => Some(InfixOp::BitOr),
            (Some(TokenKind::Caret), _) => Some(InfixOp::BitXor),

            _ => None,
        }
    }

    /// Parses this operator.
    fn parse(self, p: &mut Parser) {
        p.node(|p| {
            let final_token = match self {
                InfixOp::Add => TokenKind::Plus,
                InfixOp::Sub => TokenKind::Minus,
                InfixOp::Mul => TokenKind::Star,
                InfixOp::Div => TokenKind::Slash,
                InfixOp::Mod => TokenKind::Percent,
                InfixOp::Eq => {
                    p.expect(TokenKind::Equals);
                    TokenKind::Equals
                }
                InfixOp::Neq => {
                    p.expect(TokenKind::ExclamationMark);
                    TokenKind::Equals
                }
                InfixOp::Gt => TokenKind::RAngle,
                InfixOp::Geq => {
                    p.expect(TokenKind::RAngle);
                    TokenKind::Equals
                }
                InfixOp::Lt => TokenKind::LAngle,
                InfixOp::Leq => {
                    p.expect(TokenKind::LAngle);
                    TokenKind::Equals
                }
                InfixOp::LogicalAnd => {
                    p.expect(TokenKind::Ampersand);
                    TokenKind::Ampersand
                }
                InfixOp::LogicalOr => {
                    p.expect(TokenKind::VerticalLine);
                    TokenKind::VerticalLine
                }
                InfixOp::BitAnd => TokenKind::Ampersand,
                InfixOp::BitOr => TokenKind::VerticalLine,
                InfixOp::BitXor => TokenKind::Caret,
                InfixOp::ShiftLeft => {
                    p.expect(TokenKind::LAngle);
                    TokenKind::LAngle
                }
                InfixOp::ShiftRight => {
                    p.expect(TokenKind::RAngle);
                    TokenKind::RAngle
                }
            };
            p.expect(final_token);

            NodeKind::Op
        });
    }

    /// Returns the binding powers of this operator.
    fn binding_power(&self) -> (u8, u8) {
        match self {
            InfixOp::Add | InfixOp::Sub => (15, 16),
            InfixOp::Mul | InfixOp::Div | InfixOp::Mod => (17, 18),
            InfixOp::Eq
            | InfixOp::Neq
            | InfixOp::Gt
            | InfixOp::Geq
            | InfixOp::Lt
            | InfixOp::Leq => (5, 6),
            InfixOp::LogicalOr => (1, 2),
            InfixOp::LogicalAnd => (3, 4),
            InfixOp::BitOr => (7, 8),
            InfixOp::BitXor => (9, 10),
            InfixOp::BitAnd => (11, 12),
            InfixOp::ShiftLeft | InfixOp::ShiftRight => (13, 14),
        }
    }
}
