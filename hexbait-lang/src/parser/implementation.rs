//! Contains the actual syntax descriptions.

use expressions::expr;

use crate::{NodeKind, lexer::TokenKind};

use super::infrastructure::{Parser, TriviaBumper};

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
fn struct_content(p: &mut Parser) {
    let Some(kind) = p.cur() else {
        todo!("error here")
    };

    match kind {
        TokenKind::StructKw => r#struct(p),
        TokenKind::ExclamationMark => decl(p),
        _ => struct_field(p),
    };
}

/// Parses a struct.
fn r#struct(p: &mut Parser) {
    let m = p.start();

    p.expect(TokenKind::StructKw);
    p.expect(TokenKind::Identifier);
    p.expect(TokenKind::LBrace);

    while let Some(kind) = p.cur()
        && kind != TokenKind::RBrace
    {
        struct_content(p);
    }

    p.complete_after(m, NodeKind::Struct, TokenKind::RBrace);
}

/// Parses a declaration.
fn decl(p: &mut Parser) {
    let kind;

    let m = p.start();
    p.expect(TokenKind::ExclamationMark);

    // The final token after which the declaration is done parsing.
    let mut final_token = TokenKind::Semicolon;

    match p.expect_contextual_kw() {
        Some("endian") => {
            kind = NodeKind::EndiannessDeclaration;
            match p.expect_contextual_kw() {
                Some("le") | Some("be") => (),
                _ => todo!("error"),
            }
        }
        Some("seek") => {
            match p.expect_contextual_kw() {
                Some("by") => {
                    kind = NodeKind::SeekByDeclaration;
                }
                Some("to") => {
                    kind = NodeKind::SeekToDeclaration;
                }
                _ => todo!("error"),
            }
            expr(p);
        }
        Some("scope") => {
            match p.expect_contextual_kw() {
                Some("at") => {
                    kind = NodeKind::ScopeAtDeclaration;
                }
                _ => todo!("error"),
            }

            expr(p);

            p.expect(TokenKind::LBrace);
            while let Some(kind) = p.cur()
                && kind != TokenKind::RBrace
            {
                struct_content(p);
            }

            final_token = TokenKind::RBrace;
        }
        Some("align") => {
            kind = NodeKind::AlignDeclaration;
            expr(p);
        }
        _ => todo!(),
    }

    p.complete_after(m, kind, final_token);
}

/// Parses a struct field.
fn struct_field(p: &mut Parser) {
    let m = p.start();

    p.expect(TokenKind::Identifier);
    top_level_parse_type(p);
    if p.cur() == Some(TokenKind::Equals) {
        p.expect(TokenKind::Equals);
        expr(p);
    }

    p.complete_after(m, NodeKind::StructField, TokenKind::Semicolon);
}

/// Parses a top-level parse type.
fn top_level_parse_type<'p, 'src>(p: &'p mut Parser<'src>) -> TriviaBumper<'p, 'src> {
    parse_type_raw(p, false)
}

/// Parses a nested parse type.
fn nested_parse_type<'p, 'src>(p: &'p mut Parser<'src>) -> TriviaBumper<'p, 'src> {
    parse_type_raw(p, true)
}

/// Parses a parse type.
fn parse_type_raw<'p, 'src>(p: &'p mut Parser<'src>, nested: bool) -> TriviaBumper<'p, 'src> {
    let m = p.start();

    match p.cur() {
        Some(TokenKind::BytesKw) => {
            if !nested && matches!(p.peek().nth(1), Some((_, TokenKind::Equals))) {
                p.complete_after(m, NodeKind::BytesParseType, TokenKind::BytesKw)
                    .1
            } else {
                p.expect(TokenKind::BytesKw);
                repeat_decl(p).and_complete(m, NodeKind::BytesParseType)
            }
        }
        Some(TokenKind::Identifier) => {
            p.complete_after(m, NodeKind::NamedParseType, TokenKind::Identifier)
                .1
        }
        Some(TokenKind::LBracket) => {
            p.expect(TokenKind::LBracket);
            nested_parse_type(p);
            p.expect(TokenKind::RBracket);

            repeat_decl(p).and_complete(m, NodeKind::RepeatParseType)
        }
        _ => todo!("error"),
    }
}

/// Parses a repeating declaration.
fn repeat_decl<'p, 'src>(p: &'p mut Parser<'src>) -> TriviaBumper<'p, 'src> {
    let m = p.start();

    match p.expect_contextual_kw() {
        Some("len") => expr(p).and_complete(m, NodeKind::RepeatLenDecl),
        Some("while") => expr(p).and_complete(m, NodeKind::RepeatWhileDecl),
        _ => todo!("error"),
    }
}
