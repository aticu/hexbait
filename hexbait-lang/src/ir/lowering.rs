//! Implements lowering the AST to the IR.

use crate::{
    Int,
    ast::{self, AstNode as _},
    int_from_str,
    lexer::TokenKind,
    span::Span,
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
pub fn lower_file(file: ast::File) -> File {
    let mut ctx = LoweringCtx::new();
    let mut out = Vec::new();

    for content in file.struct_content() {
        out.push(ctx.lower_struct_content(content));
    }

    File { content: out }
}

/// The context in which lowering is performed.
struct LoweringCtx {}

/// Accesses a required field in the given value.
///
/// Logs an error with the given message and returns the error type if the field is not present.
macro_rules! required_field {
    ($value:expr => $field:ident ? $this:ident: $message:expr => $err_ty:expr) => {
        match $value.$field() {
            Some(val) => val,
            None => {
                $this.error($message, $value.span());
                return $err_ty;
            }
        }
    };
}

impl LoweringCtx {
    /// Creates a new lowering context.
    fn new() -> LoweringCtx {
        LoweringCtx {}
    }

    /// Shows the given error message for the given span.
    fn error(&mut self, message: impl Into<String>, span: Span) {
        eprintln!(
            "TODO: improve error handling: {} at {span:?}",
            message.into()
        );
    }

    /// Lowers the given `struct` content AST to IR.
    fn lower_struct_content(&mut self, struct_content: ast::StructContent) -> StructContent {
        match struct_content {
            ast::StructContent::Declaration(declaration) => self
                .lower_declaration(declaration)
                .map(StructContent::Declaration),
            ast::StructContent::StructField(struct_field) => self
                .lower_struct_field(struct_field)
                .map(StructContent::Field),
            ast::StructContent::Struct(_) => todo!(),
            ast::StructContent::LetStatement(let_statement) => self
                .lower_let_statement(let_statement)
                .map(StructContent::LetStatement),
        }
        .unwrap_or(StructContent::Error)
    }

    /// Lowers the given AST `struct` field to IR.
    fn lower_struct_field(&mut self, struct_field: ast::StructField) -> Option<StructField> {
        let expected = struct_field
            .expected()
            .map(|expected| self.lower_expr(expected));

        Some(StructField {
            name: Spanned::<Symbol>::from(
                required_field!(struct_field => name ? self: "expected name for `struct` field" => None),
            ),
            ty: self.lower_parse_type(
                required_field!(struct_field => parse_type ? self: "expected parse type for `struct` field" => None),
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
        match parse_type {
            ast::ParseType::NamedParseType(named_parse_type) => {
                let name_token = required_field!(named_parse_type => name ? self: "expected parse type" => ParseType::Error);

                let name = name_token.text();
                if (name.starts_with("i") || name.starts_with("u"))
                    && let Ok(num_bits) = name[1..].parse::<u32>()
                {
                    ParseType::Integer {
                        bit_width: num_bits,
                        signed: name.starts_with("i"),
                    }
                } else {
                    ParseType::Named {
                        name: Spanned::<Symbol>::from(name_token),
                    }
                }
            }
            ast::ParseType::BytesParseType(bytes_parse_type) => {
                let repetition_kind = if let Some(repeat_decl) = bytes_parse_type.repeat_decl() {
                    self.lower_repetition(repeat_decl)
                } else {
                    let expected = expected.as_ref().parser_expect();
                    let ExprKind::Lit(Lit::Bytes(bytes)) = &expected.kind else {
                        todo!()
                    };
                    RepeatKind::Len {
                        count: Expr {
                            kind: ExprKind::Lit(Lit::Int(Int::from(bytes.len()))),
                            span: expected.span
                        }
                    }
                };

                ParseType::Bytes { repetition_kind }
            }
            ast::ParseType::RepeatParseType(repeat_parse_type) => {
                ParseType::Repeating {
                    parse_type: Box::new(self.lower_parse_type(
                        required_field!(repeat_parse_type => ty ? self: "expected parse type" => ParseType::Error),
                        &None,
                    )),
                    repetition_kind: self.lower_repetition(
                        required_field!(repeat_parse_type => repetition ? self: "expected repetition type" => ParseType::Error)
                    ),
                }
            }
            ast::ParseType::AnonymousStructParseType(struct_parse_type) => {
                let content = struct_parse_type.struct_content().map(|content| self.lower_struct_content(content)).collect();

                ParseType::Struct {
                    content,
                }
            }
            ast::ParseType::SwitchParseType(switch_parse_type) => {
                let scrutinee = self.lower_expr(
                    required_field!(switch_parse_type => scrutinee ? self: "expected `switch` scrutinee" => ParseType::Error)
                );

                let mut branches = Vec::new();

                for arm in switch_parse_type.switch_parse_type_arm() {
                    let value = self.lower_expr(
                        required_field!(arm => val ? self: "expected arm value" => ParseType::Error)
                    );
                    let parse_ty = self.lower_parse_type(
                        required_field!(arm => parse_type ? self: "expected arm parse type" => ParseType::Error),
                        &None,
                    );

                    if let ExprKind::Lit(lit) = value.kind {
                        branches.push((lit, parse_ty));
                    } else {
                        self.error("expected literal", value.span);
                    }
                }

                let default = Box::new(self.lower_parse_type(
                    required_field!(switch_parse_type => default ? self: "expected `switch` default branch" => ParseType::Error),
                    &None
                ));

                ParseType::Switch { scrutinee, branches, default }
            }
        }
    }

    /// Lowers the given AST repetition to IR.
    fn lower_repetition(&mut self, repetition: ast::RepeatDecl) -> RepeatKind {
        match repetition {
            ast::RepeatDecl::RepeatLenDecl(repeat_len_decl) => {
                RepeatKind::Len {
                    count: self.lower_expr(
                        required_field!(repeat_len_decl => count ? self: "expected length expression" => RepeatKind::Error)
                    ),
                }
            }
            ast::RepeatDecl::RepeatWhileDecl(repeat_while_decl) => {
                RepeatKind::While {
                    condition: self.lower_expr(
                        required_field!(repeat_while_decl => condition ? self: "expected length expression" => RepeatKind::Error)
                    ),
                }
            }
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
                let name = required_field!(metavar => name ? self: "expected variable name" => ExprKind::Error);
                match name.text() {
                    "offset" => ExprKind::Offset,
                    "parent" => ExprKind::Parent,
                    "last" => ExprKind::Last,
                    "len" => ExprKind::Len,
                    var => {
                        self.error(format!("unknown metavariable: {var}"), metavar.span());
                        ExprKind::Error
                    }
                }
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
                                self.error(msg, Span::from(part.text_range()));
                                return ExprKind::Error;
                            }
                        }
                        TokenKind::ByteLiteral
                        | TokenKind::DecimalIntegerLiteral
                        | TokenKind::Identifier => {
                            let text = part.text();
                            if text.len() != 2 {
                                self.error(
                                    "expected hex byte literal to be of length two",
                                    Span::from(part.text_range()),
                                );
                                return ExprKind::Error;
                            }

                            let to_val = |c: char| {
                                c.to_digit(16)
                                    .map(|val| {
                                        u8::try_from(val)
                                            .expect("a single hex digit cannot exceed a u8")
                                    })
                                    .parser_expect()
                            };

                            let mut iter = text.chars();
                            let most_significant_nibble = to_val(iter.next().parser_expect());
                            let least_significant_nibble = to_val(iter.next().parser_expect());

                            out.push(most_significant_nibble << 4 | least_significant_nibble);
                        }
                        _ => parser_unreachable!(),
                    }
                }

                ExprKind::Lit(Lit::Bytes(out))
            }
            ast::Expr::ParenExpr(paren_expr) => paren_expr
                .expr()
                .map(|expr| self.lower_expr_kind(expr))
                .unwrap_or(ExprKind::Error),
            ast::Expr::PrefixExpr(prefix_expr) => self.lower_prefix_expr(prefix_expr),
            ast::Expr::InfixExpr(infix_expr) => self.lower_infix_expr(infix_expr),
            ast::Expr::FieldAccess(field_access) => self.lower_field_access(field_access),
            ast::Expr::PeekExpr(peek_expr) => self.lower_peek_expr(peek_expr),
        }
    }

    /// Lowers the given AST atom to IR.
    fn lower_atom(&mut self, atom: ast::Atom) -> ExprKind {
        let token = atom.child().parser_expect();
        let kind = atom.child_kind().parser_expect();

        match kind {
            TokenKind::BinaryIntegerLiteral => {
                let text = token.text().strip_prefix("0b").parser_expect();
                let int = int_from_str(2, text).parser_expect();
                ExprKind::Lit(Lit::Int(int))
            }
            TokenKind::OctalIntegerLiteral => {
                let text = token.text().strip_prefix("0o").parser_expect();
                let int = int_from_str(8, text).parser_expect();
                ExprKind::Lit(Lit::Int(int))
            }
            TokenKind::DecimalIntegerLiteral => {
                let int = int_from_str(10, token.text()).parser_expect();
                ExprKind::Lit(Lit::Int(int))
            }
            TokenKind::HexadecimalIntegerLiteral => {
                let text = token.text().strip_prefix("0x").parser_expect();
                let int = int_from_str(16, text).parser_expect();
                ExprKind::Lit(Lit::Int(int))
            }
            TokenKind::StringLiteral => {
                let text = token.text();
                // strip the leading and trailing `"` characters
                let content = &text[1..text.len() - 1];
                let mut bytes = Vec::new();

                if let Err((msg, _)) = str_lit_content_to_bytes(content, &mut bytes) {
                    self.error(msg, atom.span());
                    return ExprKind::Error;
                }

                ExprKind::Lit(Lit::Bytes(bytes))
            }
            TokenKind::TrueKw => ExprKind::Lit(Lit::Bool(true)),
            TokenKind::FalseKw => ExprKind::Lit(Lit::Bool(false)),
            TokenKind::Identifier => ExprKind::VarUse(Spanned::<Symbol>::from(token)),
            _ => parser_unreachable!(),
        }
    }

    /// Lowers the given AST prefix expression to IR.
    fn lower_prefix_expr(&mut self, expr: ast::PrefixExpr) -> ExprKind {
        let op = expr.op().parser_expect();
        let expr = required_field!(expr => expr ? self: "expected expression after prefix operator" => ExprKind::Error);

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
        let rhs = required_field!(expr => rhs ? self: "expected right hand side of expression" => ExprKind::Error);

        let op = match &*op.text().to_string() {
            "+" => BinOp::Add,
            "-" => BinOp::Sub,
            "*" => BinOp::Mul,
            "/" => BinOp::Div,
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
        let expr = field_access.expr().parser_expect();
        let field = Spanned::<Symbol>::from(
            required_field!(field_access => field ? self: "expected field name" => ExprKind::Error),
        );

        ExprKind::FieldAccess {
            expr: Box::new(self.lower_expr(expr)),
            field,
        }
    }

    /// Lowers the given AST `peek` expression to IR.
    fn lower_peek_expr(&mut self, peek_expr: ast::PeekExpr) -> ExprKind {
        let offset = peek_expr
            .offset()
            .map(|expr| Box::new(self.lower_expr(expr)));

        ExprKind::Peek {
            ty: Box::new(self.lower_parse_type(
                required_field!(peek_expr => parse_type ? self: "expected parse type" => ExprKind::Error),
                &None,
            )),
            offset
        }
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
            ast::Declaration::AssertDeclaration(assert) => self.lower_assert_declaration(assert),
            ast::Declaration::WarnIfDeclaration(warn_if) => self.lower_warn_if_declaration(warn_if),
        }
    }

    /// Lowers the given AST endianness declaration to IR.
    fn lower_endianness_declaration(
        &mut self,
        endianness_declaration: ast::EndiannessDeclaration,
    ) -> Option<Declaration> {
        let token =
            required_field!(endianness_declaration => kind ? self: "expected `be` or `le`" => None);

        let endianness = match token.text() {
            "le" => Endianness::Little,
            "be" => Endianness::Big,
            _ => {
                self.error("expected `be` or `le`", endianness_declaration.span());
                return None;
            }
        };

        Some(Declaration::Endianness(endianness))
    }

    /// Lowers the given AST `align` declaration to IR.
    fn lower_align_declaration(
        &mut self,
        align_declaration: ast::AlignDeclaration,
    ) -> Option<Declaration> {
        Some(Declaration::Align(self.lower_expr(
            required_field!(align_declaration => amount ? self: "expected alignment amount" => None)
        )))
    }

    /// Lowers the given AST `seek by` declaration to IR.
    fn lower_seek_by_declaration(
        &mut self,
        seek_by: ast::SeekByDeclaration,
    ) -> Option<Declaration> {
        Some(Declaration::SeekBy(self.lower_expr(
            required_field!(seek_by => amount ? self: "expected seek amount" => None),
        )))
    }

    /// Lowers the given AST `seek to` declaration to IR.
    fn lower_seek_to_declaration(
        &mut self,
        seek_to: ast::SeekToDeclaration,
    ) -> Option<Declaration> {
        Some(Declaration::SeekTo(self.lower_expr(
            required_field!(seek_to => amount ? self: "expected seek target offset" => None),
        )))
    }

    /// Lowers the given AST `scope at` declaration to IR.
    fn lower_scope_at_declaration(
        &mut self,
        scope_at: ast::ScopeAtDeclaration,
    ) -> Option<Declaration> {
        let start = self.lower_expr(
            required_field!(scope_at => start ? self: "expected scope start offset" => None),
        );
        let end = scope_at.end().map(|expr| self.lower_expr(expr));
        let mut content = Vec::new();

        for single_content in scope_at.struct_content() {
            content.push(self.lower_struct_content(single_content));
        }

        Some(Declaration::ScopeAt {
            start,
            end,
            content,
        })
    }

    /// Lowers the given AST `assert` declaration to IR.
    fn lower_assert_declaration(&mut self, assert: ast::AssertDeclaration) -> Option<Declaration> {
        Some(Declaration::Assert {
            condition: self
                .lower_expr(required_field!(assert => expr ? self: "expected assertion" => None)),
            message: assert.message().map(|expr| self.lower_expr(expr)),
        })
    }

    /// Lowers the given AST `warn if` declaration to IR.
    fn lower_warn_if_declaration(
        &mut self,
        warn_if: ast::WarnIfDeclaration,
    ) -> Option<Declaration> {
        Some(Declaration::WarnIf {
            condition: self
                .lower_expr(required_field!(warn_if => expr ? self: "expected expression" => None)),
            message: warn_if.message().map(|expr| self.lower_expr(expr)),
        })
    }

    /// Lowers the given AST `let` statement to IR.
    fn lower_let_statement(&mut self, let_statement: ast::LetStatement) -> Option<LetStatement> {
        Some(LetStatement {
            name: Spanned::<Symbol>::from(
                required_field!(let_statement => name ? self: "expected name" => None),
            ),
            expr: self.lower_expr(
                required_field!(let_statement => expr ? self: "expected expression" => None),
            ),
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
