//! Implements parsing of expressions.

use crate::{
    NodeKind,
    lexer::TokenKind,
    parser::infrastructure::{CompletedMarker, Parser, TriviaBumper},
};

/// Parses an atomic expression.
fn atom<'p, 'src>(p: &'p mut Parser<'src>) -> (CompletedMarker, TriviaBumper<'p, 'src>) {
    let m = p.start();

    let (node_kind, next) = match p.cur() {
        Some(
            kind @ (TokenKind::Identifier
            | TokenKind::BinaryIntegerLiteral
            | TokenKind::OctalIntegerLiteral
            | TokenKind::DecimalIntegerLiteral
            | TokenKind::HexadecimalIntegerLiteral
            | TokenKind::StringLiteral),
        ) => (NodeKind::Atom, kind),
        Some(TokenKind::LParen) => {
            p.expect(TokenKind::LParen);
            expr(p);
            (NodeKind::ParenExpr, TokenKind::RParen)
        }
        _ => {
            p.expect_error(vec!["ident", "literal", "`(`"]);
            let completed = p.complete(m, NodeKind::Atom);
            return (completed, p.trivia_bumper());
        }
    };

    p.complete_after(m, node_kind, next)
}

/// Parses an expression.
pub(crate) fn expr<'p, 'src>(p: &'p mut Parser<'src>) -> TriviaBumper<'p, 'src> {
    expr_bp(p, 0);

    // ensure that trivia is properly bumped before continuing
    p.trivia_bumper()
}

/// Parses an expression using a Pratt parser with the given minimum binding power.
fn expr_bp<'p, 'src>(p: &'p mut Parser<'src>, min_bp: u8) -> CompletedMarker {
    let mut lhs = if matches!(p.peek().next(), Some((_, TokenKind::LAngle))) {
        let m = p.start();

        p.expect(TokenKind::LAngle);

        loop {
            match p.cur() {
                Some(TokenKind::RAngle) => break,
                Some(
                    lit @ (TokenKind::StringLiteral
                    | TokenKind::ByteLiteral
                    | TokenKind::DecimalIntegerLiteral),
                ) => p.expect(lit),
                _ => todo!("error"),
            }
        }

        let (lhs, trivia_bumper) = p.complete_after(m, NodeKind::ByteConcat, TokenKind::RAngle);

        // we will handle trivia bumping manually
        std::mem::forget(trivia_bumper);

        lhs
    } else if let Some(op) = PrefixOp::peek(p) {
        let m = p.start();

        let (_l_bp, r_bp) = op.binding_power();
        op.parse(p);

        expr_bp(p, r_bp);

        p.complete(m, NodeKind::PrefixExpr)
    } else {
        let (lhs, trivia_bumper) = atom(p);

        // we will handle trivia bumping manually
        std::mem::forget(trivia_bumper);

        lhs
    };

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
            PrefixOp::Neg | PrefixOp::Plus | PrefixOp::Not => ((), 7),
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

            // single character operators
            (Some((_, TokenKind::Plus)), _) => Some(InfixOp::Add),
            (Some((_, TokenKind::Minus)), _) => Some(InfixOp::Sub),
            (Some((_, TokenKind::Star)), _) => Some(InfixOp::Mul),
            (Some((_, TokenKind::Slash)), _) => Some(InfixOp::Div),
            (Some((_, TokenKind::RAngle)), _) => Some(InfixOp::Gt),
            (Some((_, TokenKind::LAngle)), _) => Some(InfixOp::Lt),

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
        };

        p.complete_after(m, NodeKind::Op, final_token);
    }

    /// Returns the binding powers of this operator.
    fn binding_power(&self) -> (u8, u8) {
        match self {
            InfixOp::Add | InfixOp::Sub => (3, 4),
            InfixOp::Mul | InfixOp::Div => (5, 6),
            InfixOp::Eq
            | InfixOp::Neq
            | InfixOp::Gt
            | InfixOp::Geq
            | InfixOp::Lt
            | InfixOp::Leq => (1, 2),
        }
    }
}
