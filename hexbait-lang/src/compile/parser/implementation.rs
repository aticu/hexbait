//! Contains the actual syntax descriptions.

pub(crate) use expressions::expr;

use crate::compile::{lexer::TokenKind, syntax::NodeKind};

use super::infrastructure::Parser;

mod expressions;

/// Parses the root node of the grammar.
pub(crate) fn root(p: &mut Parser) {
    p.node(|p| {
        while p.cur().is_some() {
            struct_content(p);
        }

        NodeKind::File
    });
}

/// Parses the content of a struct.
fn struct_content(p: &mut Parser) {
    let Some(kind) = p.cur() else {
        unreachable!("should only try to parse struct content when tokens are present");
    };

    match kind {
        TokenKind::StructKw => r#struct(p),
        TokenKind::LetKw => r#let(p),
        TokenKind::ExclamationMark => decl(p),
        _ => struct_field(p),
    }
}

/// Parses a struct block (`{` StructContent* `}`).
fn struct_block(p: &mut Parser) {
    p.node(|p| {
        p.expect(TokenKind::LBrace);
        while p.cur().is_some_and(|t| t != TokenKind::RBrace) {
            struct_content(p);
        }
        p.expect(TokenKind::RBrace);

        NodeKind::StructBlock
    });
}

/// Parses a `struct`.
fn r#struct(p: &mut Parser) {
    p.node(|p| {
        p.expect(TokenKind::StructKw);
        p.expect(TokenKind::Identifier);

        struct_block(p);

        NodeKind::Struct
    });
}

/// Parses a `let` statement.
fn r#let(p: &mut Parser) {
    p.node(|p| {
        p.expect(TokenKind::LetKw);
        p.expect(TokenKind::Identifier);
        p.expect(TokenKind::Equals);
        expr(p);
        p.expect(TokenKind::Semicolon);

        NodeKind::LetStatement
    });
}

/// Parses an `if` chain.
fn if_chain(p: &mut Parser) {
    p.node(|p| {
        if p.expect_and_bump_contextual_kw() != Some("if") {
            todo!()
        }

        expr(p);

        struct_block(p);

        let else_is_next_token = p.peek().next().map(|t| t.text == "else").unwrap_or(false);

        if else_is_next_token {
            p.bump();

            if p.at_contextual_kw("if") {
                if_chain(p)
            } else {
                p.node(|p| {
                    struct_block(p);
                    NodeKind::ElseBlock
                });
            }
        }

        NodeKind::IfChain
    });
}

/// Parses a declaration.
fn decl(p: &mut Parser) {
    p.node(|p| {
        p.expect(TokenKind::ExclamationMark);

        match p.expect_peek_contextual_kw() {
            Some("endian") => {
                p.bump();
                match p.expect_and_bump_contextual_kw() {
                    Some("le") | Some("be") => (),
                    _ => todo!("error"),
                }

                p.expect(TokenKind::Semicolon);
                NodeKind::EndiannessDeclaration
            }
            Some("seek") => {
                p.bump();
                let kind = match p.expect_and_bump_contextual_kw() {
                    Some("by") => NodeKind::SeekByDeclaration,
                    Some("to") => NodeKind::SeekToDeclaration,
                    _ => todo!("error"),
                };
                expr(p);

                p.expect(TokenKind::Semicolon);
                kind
            }
            Some("scope") => {
                p.bump();
                let kind = match p.expect_and_bump_contextual_kw() {
                    Some("at") => NodeKind::ScopeAtDeclaration,
                    Some("in") => NodeKind::ScopeInDeclaration,
                    _ => todo!("error"),
                };

                expr(p);

                if kind == NodeKind::ScopeAtDeclaration && p.at_contextual_kw("until") {
                    p.bump();
                    expr(p);
                }

                struct_block(p);
                kind
            }
            Some("if") => {
                if_chain(p);
                NodeKind::IfDeclaration
            }
            Some("align") => {
                p.bump();
                expr(p);
                p.expect(TokenKind::Semicolon);

                NodeKind::AlignDeclaration
            }
            Some("assert") => {
                p.bump();
                expr(p);
                if p.at(TokenKind::Colon) {
                    p.expect(TokenKind::Colon);
                    expr(p);
                }
                p.expect(TokenKind::Semicolon);

                NodeKind::AssertDeclaration
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
                p.expect(TokenKind::Semicolon);

                NodeKind::WarnIfDeclaration
            }
            Some("recover") => {
                p.bump();
                if p.at_contextual_kw("at") {
                    p.bump();
                } else {
                    todo!("recover currently requires at");
                }

                expr(p);
                p.expect(TokenKind::Semicolon);

                NodeKind::RecoveryDeclaration
            }
            _ => todo!("error"),
        }
    });
}

/// Parses a struct field.
fn struct_field(p: &mut Parser) {
    p.node(|p| {
        p.expect(TokenKind::Identifier);
        top_level_parse_type(p);
        if p.cur() == Some(TokenKind::Equals) {
            p.expect(TokenKind::Equals);
            expr(p);
        }
        p.expect(TokenKind::Semicolon);

        NodeKind::StructField
    });
}

/// Parses a top-level parse type.
fn top_level_parse_type(p: &mut Parser) {
    parse_type_raw(p, false);
}

/// Parses a nested parse type.
fn nested_parse_type(p: &mut Parser) {
    parse_type_raw(p, true);
}

/// Parses a parse type.
fn parse_type_raw(p: &mut Parser, nested: bool) {
    p.node(|p| match p.cur() {
        Some(TokenKind::BytesKw) => {
            if !nested && matches!(p.peek().nth(1).map(|t| t.kind), Some(TokenKind::Equals)) {
                p.expect(TokenKind::BytesKw);
            } else {
                p.expect(TokenKind::BytesKw);
                repeat_decl(p);
            }

            NodeKind::BytesParseType
        }
        Some(TokenKind::Identifier)
            if matches!(p.peek().next().map(|t| t.text), Some("u" | "i"))
                && matches!(p.peek().nth(1).map(|t| t.kind), Some(TokenKind::LParen)) =>
        {
            let kind = match p.expect_and_bump_contextual_kw() {
                Some("i") => NodeKind::DynamicSizeIntParseType,
                Some("u") => NodeKind::DynamicSizeUIntParseType,
                _ => unreachable!(),
            };

            p.expect(TokenKind::LParen);
            expr(p);
            p.expect(TokenKind::RParen);

            kind
        }
        Some(TokenKind::LBrace) => {
            struct_block(p);
            NodeKind::AnonymousStructParseType
        }
        Some(TokenKind::Identifier) => {
            p.expect(TokenKind::Identifier);
            NodeKind::NamedParseType
        }
        Some(TokenKind::LBracket) => {
            p.expect(TokenKind::LBracket);
            nested_parse_type(p);
            p.expect(TokenKind::RBracket);

            repeat_decl(p);
            NodeKind::RepeatParseType
        }
        Some(TokenKind::SwitchKw) => {
            p.expect(TokenKind::SwitchKw);
            expr(p);
            p.expect(TokenKind::LBrace);

            while p.cur().is_some_and(|t| t != TokenKind::Underscore) {
                p.node(|p| {
                    expr(p);
                    p.expect(TokenKind::Equals);
                    p.expect(TokenKind::RAngle);
                    nested_parse_type(p);
                    p.expect(TokenKind::Comma);

                    NodeKind::SwitchParseTypeArm
                });
            }

            p.expect(TokenKind::Underscore);
            p.expect(TokenKind::Equals);
            p.expect(TokenKind::RAngle);
            nested_parse_type(p);

            if p.at(TokenKind::Comma) {
                p.expect(TokenKind::Comma);
            }
            p.expect(TokenKind::RBrace);

            NodeKind::SwitchParseType
        }
        _ => {
            p.dbg();
            todo!("error")
        }
    });
}

/// Parses a repeating declaration.
fn repeat_decl(p: &mut Parser) {
    p.node(|p| match p.expect_and_bump_contextual_kw() {
        Some("len") => {
            expr(p);
            NodeKind::RepeatLenDecl
        }
        Some("while") => {
            expr(p);
            NodeKind::RepeatWhileDecl
        }
        _ => todo!("error"),
    });
}
