//! Implements parsing of expressions.

use crate::{
    NodeKind,
    lexer::TokenKind,
    parser::infrastructure::{Completed, CompletedMarker, Parser},
};

use super::nested_parse_type;

/// Parses an atomic expression.
fn atom<'p, 'src>(p: &'p mut Parser<'src>) -> Completed<'p, 'src> {
    let m = p.start();

    let (node_kind, next) = match p.cur() {
        Some(
            kind @ (TokenKind::Identifier
            | TokenKind::BinaryIntegerLiteral
            | TokenKind::OctalIntegerLiteral
            | TokenKind::DecimalIntegerLiteral
            | TokenKind::HexadecimalIntegerLiteral
            | TokenKind::TrueKw
            | TokenKind::FalseKw
            | TokenKind::StringLiteral),
        ) => (NodeKind::Atom, kind),
        Some(TokenKind::Dollar) => {
            p.expect(TokenKind::Dollar);
            (NodeKind::Metavar, TokenKind::Identifier)
        }
        Some(TokenKind::PeekKw) => {
            p.expect(TokenKind::PeekKw);
            p.expect(TokenKind::LParen);

            nested_parse_type(p);

            if p.at_contextual_kw("at") {
                p.bump();
                expr(p);
            }

            (NodeKind::PeekExpr, TokenKind::RParen)
        }
        Some(TokenKind::LAngle) => {
            p.expect(TokenKind::LAngle);
            loop {
                match p.cur() {
                    Some(TokenKind::RAngle) => break,
                    Some(
                        lit @ (TokenKind::StringLiteral
                        | TokenKind::ByteLiteral // for things like 1a
                        | TokenKind::DecimalIntegerLiteral // for things like 10
                        | TokenKind::Identifier), // for things like a1
                    ) => p.expect(lit),
                    _ => {
                        p.dbg();
                        todo!("error")
                    }
                }
            }
            (NodeKind::ByteConcat, TokenKind::RAngle)
        }
        Some(TokenKind::LParen) => {
            p.expect(TokenKind::LParen);
            expr(p);
            (NodeKind::ParenExpr, TokenKind::RParen)
        }
        _ => {
            p.expect_error(vec![
                "identifier",
                "literal",
                "`parse`",
                "`$`",
                "`<`",
                "`(`",
            ]);
            let completed = p.complete(m, NodeKind::Atom);
            return p.completed_from_marker(completed);
        }
    };

    p.complete_after(m, node_kind, next)
}

/// Parses an expression.
pub(crate) fn expr<'p, 'src>(p: &'p mut Parser<'src>) -> Completed<'p, 'src> {
    let completed_marker = expr_bp(p, 0);

    // ensure that trivia is properly bumped before continuing
    p.completed_from_marker(completed_marker)
}

/// Parses an expression using a Pratt parser with the given minimum binding power.
fn expr_bp<'p, 'src>(p: &'p mut Parser<'src>, min_bp: u8) -> CompletedMarker {
    // parse prefix and first atom
    let mut lhs = if let Some(op) = PrefixOp::peek(p) {
        let m = p.start();

        let (_l_bp, r_bp) = op.binding_power();
        op.parse(p);

        expr_bp(p, r_bp);

        p.complete(m, NodeKind::PrefixExpr)
    } else {
        atom(p).handle_trivia_manually()
    };

    // postfix loop
    loop {
        let next_token = p.peek().map(|(_, kind)| kind).next();
        match next_token {
            Some(TokenKind::Dot) => {
                let m = lhs.precede(p);

                p.expect(TokenKind::Dot);

                lhs = p
                    .complete_after(m, NodeKind::FieldAccess, TokenKind::Identifier)
                    .handle_trivia_manually();
            }
            _ => break,
        };
    }

    // infix loop
    loop {
        let Some(op) = InfixOp::peek(p) else {
            // no infix operator upcoming -> no more expression to parse
            break;
        };

        let (l_bp, r_bp) = op.binding_power();
        if l_bp < min_bp {
            break;
        }

        // we know we will parse an infix expression, so bump trivia before that
        p.trivia_bumper().bump();

        op.parse(p);

        let _rhs = expr_bp(p, r_bp);

        // finally wrap everything into it's own node
        let m = lhs.precede(p);
        lhs = p.complete(m, NodeKind::InfixExpr);
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
        match p.peek().next() {
            Some((_, TokenKind::Minus)) => Some(PrefixOp::Neg),
            Some((_, TokenKind::Plus)) => Some(PrefixOp::Plus),
            Some((_, TokenKind::ExclamationMark)) => Some(PrefixOp::Not),
            _ => None,
        }
    }

    /// Parses this operator.
    fn parse(self, p: &mut Parser) {
        let m = p.start();
        let final_token = match self {
            PrefixOp::Neg => TokenKind::Minus,
            PrefixOp::Plus => TokenKind::Plus,
            PrefixOp::Not => TokenKind::ExclamationMark,
        };

        p.complete_after(m, NodeKind::Op, final_token);
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

        match (peek.next(), peek.next()) {
            // two character operators
            (Some((i1, TokenKind::Equals)), Some((i2, TokenKind::Equals))) if i1 + 1 == i2 => {
                Some(InfixOp::Eq)
            }
            (Some((i1, TokenKind::ExclamationMark)), Some((i2, TokenKind::Equals)))
                if i1 + 1 == i2 =>
            {
                Some(InfixOp::Neq)
            }
            (Some((i1, TokenKind::RAngle)), Some((i2, TokenKind::Equals))) if i1 + 1 == i2 => {
                Some(InfixOp::Geq)
            }
            (Some((i1, TokenKind::LAngle)), Some((i2, TokenKind::Equals))) if i1 + 1 == i2 => {
                Some(InfixOp::Leq)
            }
            (Some((i1, TokenKind::Ampersand)), Some((i2, TokenKind::Ampersand)))
                if i1 + 1 == i2 =>
            {
                Some(InfixOp::LogicalAnd)
            }
            (Some((i1, TokenKind::VerticalLine)), Some((i2, TokenKind::VerticalLine)))
                if i1 + 1 == i2 =>
            {
                Some(InfixOp::LogicalOr)
            }
            (Some((i1, TokenKind::LAngle)), Some((i2, TokenKind::LAngle))) if i1 + 1 == i2 => {
                Some(InfixOp::ShiftLeft)
            }
            (Some((i1, TokenKind::RAngle)), Some((i2, TokenKind::RAngle))) if i1 + 1 == i2 => {
                Some(InfixOp::ShiftRight)
            }

            // single character operators
            (Some((_, TokenKind::Plus)), _) => Some(InfixOp::Add),
            (Some((_, TokenKind::Minus)), _) => Some(InfixOp::Sub),
            (Some((_, TokenKind::Star)), _) => Some(InfixOp::Mul),
            (Some((_, TokenKind::Slash)), _) => Some(InfixOp::Div),
            (Some((_, TokenKind::RAngle)), _) => Some(InfixOp::Gt),
            (Some((_, TokenKind::LAngle)), _) => Some(InfixOp::Lt),
            (Some((_, TokenKind::Ampersand)), _) => Some(InfixOp::BitAnd),
            (Some((_, TokenKind::VerticalLine)), _) => Some(InfixOp::BitOr),
            (Some((_, TokenKind::Caret)), _) => Some(InfixOp::BitXor),

            _ => None,
        }
    }

    /// Parses this operator.
    fn parse(self, p: &mut Parser) {
        let m = p.start();
        let final_token = match self {
            InfixOp::Add => TokenKind::Plus,
            InfixOp::Sub => TokenKind::Minus,
            InfixOp::Mul => TokenKind::Star,
            InfixOp::Div => TokenKind::Slash,
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

        p.complete_after(m, NodeKind::Op, final_token);
    }

    /// Returns the binding powers of this operator.
    fn binding_power(&self) -> (u8, u8) {
        match self {
            InfixOp::Add | InfixOp::Sub => (15, 16),
            InfixOp::Mul | InfixOp::Div => (17, 18),
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
