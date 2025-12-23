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

/// Parses a struct block (`{` StructContent* `}`).
fn struct_block<'p, 'src>(p: &'p mut Parser<'src>) -> Completed<'p, 'src> {
    let m = p.start();
    p.expect(TokenKind::LBrace);

    while p.cur().is_some_and(|t| t != TokenKind::RBrace) {
        struct_content(p);
    }

    p.complete_after(m, NodeKind::StructBlock, TokenKind::RBrace)
}

/// Parses a `struct`.
fn r#struct<'p, 'src>(p: &'p mut Parser<'src>) -> Completed<'p, 'src> {
    let m = p.start();

    p.expect(TokenKind::StructKw);
    p.expect(TokenKind::Identifier);

    struct_block(p).and_complete(m, NodeKind::Struct)
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

/// Parses an `if` chain.
fn if_chain<'p, 'src>(p: &'p mut Parser<'src>) -> Completed<'p, 'src> {
    let m = p.start();

    if p.expect_and_bump_contextual_kw() != Some("if") {
        todo!()
    }

    expr(p);

    // handle trivia manually here to satisfy the borrow checker (we may or may not need to parse
    // further things before finishing)
    struct_block(p).handle_trivia_manually();

    let else_is_next_token = p
        .peek()
        .next()
        .map(|(index, _)| p.text_at(index) == Some("else"))
        .unwrap_or(false);

    if else_is_next_token {
        // bump trivia first
        p.trivia_bumper().bump();

        p.bump();

        let m_else_part = p.start();

        if p.at_contextual_kw("if") {
            if_chain(p)
        } else {
            struct_block(p).and_complete(m_else_part, NodeKind::ElseBlock)
        }
        .and_complete(m_else_part, NodeKind::ElsePart)
        .and_complete(m, NodeKind::IfChain)
    } else {
        // complete the chain without bumping trivia
        let completed = p.complete(m, NodeKind::IfChain);

        // then use the finished marker to create a trivia bumper again
        p.completed_from_marker(completed)
    }
}

/// Parses a declaration.
fn decl<'p, 'src>(p: &'p mut Parser<'src>) -> Completed<'p, 'src> {
    let m = p.start();
    p.expect(TokenKind::ExclamationMark);

    match p.expect_peek_contextual_kw() {
        Some("endian") => {
            p.bump();
            match p.expect_and_bump_contextual_kw() {
                Some("le") | Some("be") => (),
                _ => todo!("error"),
            }

            p.complete_after(m, NodeKind::EndiannessDeclaration, TokenKind::Semicolon)
        }
        Some("seek") => {
            p.bump();
            let kind = match p.expect_and_bump_contextual_kw() {
                Some("by") => NodeKind::SeekByDeclaration,
                Some("to") => NodeKind::SeekToDeclaration,
                _ => todo!("error"),
            };
            expr(p);

            p.complete_after(m, kind, TokenKind::Semicolon)
        }
        Some("scope") => {
            p.bump();
            let kind = match p.expect_and_bump_contextual_kw() {
                Some("at") => NodeKind::ScopeAtDeclaration,
                _ => todo!("error"),
            };

            expr(p);

            if p.at_contextual_kw("until") {
                p.bump();
                expr(p);
            }

            struct_block(p).and_complete(m, kind)
        }
        Some("if") => if_chain(p).and_complete(m, NodeKind::IfDeclaration),
        Some("align") => {
            p.bump();
            expr(p);

            p.complete_after(m, NodeKind::AlignDeclaration, TokenKind::Semicolon)
        }
        Some("assert") => {
            p.bump();
            expr(p);
            if p.at(TokenKind::Colon) {
                p.expect(TokenKind::Colon);
                expr(p);
            }

            p.complete_after(m, NodeKind::AssertDeclaration, TokenKind::Semicolon)
        }
        Some("warn") => {
            p.bump();
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

            p.complete_after(m, NodeKind::WarnIfDeclaration, TokenKind::Semicolon)
        }
        Some("recover") => {
            p.bump();
            if p.at_contextual_kw("at") {
                p.bump();
            } else {
                todo!("recover currently requires at");
            }

            expr(p);

            p.complete_after(m, NodeKind::RecoveryDeclaration, TokenKind::Semicolon)
        }
        _ => todo!("error"),
    }
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
        Some(TokenKind::Identifier)
            if matches!(p.cur_text(), Some("u" | "i"))
                && matches!(p.peek().nth(1), Some((_, TokenKind::LParen))) =>
        {
            let kind = match p.expect_and_bump_contextual_kw() {
                Some("i") => NodeKind::DynamicSizeIntParseType,
                Some("u") => NodeKind::DynamicSizeUIntParseType,
                _ => unreachable!(),
            };
            p.expect(TokenKind::LParen);

            expr(p);

            p.complete_after(m, kind, TokenKind::RParen)
        }
        Some(TokenKind::LBrace) => {
            struct_block(p).and_complete(m, NodeKind::AnonymousStructParseType)
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

    match p.expect_and_bump_contextual_kw() {
        Some("len") => expr(p).and_complete(m, NodeKind::RepeatLenDecl),
        Some("while") => expr(p).and_complete(m, NodeKind::RepeatWhileDecl),
        _ => todo!("error"),
    }
}
