//! Contains the actual syntax descriptions.

pub(crate) use expressions::expr;

use crate::compile::{lexer::TokenKind, syntax::NodeKind};

use super::infrastructure::Parser;

mod expressions;

/// Parses the root node of the grammar.
pub(crate) fn root(p: &mut Parser) {
    p.node(|p| {
        while p.cur().is_some() {
            p.ensure_progress(|p| {
                struct_content(p);
            });
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
        p.with_consuming_recovery(TokenKind::RBrace, |p| {
            while !p.at_recovery_token() {
                p.ensure_progress(|p| {
                    struct_content(p);
                });
            }
        });

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
        p.with_consuming_recovery(TokenKind::Semicolon, |p| {
            p.expect(TokenKind::LetKw);
            p.expect(TokenKind::Identifier);
            p.expect(TokenKind::Equals);
            expr(p);
        });

        NodeKind::LetStatement
    });
}

/// Parses an `if` chain.
fn if_chain(p: &mut Parser) {
    p.node(|p| {
        if p.peek_contextual_kw() != Some("if") {
            unreachable!("if chain is only parsed when it starts with `if`");
        }
        p.bump();

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

        match p.peek_contextual_kw() {
            Some("endian") => {
                p.with_consuming_recovery(TokenKind::Semicolon, |p| {
                    p.bump();
                    match p.peek_contextual_kw() {
                        Some("le") | Some("be") => p.bump(),
                        // let lowering deal with the error
                        Some(_) => p.bump(),
                        _ => p.expect_error(&["`le`", "`be`"]),
                    }
                });

                NodeKind::EndiannessDeclaration
            }
            Some("seek") => p.with_consuming_recovery(TokenKind::Semicolon, |p| {
                p.bump();
                let kind = match p.peek_contextual_kw() {
                    Some("by") => {
                        p.bump();
                        Some(NodeKind::SeekByDeclaration)
                    }
                    Some("to") => {
                        p.bump();
                        Some(NodeKind::SeekToDeclaration)
                    }
                    _ => {
                        p.expect_error(&["`by`", "`to`"]);
                        p.recover();
                        None
                    }
                };

                if let Some(kind) = kind {
                    expr(p);
                    kind
                } else {
                    NodeKind::Error
                }
            }),
            Some("scope") => {
                p.bump();
                let kind = match p.peek_contextual_kw() {
                    Some("at") => {
                        p.bump();
                        Some(NodeKind::ScopeAtDeclaration)
                    }
                    Some("in") => {
                        p.bump();
                        Some(NodeKind::ScopeInDeclaration)
                    }
                    _ => {
                        p.expect_error(&["`at`", "`in`"]);
                        None
                    }
                };

                if let Some(kind) = kind {
                    expr(p);

                    if kind == NodeKind::ScopeAtDeclaration && p.at_contextual_kw("until") {
                        p.bump();
                        p.with_unconsuming_recovery(TokenKind::LBrace, |p| {
                            expr(p);
                        });
                    }

                    struct_block(p);
                    kind
                } else {
                    NodeKind::Error
                }
            }
            Some("if") => {
                if_chain(p);
                NodeKind::IfDeclaration
            }
            Some("align") => {
                p.with_consuming_recovery(TokenKind::Semicolon, |p| {
                    p.bump();
                    expr(p);
                });

                NodeKind::AlignDeclaration
            }
            Some("assert") => {
                p.with_consuming_recovery(TokenKind::Semicolon, |p| {
                    p.bump();
                    expr(p);
                    if p.at(TokenKind::Colon) {
                        p.expect(TokenKind::Colon);
                        expr(p);
                    }
                });

                NodeKind::AssertDeclaration
            }
            Some("warn") => {
                p.with_consuming_recovery(TokenKind::Semicolon, |p| {
                    p.bump();
                    if p.at_contextual_kw("if") {
                        p.bump();
                    } else {
                        p.expect_error(&["`if`"]);
                    }

                    expr(p);
                    if p.at(TokenKind::Colon) {
                        p.expect(TokenKind::Colon);
                        expr(p);
                    }
                });

                NodeKind::WarnIfDeclaration
            }
            Some("recover") => {
                p.with_consuming_recovery(TokenKind::Semicolon, |p| {
                    p.bump();
                    if p.at_contextual_kw("at") {
                        p.bump();
                    } else {
                        p.expect_error(&["`at`"]);
                    }

                    expr(p);
                });

                NodeKind::RecoveryDeclaration
            }
            _ => {
                p.expect_error(&[
                    "`endian`",
                    "`seek`",
                    "`scope`",
                    "`if`",
                    "`align`",
                    "`assert`",
                    "`warn`",
                    "`recover`",
                ]);
                p.with_consuming_recovery(TokenKind::Semicolon, |p| {
                    p.recover();
                });

                NodeKind::Error
            }
        }
    });
}

/// Parses a struct field.
fn struct_field(p: &mut Parser) {
    p.node(|p| {
        p.with_consuming_recovery(TokenKind::Semicolon, |p| {
            p.expect(TokenKind::Identifier);
            top_level_parse_type(p);
            if p.cur() == Some(TokenKind::Equals) {
                p.expect(TokenKind::Equals);
                expr(p);
            }
        });

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
            let kind = match p.peek_contextual_kw() {
                Some("i") => NodeKind::DynamicSizeIntParseType,
                Some("u") => NodeKind::DynamicSizeUIntParseType,
                _ => unreachable!(),
            };
            p.bump();

            p.expect(TokenKind::LParen);
            p.with_consuming_recovery(TokenKind::RParen, |p| {
                expr(p);
            });

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
            p.with_consuming_recovery(TokenKind::RBrace, |p| {
                p.with_consuming_recovery(TokenKind::Underscore, |p| {
                    while !p.at_recovery_token() {
                        p.ensure_progress(|p| {
                            p.node(|p| {
                                p.with_consuming_recovery(TokenKind::Comma, |p| {
                                    expr(p);
                                    p.expect(TokenKind::Equals);
                                    p.expect(TokenKind::RAngle);
                                    nested_parse_type(p);
                                });

                                NodeKind::SwitchParseTypeArm
                            });
                        });
                    }
                });

                p.expect(TokenKind::Equals);
                p.expect(TokenKind::RAngle);
                nested_parse_type(p);

                if p.at(TokenKind::Comma) {
                    p.expect(TokenKind::Comma);
                }
            });

            NodeKind::SwitchParseType
        }
        _ => {
            p.expect_error(&["an identifier", "`{`", "`[`", "`bytes`", "`switch`"]);
            p.recover();

            NodeKind::Error
        }
    });
}

/// Parses a repeating declaration.
fn repeat_decl(p: &mut Parser) {
    p.node(|p| match p.peek_contextual_kw() {
        Some("len") => {
            p.bump();
            expr(p);
            NodeKind::RepeatLenDecl
        }
        Some("while") => {
            p.bump();
            expr(p);
            NodeKind::RepeatWhileDecl
        }
        _ => {
            p.expect_error(&["`len`", "`while`"]);
            p.recover();

            NodeKind::Error
        }
    });
}
