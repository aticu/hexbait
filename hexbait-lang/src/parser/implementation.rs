//! Contains the actual syntax descriptions.

use expressions::expr;

use crate::{NodeKind, lexer::TokenKind};

use super::infrastructure::{Completed, Parser};

mod expressions;

/// Parses the root node of the grammar.
pub(crate) fn root(p: &mut Parser) {
    let m = p.start();

    // bump all initial trivia
    p.trivia_bumper().bump();

    while p.cur().is_some() {
        struct_content(p);
    }

    p.complete(m, NodeKind::File);
}

/// Parses the content of a struct.
fn struct_content<'p, 'src>(p: &'p mut Parser<'src>) -> Completed<'p, 'src> {
    let Some(kind) = p.cur() else {
        todo!("error here")
    };

    match kind {
        TokenKind::StructKw => r#struct(p),
        TokenKind::LetKw => r#let(p),
        TokenKind::ExclamationMark => decl(p),
        _ => struct_field(p),
    }
}

/// Parses a `struct`.
fn r#struct<'p, 'src>(p: &'p mut Parser<'src>) -> Completed<'p, 'src> {
    let m = p.start();

    p.expect(TokenKind::StructKw);
    p.expect(TokenKind::Identifier);
    p.expect(TokenKind::LBrace);

    while p.cur().is_some_and(|t| t != TokenKind::RBrace) {
        struct_content(p);
    }

    p.complete_after(m, NodeKind::Struct, TokenKind::RBrace)
}

/// Parses a `let` statement.
fn r#let<'p, 'src>(p: &'p mut Parser<'src>) -> Completed<'p, 'src> {
    let m = p.start();

    p.expect(TokenKind::LetKw);
    p.expect(TokenKind::Identifier);
    p.expect(TokenKind::Equals);

    expr(p);

    p.complete_after(m, NodeKind::LetStatement, TokenKind::Semicolon)
}

/// Parses a declaration.
fn decl<'p, 'src>(p: &'p mut Parser<'src>) -> Completed<'p, 'src> {
    let m = p.start();
    p.expect(TokenKind::ExclamationMark);

    // The final token after which the declaration is done parsing.
    let mut final_token = TokenKind::Semicolon;

    let kind = match p.expect_contextual_kw() {
        Some("endian") => {
            match p.expect_contextual_kw() {
                Some("le") | Some("be") => (),
                _ => todo!("error"),
            }
            NodeKind::EndiannessDeclaration
        }
        Some("seek") => {
            let kind = match p.expect_contextual_kw() {
                Some("by") => NodeKind::SeekByDeclaration,
                Some("to") => NodeKind::SeekToDeclaration,
                _ => todo!("error"),
            };
            expr(p);
            kind
        }
        Some("scope") => {
            let kind = match p.expect_contextual_kw() {
                Some("at") => NodeKind::ScopeAtDeclaration,
                _ => todo!("error"),
            };

            expr(p);

            if p.at_contextual_kw("until") {
                p.bump();
                expr(p);
            }

            p.expect(TokenKind::LBrace);
            while let Some(kind) = p.cur()
                && kind != TokenKind::RBrace
            {
                struct_content(p);
            }

            final_token = TokenKind::RBrace;

            kind
        }
        Some("align") => {
            expr(p);
            NodeKind::AlignDeclaration
        }
        Some("assert") => {
            expr(p);
            if p.at(TokenKind::Colon) {
                p.expect(TokenKind::Colon);
                expr(p);
            }

            NodeKind::AssertDeclaration
        }
        Some("warn") => {
            if p.at_contextual_kw("if") {
                p.bump();
            } else {
                todo!("warn requires if");
            }

            expr(p);
            if p.at(TokenKind::Colon) {
                p.expect(TokenKind::Colon);
                expr(p);
            }

            NodeKind::WarnIfDeclaration
        }
        Some("recover") => {
            if p.at_contextual_kw("at") {
                p.bump();
            } else {
                todo!("recover currently requires at");
            }

            expr(p);

            NodeKind::RecoveryDeclaration
        }
        _ => todo!(),
    };

    p.complete_after(m, kind, final_token)
}

/// Parses a struct field.
fn struct_field<'p, 'src>(p: &'p mut Parser<'src>) -> Completed<'p, 'src> {
    let m = p.start();

    p.expect(TokenKind::Identifier);
    top_level_parse_type(p);
    if p.cur() == Some(TokenKind::Equals) {
        p.expect(TokenKind::Equals);
        expr(p);
    }

    p.complete_after(m, NodeKind::StructField, TokenKind::Semicolon)
}

/// Parses a top-level parse type.
fn top_level_parse_type<'p, 'src>(p: &'p mut Parser<'src>) -> Completed<'p, 'src> {
    parse_type_raw(p, false)
}

/// Parses a nested parse type.
fn nested_parse_type<'p, 'src>(p: &'p mut Parser<'src>) -> Completed<'p, 'src> {
    parse_type_raw(p, true)
}

/// Parses a parse type.
fn parse_type_raw<'p, 'src>(p: &'p mut Parser<'src>, nested: bool) -> Completed<'p, 'src> {
    let m = p.start();

    match p.cur() {
        Some(TokenKind::BytesKw) => {
            if !nested && matches!(p.peek().nth(1), Some((_, TokenKind::Equals))) {
                p.complete_after(m, NodeKind::BytesParseType, TokenKind::BytesKw)
            } else {
                p.expect(TokenKind::BytesKw);
                repeat_decl(p).and_complete(m, NodeKind::BytesParseType)
            }
        }
        Some(TokenKind::LBrace) => {
            p.expect(TokenKind::LBrace);
            while p.cur().is_some_and(|t| t != TokenKind::RBrace) {
                struct_content(p);
            }
            p.complete_after(m, NodeKind::AnonymousStructParseType, TokenKind::RBrace)
        }
        Some(TokenKind::Identifier) => {
            p.complete_after(m, NodeKind::NamedParseType, TokenKind::Identifier)
        }
        Some(TokenKind::LBracket) => {
            p.expect(TokenKind::LBracket);
            nested_parse_type(p);
            p.expect(TokenKind::RBracket);

            repeat_decl(p).and_complete(m, NodeKind::RepeatParseType)
        }
        Some(TokenKind::SwitchKw) => {
            p.expect(TokenKind::SwitchKw);
            expr(p);
            p.expect(TokenKind::LBrace);

            while p.cur().is_some_and(|t| t != TokenKind::Underscore) {
                let m = p.start();

                expr(p);
                p.expect(TokenKind::Equals);
                p.expect(TokenKind::RAngle);
                nested_parse_type(p);

                p.complete_after(m, NodeKind::SwitchParseTypeArm, TokenKind::Comma);
            }

            p.expect(TokenKind::Underscore);
            p.expect(TokenKind::Equals);
            p.expect(TokenKind::RAngle);
            nested_parse_type(p);

            if p.at(TokenKind::Comma) {
                p.expect(TokenKind::Comma);
            }

            p.complete_after(m, NodeKind::SwitchParseType, TokenKind::RBrace)
        }
        _ => {
            p.dbg();
            todo!("error")
        }
    }
}

/// Parses a repeating declaration.
fn repeat_decl<'p, 'src>(p: &'p mut Parser<'src>) -> Completed<'p, 'src> {
    let m = p.start();

    match p.expect_contextual_kw() {
        Some("len") => expr(p).and_complete(m, NodeKind::RepeatLenDecl),
        Some("while") => expr(p).and_complete(m, NodeKind::RepeatWhileDecl),
        _ => todo!("error"),
    }
}
