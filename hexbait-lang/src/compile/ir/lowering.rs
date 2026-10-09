//! Implements lowering the AST to the IR.

use std::sync::Arc;

use crate::{
    Int,
    compile::{
        Diagnostic, Diagnostics,
        ast::{self, AstNode as _},
        diagnostics::Label,
        ir::{
            Block, ConcatArg, DeclarationKind, EnumInfo, EnumVariant, EnumVariantKind, IfBlock,
            IfChain, ParseTypeKind, Repetition, ScopeKind, StructContentKind, StructRef,
            StructRefPart,
        },
        lexer::TokenKind,
        span::Span,
        syntax::{SyntaxKind, SyntaxNode},
    },
    int_from_str,
};

use super::{
    Declaration, Endianness, File, LetStatement, ParseType, RepeatKind, Spanned, StructContent,
    StructField, Symbol,
    expr::{BinOp, Expr, ExprKind, Lit, UnOp},
    str::str_lit_content_to_bytes,
};

macro_rules! parser_unreachable {
    () => {
        unreachable!("this should be rejected by the parser")
    };
}

/// Lowers the given file AST to IR.
pub fn lower_file(file: ast::File, diagnostics: &mut Diagnostics) -> File {
    let mut ctx = LoweringCtx::new(diagnostics);
    let mut out = Vec::new();

    for content in file.struct_content() {
        out.push(ctx.lower_struct_content(content));
    }

    File { content: out }
}

/// Lowers the given expression AST to IR.
pub fn lower_expr(expr: ast::Expr, diagnostics: &mut Diagnostics) -> Expr {
    let mut ctx = LoweringCtx::new(diagnostics);

    ctx.lower_expr(expr)
}

/// The context in which lowering is performed.
struct LoweringCtx<'ctx> {
    /// The diagnostics.
    diagnostics: &'ctx mut Diagnostics,
}

/// Accesses a required field in the given value.
///
/// Logs an error with the given message and returns the error type if the field is not present.
macro_rules! required_field {
    ($value:expr => $field:ident ? $this:ident => $err_ty:expr) => {
        match $value.$field() {
            Some(val) => val,
            None => {
                assert!(
                    $this
                        .diagnostics
                        .contains_error_in(missing_part_error_span($value.syntax())),
                    "required field not present, but there are no errors"
                );
                return $err_ty;
            }
        }
    };
}

/// Returns the span in which parse errors for missing parts of `node` are reported.
///
/// The parser attaches errors about missing tokens to the next token, which lies outside of
/// `node` (especially if `node` is empty), so the span is extended up to the end of the next
/// non-trivia token.
fn missing_part_error_span(node: &SyntaxNode) -> Span {
    let range = node.text_range();
    let root = node.ancestors().last().unwrap_or_else(|| node.clone());
    let mut next = root.token_at_offset(range.end()).right_biased();
    while let Some(token) = &next
        && matches!(token.kind(), SyntaxKind::Token { kind } if kind.is_trivia())
    {
        next = token.next_token();
    }
    let end = next
        .map(|token| token.text_range().end())
        .unwrap_or(root.text_range().end());

    Span::from(rowan::TextRange::new(range.start(), end))
}

impl LoweringCtx<'_> {
    /// Creates a new lowering context.
    fn new(diagnostics: &mut Diagnostics) -> LoweringCtx<'_> {
        LoweringCtx { diagnostics }
    }

    /// Shows the given error message for the given span.
    fn add_diagnostic(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.add_diagnostic(diagnostic);
    }

    /// Lowers the given `struct` content AST to IR.
    fn lower_struct_content(&mut self, struct_content: ast::StructContent) -> StructContent {
        let span = struct_content.span();
        let kind = match struct_content {
            ast::StructContent::Declaration(declaration) => self
                .lower_declaration(declaration)
                .map(StructContentKind::Declaration),
            ast::StructContent::StructField(struct_field) => self
                .lower_struct_field(struct_field)
                .map(StructContentKind::Field),
            ast::StructContent::Struct(_) => {
                self.add_diagnostic(Diagnostic::error(
                    "named `struct`s currently unsupported",
                    Label::new("unsupported language feature", struct_content.span()),
                ));
                None
            }
            ast::StructContent::LetStatement(let_statement) => self
                .lower_let_statement(let_statement)
                .map(StructContentKind::LetStatement),
        }
        .unwrap_or(StructContentKind::Error);

        StructContent { kind, span }
    }

    /// Lowers the given `struct` block AST to IR.
    fn lower_block(&mut self, block: ast::StructBlock) -> Block {
        Block {
            content: block
                .struct_content()
                .map(|content| self.lower_struct_content(content))
                .collect(),
            span: block.span(),
        }
    }

    /// Lowers the given AST `struct` field to IR.
    fn lower_struct_field(&mut self, struct_field: ast::StructField) -> Option<StructField> {
        let expected = struct_field
            .expected()
            .map(|expected| self.lower_expr(expected));

        Some(StructField {
            name: Spanned::<Symbol>::from(required_field!(struct_field => name ? self => None)),
            ty: self.lower_parse_type(
                required_field!(struct_field => parse_type ? self => None),
                &expected,
            ),
            expected,
        })
    }

    /// Lowers the given AST parse type to IR.
    fn lower_parse_type(
        &mut self,
        parse_type: ast::ParseType,
        expected: &Option<Expr>,
    ) -> ParseType {
        let span = parse_type.span();
        let kind = self.lower_parse_type_kind(parse_type, expected);

        ParseType { kind, span }
    }

    /// Lowers the given AST parse type into an IR parse type kind.
    fn lower_parse_type_kind(
        &mut self,
        parse_type: ast::ParseType,
        expected: &Option<Expr>,
    ) -> ParseTypeKind {
        match parse_type {
            ast::ParseType::NamedParseType(named_parse_type) => {
                let name_token = required_field!(named_parse_type => name ? self => ParseTypeKind::Error);

                let name = name_token.text();
                if (name.starts_with("i") || name.starts_with("u"))
                    && let Ok(num_bits) = name[1..].parse::<u32>()
                {
                    ParseTypeKind::Integer {
                        bit_width: num_bits,
                        signed: name.starts_with("i"),
                    }
                } else {
                    ParseTypeKind::Named {
                        name: Spanned::<Symbol>::from(name_token),
                    }
                }
            }
            ast::ParseType::DynamicSizeIntParseType(dynamic_int_parse_type) => {
                ParseTypeKind::DynamicInteger {
                    bit_width: self.lower_expr(required_field!(dynamic_int_parse_type => expr ? self  => ParseTypeKind::Error)),
                    signed: true,
                }
            }
            ast::ParseType::DynamicSizeUIntParseType(dynamic_uint_parse_type) => {
                ParseTypeKind::DynamicInteger {
                    bit_width: self.lower_expr(required_field!(dynamic_uint_parse_type => expr ? self  => ParseTypeKind::Error)),
                    signed: false,
                }
            }
            ast::ParseType::BytesParseType(bytes_parse_type) => {
                let repetition = if let Some(repeat_decl) = bytes_parse_type.repeat_decl() {
                    self.lower_repetition(repeat_decl)
                } else {
                    let expected = expected.as_ref().parser_expect();
                    let ExprKind::Lit(Lit::Bytes(bytes)) = &expected.kind else {
                        if !matches!(expected.kind, ExprKind::Error) {
                            self.add_diagnostic(Diagnostic::error(
                                "expected bytes value as the expected value for bytes type",
                                Label::new("found unexpected type", expected.span)
                            ));
                        }
                        return ParseTypeKind::Error
                    };
                    Repetition {
                        kind: RepeatKind::Len {
                            count: Expr {
                                kind: ExprKind::Lit(Lit::Int(Int::from(bytes.len()))),
                                span: expected.span
                            }
                        },
                        span: expected.span
                    }
                };

                ParseTypeKind::Bytes { repetition }
            }
            ast::ParseType::RepeatParseType(repeat_parse_type) => {
                ParseTypeKind::Repeating {
                    parse_type: Box::new(self.lower_parse_type(
                        required_field!(repeat_parse_type => ty ? self => ParseTypeKind::Error),
                        &None,
                    )),
                    repetition: self.lower_repetition(
                        required_field!(repeat_parse_type => repetition ? self => ParseTypeKind::Error)
                    ),
                }
            }
            ast::ParseType::AnonymousStructParseType(struct_parse_type) => {
                ParseTypeKind::Struct {
                    block: self.lower_block(required_field!(struct_parse_type => struct_block ? self => ParseTypeKind::Error))
                }
            }
            ast::ParseType::SwitchParseType(switch_parse_type) => {
                let scrutinee = self.lower_expr(
                    required_field!(switch_parse_type => scrutinee ? self => ParseTypeKind::Error)
                );

                let mut branches = Vec::new();

                for arm in switch_parse_type.switch_parse_type_arm() {
                    let value = self.lower_expr(
                        required_field!(arm => val ? self => ParseTypeKind::Error)
                    );
                    let parse_ty = self.lower_parse_type(
                        required_field!(arm => parse_type ? self => ParseTypeKind::Error),
                        &None,
                    );

                    if let ExprKind::Lit(lit) = value.kind {
                        branches.push((lit, value.span, parse_ty));
                    } else {
                        self.add_diagnostic(Diagnostic::error("switch branch must be a literal", Label::new("not a literal", value.span)));
                    }
                }

                let default = Box::new(self.lower_parse_type(
                    required_field!(switch_parse_type => default ? self => ParseTypeKind::Error),
                    &None
                ));

                ParseTypeKind::Switch { scrutinee, branches, default }
            }
            ast::ParseType::EnumParseType(enum_parse_type) => {
                let backing_type = self.lower_parse_type(required_field!(enum_parse_type => size ? self => ParseTypeKind::Error), &None);

                let mut variants = Vec::new();
                for arm in enum_parse_type.enum_parse_type_arm() {
                    let name = Spanned::<Symbol>::from(required_field!(arm => name ? self => ParseTypeKind::Error));
                    let start = self.lower_expr(required_field!(arm => start ? self => ParseTypeKind::Error));
                    let end = arm.end().map(|end| self.lower_expr(end));

                    fn extract_lit(this: &mut LoweringCtx, expr: Expr) -> Option<Int> {
                        match expr.kind {
                            ExprKind::Lit(lit) => match lit {
                                Lit::Int(int) => Some(int),
                                Lit::Bytes(_) |
                                Lit::Bool(_) |
                                Lit::Enum(_) => {
                                    this.add_diagnostic(Diagnostic::error(
                                        "expected integer literal",
                                        Label::new("unexpected literal type", expr.span)
                                    ));
                                    None
                                }
                            }
                            ExprKind::Offset |
                            ExprKind::Last |
                            ExprKind::Len |
                            ExprKind::FieldAccess {..} |
                            ExprKind::BinOp {..} |
                            ExprKind::Peek {..} |
                            ExprKind::Concat { .. } => {
                                this.add_diagnostic(Diagnostic::error(
                                    "expected integer literal",
                                    Label::new("unexpected expression type", expr.span)
                                ));
                                None
                            }
                            ExprKind::Error => None,
                            ExprKind::UnOp { op, operand } => match op {
                                UnOp::Neg => extract_lit(this, *operand).map(|lit| -lit),
                                UnOp::Plus => extract_lit(this, *operand),
                                UnOp::Not => {
                                    this.add_diagnostic(Diagnostic::error(
                                        "expected integer literal",
                                        Label::new("unexpected literal type", expr.span)
                                    ));
                                    None
                                },
                            }
                        }
                    }

                    let start_span = start.span;
                    let Some(start) = extract_lit(self, start) else {
                        return ParseTypeKind::Error;
                    };
                    let kind = match end {
                        Some(end) => {
                            let end_span = end.span;
                            let Some(end) = extract_lit(self, end) else {
                                return ParseTypeKind::Error;
                            };
                            Spanned { inner: EnumVariantKind::Range { start, end }, span: start_span.join(end_span) }
                        }
                        None => Spanned { inner: EnumVariantKind::SingleValue(start), span: start_span },
                    };

                    variants.push(EnumVariant {
                        name,
                        kind,
                    });
                }

                ParseTypeKind::Enum { backing_type: Box::new(backing_type), enum_info: Arc::new(EnumInfo {
                    variants,
                })}
            }
        }
    }

    /// Lowers the given AST repetition to IR.
    fn lower_repetition(&mut self, repetition: ast::RepeatDecl) -> Repetition {
        let span = repetition.span();
        Repetition {
            kind: match repetition {
                ast::RepeatDecl::RepeatLenDecl(repeat_len_decl) => RepeatKind::Len {
                    count: self.lower_expr(
                        required_field!(repeat_len_decl => count ? self => Repetition { kind: RepeatKind::Error, span }),
                    ),
                },
                ast::RepeatDecl::RepeatWhileDecl(repeat_while_decl) => RepeatKind::While {
                    condition: self.lower_expr(
                        required_field!(repeat_while_decl => condition ? self => Repetition { kind: RepeatKind::Error, span }),
                    ),
                },
            },
            span
        }
    }

    /// Lowers the given AST expression to IR.
    fn lower_expr(&mut self, expr: ast::Expr) -> Expr {
        let span = expr.span();
        let kind = self.lower_expr_kind(expr);

        Expr { kind, span }
    }

    /// Lowers the given AST expression into an IR expression kind.
    fn lower_expr_kind(&mut self, expr: ast::Expr) -> ExprKind {
        match expr {
            ast::Expr::Atom(atom) => self.lower_atom(atom),
            ast::Expr::Metavar(metavar) => {
                let name = required_field!(metavar => name ? self => ExprKind::Error);
                match name.text() {
                    "offset" => ExprKind::Offset,
                    "last" => ExprKind::Last,
                    "len" => ExprKind::Len,
                    "parent" => {
                        self.add_diagnostic(Diagnostic::error(
                            "metavariable `$parent` cannot only be used in a field access",
                            Label::new("invalid use of `$parent`", metavar.span()),
                        ));
                        ExprKind::Error
                    }
                    var => {
                        self.add_diagnostic(Diagnostic::error(
                            format!("metavariable `${var}` is unknown"),
                            Label::new("unknown metavariable", metavar.span()),
                        ));
                        ExprKind::Error
                    }
                }
            }
            ast::Expr::EnumValue(enum_value) => {
                let field = required_field!(enum_value => field ? self => ExprKind::Error);

                ExprKind::Lit(Lit::Enum(field.text().to_string()))
            }
            ast::Expr::ByteConcat(byte_concat) => {
                let mut out = Vec::new();

                for part in byte_concat.tokens() {
                    match part.kind().expect_token() {
                        // Ignore surrounding tokens
                        TokenKind::LAngle | TokenKind::RAngle => (),
                        token if token.is_trivia() => (),

                        TokenKind::StringLiteral => {
                            let text = part.text();
                            // strip the leading and trailing `"` characters
                            let content = &text[1..text.len() - 1];

                            if let Err((msg, _)) = str_lit_content_to_bytes(content, &mut out) {
                                self.add_diagnostic(Diagnostic::error(
                                    "could not lower byte literal",
                                    Label::new(msg, Span::from(part.text_range())),
                                ));
                                return ExprKind::Error;
                            }
                        }
                        TokenKind::ByteLiteral
                        | TokenKind::DecimalIntegerLiteral
                        | TokenKind::Identifier => {
                            let text = part.text();
                            let span = Span::from(part.text_range());
                            if text.len() != 2 {
                                self.add_diagnostic(Diagnostic::error(
                                    "expected hex byte liteal to be of length two",
                                    Label::new("unexpected byte literal size", span),
                                ));
                                return ExprKind::Error;
                            }

                            match u8::from_str_radix(text, 16) {
                                Ok(val) => out.push(val),
                                Err(_) => {
                                    self.add_diagnostic(Diagnostic::error(
                                        "expected two hexdecimal digits",
                                        Label::new("found invalid hexadecimal literal", span),
                                    ));
                                    return ExprKind::Error;
                                }
                            }
                        }
                        _ => parser_unreachable!(),
                    }
                }

                ExprKind::Lit(Lit::Bytes(out.into()))
            }
            ast::Expr::ParenExpr(paren_expr) => paren_expr
                .expr()
                .map(|expr| self.lower_expr_kind(expr))
                .unwrap_or(ExprKind::Error),
            ast::Expr::PrefixExpr(prefix_expr) => self.lower_prefix_expr(prefix_expr),
            ast::Expr::InfixExpr(infix_expr) => self.lower_infix_expr(infix_expr),
            ast::Expr::FieldAccess(field_access) => self.lower_field_access(field_access),
            ast::Expr::PeekExpr(peek_expr) => self.lower_peek_expr(peek_expr),
            ast::Expr::ConcatExpr(concat_expr) => self.lower_concat_expr(concat_expr),
        }
    }

    /// Lowers the given AST atom to IR.
    fn lower_atom(&mut self, atom: ast::Atom) -> ExprKind {
        if atom.child().is_none() {
            return ExprKind::Error;
        }
        let token = atom.child().parser_expect();
        let kind = atom.child_kind().parser_expect();

        fn int_text_and_multiplier(text: &str) -> (&str, u64) {
            if text.ends_with('B') {
                let i_present = text.ends_with("iB");
                let (text, suffix) = text.split_at(text.len() - if i_present { 3 } else { 2 });
                let base = if i_present { 1024u64 } else { 1000u64 };
                let multiplier = match suffix.chars().next().unwrap() {
                    'K' => base.pow(1),
                    'M' => base.pow(2),
                    'G' => base.pow(3),
                    'T' => base.pow(4),
                    'P' => base.pow(5),
                    'E' => base.pow(6),
                    _ => unreachable!(),
                };

                (text, multiplier)
            } else {
                (text, 1)
            }
        }

        match kind {
            TokenKind::BinaryIntegerLiteral
            | TokenKind::OctalIntegerLiteral
            | TokenKind::DecimalIntegerLiteral
            | TokenKind::HexadecimalIntegerLiteral => {
                let (base, prefix) = match kind {
                    TokenKind::BinaryIntegerLiteral => (2, "0b"),
                    TokenKind::OctalIntegerLiteral => (8, "0o"),
                    TokenKind::DecimalIntegerLiteral => (10, ""),
                    TokenKind::HexadecimalIntegerLiteral => (16, "0x"),
                    _ => unreachable!(),
                };
                let text = token.text().strip_prefix(prefix).parser_expect();
                let (text, multiplier) = int_text_and_multiplier(text);
                let int = int_from_str(base, text).parser_expect();
                ExprKind::Lit(Lit::Int(int * multiplier))
            }
            TokenKind::StringLiteral => {
                let text = token.text();
                // strip the leading and trailing `"` characters
                let content = &text[1..text.len() - 1];
                let mut bytes = Vec::new();

                if let Err((msg, _)) = str_lit_content_to_bytes(content, &mut bytes) {
                    self.add_diagnostic(Diagnostic::error(
                        "could not parse string literal",
                        Label::new(msg, atom.span()),
                    ));
                    return ExprKind::Error;
                }

                ExprKind::Lit(Lit::Bytes(bytes.into()))
            }
            TokenKind::TrueKw => ExprKind::Lit(Lit::Bool(true)),
            TokenKind::FalseKw => ExprKind::Lit(Lit::Bool(false)),
            TokenKind::Identifier => {
                self.add_diagnostic(
                    Diagnostic::error(
                        "bare variables are reserved syntax",
                        Label::new("bare variables are unsupported", atom.span()),
                    )
                    .with_help("try adding a leading `.`"),
                );
                ExprKind::Error
            }
            _ => parser_unreachable!(),
        }
    }

    /// Lowers the given AST prefix expression to IR.
    fn lower_prefix_expr(&mut self, expr: ast::PrefixExpr) -> ExprKind {
        let op = expr.op().parser_expect();
        let expr = required_field!(expr => expr ? self => ExprKind::Error);

        let op = match op.child_kind() {
            Some(TokenKind::Minus) => UnOp::Neg,
            Some(TokenKind::Plus) => UnOp::Plus,
            Some(TokenKind::ExclamationMark) => UnOp::Not,
            _ => parser_unreachable!(),
        };

        ExprKind::UnOp {
            op,
            operand: Box::new(self.lower_expr(expr)),
        }
    }

    /// Lowers the given AST infix expression to IR.
    fn lower_infix_expr(&mut self, expr: ast::InfixExpr) -> ExprKind {
        let op = expr.op().parser_expect();
        let lhs = expr.lhs().parser_expect();
        let rhs = required_field!(expr => rhs ? self => ExprKind::Error);

        let op = match &*op.text().to_string() {
            "+" => BinOp::Add,
            "-" => BinOp::Sub,
            "*" => BinOp::Mul,
            "/" => BinOp::Div,
            "%" => BinOp::Mod,
            "==" => BinOp::Eq,
            "!=" => BinOp::Neq,
            ">" => BinOp::Gt,
            ">=" => BinOp::Geq,
            "<" => BinOp::Lt,
            "<=" => BinOp::Leq,
            "&&" => BinOp::LogicalAnd,
            "||" => BinOp::LogicalOr,
            "&" => BinOp::BitAnd,
            "|" => BinOp::BitOr,
            "^" => BinOp::BitXor,
            "<<" => BinOp::ShiftLeft,
            ">>" => BinOp::ShiftRight,
            _ => parser_unreachable!(),
        };

        ExprKind::BinOp {
            op,
            lhs: Box::new(self.lower_expr(lhs)),
            rhs: Box::new(self.lower_expr(rhs)),
        }
    }

    /// Lowers the given AST field access expression to IR.
    fn lower_field_access(&mut self, field_access: ast::FieldAccess) -> ExprKind {
        fn lower_struct_ref(
            lowering_ctx: &mut LoweringCtx,
            parts: &mut Vec<Spanned<StructRefPart>>,
            expr: Option<ast::Expr>,
        ) -> Option<()> {
            let expr = match expr {
                Some(expr) => expr,
                None => return Some(()),
            };

            let span = expr.span();
            let mut err = || {
                lowering_ctx.add_diagnostic(Diagnostic::error(
                    "expected `$parent`, `$last` or another field access expression here",
                    Label::new("cannot access fields of this", span),
                ));
            };

            match expr {
                ast::Expr::Atom(atom) if atom.child_kind() == Some(TokenKind::Identifier) => {
                    lowering_ctx.lower_atom(atom);
                    return None;
                }
                ast::Expr::Metavar(metavar) => {
                    let name = required_field!(metavar => name ? lowering_ctx => None);
                    if name.text() == "parent" {
                        parts.push(Spanned {
                            inner: StructRefPart::Parent,
                            span,
                        });
                    } else if name.text() == "last" {
                        parts.push(Spanned {
                            inner: StructRefPart::Last,
                            span,
                        });
                    } else {
                        err();
                        return None;
                    }
                }
                ast::Expr::FieldAccess(field_access) => {
                    lower_struct_ref(lowering_ctx, parts, field_access.expr())?;
                    let field_raw =
                        required_field!(field_access => field_name ? lowering_ctx => None);
                    let span = field_raw.span();
                    let field = lowering_ctx.lower_field_name(field_raw)?;
                    if parts.is_empty() && !matches!(field, StructRefPart::Named(..)) {
                        lowering_ctx.add_diagnostic(
                            Diagnostic::error(
                                "only sibling field accesses are allowed after the initial `.`",
                                Label::new("illegal field access", field_access.span()),
                            )
                            .with_help("remove the leading `.`"),
                        );
                        return None;
                    }
                    parts.push(Spanned { inner: field, span });
                }
                _ => {
                    err();
                    return None;
                }
            }

            Some(())
        }

        let mut parts = Vec::new();
        let Some(()) = lower_struct_ref(self, &mut parts, field_access.expr()) else {
            return ExprKind::Error;
        };

        let field_raw = required_field!(field_access => field_name ? self => ExprKind::Error);
        let field_span = field_raw.span();
        let Some(field) = self.lower_field_name(field_raw) else {
            return ExprKind::Error;
        };
        let StructRefPart::Named(field) = field else {
            self.add_diagnostic(Diagnostic::error(
                "expected final field to be a bare field",
                Label::new("must be a bare field", field_span),
            ));
            return ExprKind::Error;
        };

        ExprKind::FieldAccess {
            struct_ref: StructRef { parts },
            field: Spanned {
                inner: field,
                span: field_span,
            },
        }
    }

    /// Lowers the given AST field name to IR.
    fn lower_field_name(&mut self, field_name: ast::FieldName) -> Option<StructRefPart> {
        match field_name {
            ast::FieldName::BareField(bare_field) => Some(StructRefPart::Named(Symbol::from(
                required_field!(bare_field => name ? self => None),
            ))),
            ast::FieldName::MetaField(meta_field) => {
                let name = required_field!(meta_field => name ? self => None);
                if name.text() == "parent" {
                    Some(StructRefPart::Parent)
                } else {
                    self.add_diagnostic(Diagnostic::error(
                        format!("`${}` cannot be used as a field", name.text()),
                        Label::new("invalid field", meta_field.span()),
                    ));
                    None
                }
            }
        }
    }

    /// Lowers the given AST `peek` expression to IR.
    fn lower_peek_expr(&mut self, peek_expr: ast::PeekExpr) -> ExprKind {
        let offset = peek_expr
            .offset()
            .map(|expr| Box::new(self.lower_expr(expr)));

        ExprKind::Peek {
            ty: Box::new(self.lower_parse_type(
                required_field!(peek_expr => parse_type ? self => ExprKind::Error),
                &None,
            )),
            offset,
        }
    }

    /// Lowers the given AST `concat` expression to IR.
    fn lower_concat_expr(&mut self, concat_expr: ast::ConcatExpr) -> ExprKind {
        let mut args = Vec::new();

        for arg in concat_expr.args() {
            let arg = match arg {
                ast::ConcatArg::ConcatArgDirect(concat_arg_direct) => {
                    let expr = required_field!(concat_arg_direct => expr ? self => ExprKind::Error);
                    ConcatArg::Direct(self.lower_expr(expr))
                }
                ast::ConcatArg::ConcatArgExpanding(concat_arg_expanding) => {
                    let expr =
                        required_field!(concat_arg_expanding => expr ? self => ExprKind::Error);
                    ConcatArg::Expanding(self.lower_expr(expr))
                }
            };

            args.push(arg);
        }

        ExprKind::Concat { args }
    }

    /// Lowers the given AST declaration to IR.
    fn lower_declaration(&mut self, declaration: ast::Declaration) -> Option<Declaration> {
        match declaration {
            ast::Declaration::EndiannessDeclaration(endianness_declaration) => {
                self.lower_endianness_declaration(endianness_declaration)
            }
            ast::Declaration::AlignDeclaration(align_declaration) => {
                self.lower_align_declaration(align_declaration)
            }
            ast::Declaration::SeekByDeclaration(seek_by) => self.lower_seek_by_declaration(seek_by),
            ast::Declaration::SeekToDeclaration(seek_to) => self.lower_seek_to_declaration(seek_to),
            ast::Declaration::ScopeAtDeclaration(scope_at) => {
                self.lower_scope_at_declaration(scope_at)
            }
            ast::Declaration::ScopeInDeclaration(scope_in) => {
                self.lower_scope_in_declaration(scope_in)
            }
            ast::Declaration::IfDeclaration(if_decl) => self.lower_if_declaration(if_decl),
            ast::Declaration::AssertDeclaration(assert) => self.lower_assert_declaration(assert),
            ast::Declaration::WarnIfDeclaration(warn_if) => self.lower_warn_if_declaration(warn_if),
            ast::Declaration::RecoveryDeclaration(recovery) => {
                self.lower_recovery_declaration(recovery)
            }
        }
    }

    /// Lowers the given AST endianness declaration to IR.
    fn lower_endianness_declaration(
        &mut self,
        endianness_declaration: ast::EndiannessDeclaration,
    ) -> Option<Declaration> {
        let token = required_field!(endianness_declaration => kind ? self => None);

        let endianness = match token.text() {
            "le" => Endianness::Little,
            "be" => Endianness::Big,
            _ => {
                self.add_diagnostic(Diagnostic::error(
                    "expected `be` or `le`",
                    Label::new("unexpected endianness value", endianness_declaration.span()),
                ));
                return None;
            }
        };

        Some(Declaration {
            kind: DeclarationKind::Endianness(endianness),
            span: endianness_declaration.span(),
        })
    }

    /// Lowers the given AST `align` declaration to IR.
    fn lower_align_declaration(
        &mut self,
        align_declaration: ast::AlignDeclaration,
    ) -> Option<Declaration> {
        Some(Declaration {
            kind: DeclarationKind::Align(
                self.lower_expr(required_field!(align_declaration => amount ? self => None)),
            ),
            span: align_declaration.span(),
        })
    }

    /// Lowers the given AST `seek by` declaration to IR.
    fn lower_seek_by_declaration(
        &mut self,
        seek_by: ast::SeekByDeclaration,
    ) -> Option<Declaration> {
        Some(Declaration {
            kind: DeclarationKind::SeekBy(
                self.lower_expr(required_field!(seek_by => amount ? self => None)),
            ),
            span: seek_by.span(),
        })
    }

    /// Lowers the given AST `seek to` declaration to IR.
    fn lower_seek_to_declaration(
        &mut self,
        seek_to: ast::SeekToDeclaration,
    ) -> Option<Declaration> {
        Some(Declaration {
            kind: DeclarationKind::SeekTo(
                self.lower_expr(required_field!(seek_to => amount ? self => None)),
            ),
            span: seek_to.span(),
        })
    }

    /// Lowers the given AST `scope at` declaration to IR.
    fn lower_scope_at_declaration(
        &mut self,
        scope_at: ast::ScopeAtDeclaration,
    ) -> Option<Declaration> {
        let start = self.lower_expr(required_field!(scope_at => start ? self => None));
        let end = scope_at.end().map(|expr| self.lower_expr(expr));

        Some(Declaration {
            kind: DeclarationKind::Scope {
                kind: ScopeKind::At { start, end },
                block: self.lower_block(required_field!(scope_at => struct_block ? self => None)),
            },
            span: scope_at.span(),
        })
    }

    /// Lowers the given AST `scope in` declaration to IR.
    fn lower_scope_in_declaration(
        &mut self,
        scope_in: ast::ScopeInDeclaration,
    ) -> Option<Declaration> {
        let bytes = self.lower_expr(required_field!(scope_in => bytes ? self => None));

        Some(Declaration {
            kind: DeclarationKind::Scope {
                kind: ScopeKind::In { bytes },
                block: self.lower_block(required_field!(scope_in => struct_block ? self => None)),
            },
            span: scope_in.span(),
        })
    }

    /// Lowers the given AST `if` declaration to IR.
    fn lower_if_declaration(&mut self, if_decl: ast::IfDeclaration) -> Option<Declaration> {
        Some(Declaration {
            kind: DeclarationKind::If(
                self.lower_if_chain(required_field!(if_decl => if_chain ? self => None))?,
            ),
            span: if_decl.span(),
        })
    }

    /// Lowers the given AST `if` chain to IR.
    fn lower_if_chain(&mut self, if_chain: ast::IfChain) -> Option<IfChain> {
        fn lower_into(
            this: &mut LoweringCtx,
            if_blocks: &mut Vec<IfBlock>,
            if_chain: ast::IfChain,
        ) -> Option<Option<Block>> {
            let condition = this.lower_expr(required_field!(if_chain => condition ? this => None));
            let block = this.lower_block(required_field!(if_chain => then_block ? this => None));

            if_blocks.push(IfBlock { condition, block });

            Some(match if_chain.else_part() {
                Some(else_part) => {
                    match else_part {
                        ast::ElsePart::IfChain(if_chain) => lower_into(this, if_blocks, if_chain)?,
                        ast::ElsePart::ElseBlock(else_block) => Some(this.lower_block(
                            required_field!(else_block => struct_block ? this => None),
                        )),
                    }
                }
                None => None,
            })
        }

        let mut if_blocks = Vec::new();
        let else_block = lower_into(self, &mut if_blocks, if_chain)?;

        Some(IfChain {
            if_blocks,
            else_block,
        })
    }

    /// Lowers the given AST `assert` declaration to IR.
    fn lower_assert_declaration(&mut self, assert: ast::AssertDeclaration) -> Option<Declaration> {
        Some(Declaration {
            kind: DeclarationKind::Assert {
                condition: self.lower_expr(required_field!(assert => expr ? self => None)),
                message: assert.message().map(|expr| self.lower_expr(expr)),
            },
            span: assert.span(),
        })
    }

    /// Lowers the given AST `warn if` declaration to IR.
    fn lower_warn_if_declaration(
        &mut self,
        warn_if: ast::WarnIfDeclaration,
    ) -> Option<Declaration> {
        Some(Declaration {
            kind: DeclarationKind::WarnIf {
                condition: self.lower_expr(required_field!(warn_if => expr ? self => None)),
                message: warn_if.message().map(|expr| self.lower_expr(expr)),
            },
            span: warn_if.span(),
        })
    }

    /// Lowers the given AST `recover` declaration to IR.
    fn lower_recovery_declaration(
        &mut self,
        recovery: ast::RecoveryDeclaration,
    ) -> Option<Declaration> {
        Some(Declaration {
            kind: DeclarationKind::Recover {
                at: self.lower_expr(required_field!(recovery => expr ? self => None)),
            },
            span: recovery.span(),
        })
    }

    /// Lowers the given AST `let` statement to IR.
    fn lower_let_statement(&mut self, let_statement: ast::LetStatement) -> Option<LetStatement> {
        Some(LetStatement {
            name: Spanned::<Symbol>::from(required_field!(let_statement => name ? self => None)),
            expr: self.lower_expr(required_field!(let_statement => expr ? self => None)),
        })
    }
}

/// An extension trait to unwrap with a message that a situation should be impossible because of
/// the parser.
trait ParserImpossible {
    /// The type that is unwrapped to.
    type Target;

    /// Unwraps a value with a message telling that the parser should reject invalid values here.
    fn parser_expect(self) -> Self::Target;
}

impl<T> ParserImpossible for Option<T> {
    type Target = T;

    #[track_caller]
    fn parser_expect(self) -> Self::Target {
        self.expect("this should be rejected by the parser")
    }
}
