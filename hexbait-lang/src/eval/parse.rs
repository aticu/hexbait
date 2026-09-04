//! Implements the parsing evaluation logic.

use std::fmt;

use crate::{
    compile::ir::{ElsePart, Expr, File, IfChain, LetStatement, StructContent, StructField},
    eval::parse::{
        cursor::Cursor,
        diagnostics::{Diagnostics, Result},
        struct_context::StructContext,
    },
};

use super::{value::Value, view::View};

use hexbait_common::{Endianness, RelativeOffset};

pub use diagnostics::{Diagnostic, DiagnosticId, DiagnosticLevel};

mod cursor;
mod decl;
mod diagnostics;
mod expr;
mod parse_ty;
mod struct_context;

/// The result of parsing.
#[derive(Debug, Clone)]
pub struct ParseResult {
    /// The parsed value.
    pub value: Value,
    /// The diagnostics that occurred during parsing.
    pub diagnostics: Vec<Diagnostic>,
}

/// Evaluates the given IR on the given input.
pub fn eval_ir(file: &File, view: View, start_offset: RelativeOffset) -> ParseResult {
    let mut struct_ctx = StructContext::new();
    // the start offset should always be valid
    // static analysis makes sure that the endianness is set to the correct value before parsing
    let mut cursor = Cursor::new(view, start_offset, Endianness::Little).static_analysis_expect();

    let mut parse_ctx = ParseContext {
        diagnostics: Diagnostics::new(),
    };

    parse_ctx
        .eval_struct_content(&file.content, &mut cursor, &mut struct_ctx)
        .ok();

    ParseResult {
        value: struct_ctx.into_value(),
        diagnostics: parse_ctx.diagnostics.into_diagnostics(),
    }
}

/// Evaluates the given IR expression on the given view.
pub fn eval_expr(expr: &Expr, endianness: Endianness, view: View) -> ParseResult {
    let struct_ctx = StructContext::new();
    let cursor = Cursor::new(view, RelativeOffset::ZERO, endianness).static_analysis_expect();

    let mut parse_ctx = ParseContext {
        diagnostics: Diagnostics::new(),
    };

    match parse_ctx.eval_expr(expr, &cursor, &struct_ctx, Default::default()) {
        Ok(value) => ParseResult {
            value,
            diagnostics: parse_ctx.diagnostics.into_diagnostics(),
        },
        Err(_) => todo!(),
    }
}

/// The context used during parsing.
#[derive(Debug)]
struct ParseContext {
    /// Stores the diagnostics during parsing.
    diagnostics: Diagnostics,
}

impl ParseContext {
    /// Evaluates the given `if` chain.
    fn eval_if_chain(
        &mut self,
        if_chain: &IfChain,
        cursor: &mut Cursor,
        struct_ctx: &mut StructContext,
    ) -> Result<()> {
        let condition =
            self.eval_expr(&if_chain.condition, cursor, struct_ctx, Default::default())?;

        if condition.kind.expect_bool() {
            for single_content in &if_chain.then_block {
                self.eval_single_struct_content(single_content, cursor, struct_ctx)?;
            }
        } else if let Some(else_part) = &if_chain.else_part {
            match else_part {
                ElsePart::IfChain(if_chain) => {
                    self.eval_if_chain(if_chain, cursor, struct_ctx)?;
                }
                ElsePart::ElseBlock(else_block) => {
                    for single_content in else_block {
                        self.eval_single_struct_content(single_content, cursor, struct_ctx)?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Evaluates the given `struct` field.
    fn eval_struct_field(
        &mut self,
        field: &StructField,
        cursor: &mut Cursor,
        struct_ctx: &mut StructContext,
    ) -> Result<()> {
        let value = self.eval_parse_type(&field.ty, cursor, struct_ctx)?;

        if let Some(expected) = &field.expected {
            let span = expected.span;
            let expected = self.eval_expr(expected, cursor, struct_ctx, Default::default())?;
            if expected != value {
                return Err(self
                    .diagnostics
                    .new_err(
                        format!(
                            "field expectation failed: {:?} != {:?}",
                            expected.kind, value.kind
                        ),
                        &value.provenance + &expected.provenance,
                        span,
                    )
                    .with_partial_result(value));
            }
        }

        struct_ctx.insert(field.name.inner.clone(), value);

        Ok(())
    }

    /// Evaluates the given `let` statement.
    fn eval_let_statement(
        &mut self,
        let_statement: &LetStatement,
        cursor: &mut Cursor,
        struct_ctx: &mut StructContext,
    ) -> Result<()> {
        let value = self.eval_expr(&let_statement.expr, cursor, struct_ctx, Default::default())?;

        struct_ctx.insert(let_statement.name.inner.clone(), value);

        Ok(())
    }

    /// Evaluates the given single `struct` content.
    fn eval_single_struct_content(
        &mut self,
        content: &StructContent,
        cursor: &mut Cursor,
        struct_ctx: &mut StructContext,
    ) -> Result<()> {
        match content {
            StructContent::Field(field) => {
                match self.eval_struct_field(field, cursor, struct_ctx) {
                    Ok(()) => Ok(()),
                    Err(mut err) => {
                        if let Some(partial_result) = err.take_partial_result() {
                            struct_ctx.insert(field.name.inner.clone(), partial_result);
                        }
                        Err(err)
                    }
                }
            }
            StructContent::Declaration(declaration) => {
                Ok(self.eval_declaration(declaration, cursor, struct_ctx)?)
            }
            StructContent::LetStatement(let_statement) => {
                Ok(self.eval_let_statement(let_statement, cursor, struct_ctx)?)
            }
            StructContent::Error => static_analysis_impossible(),
        }
    }

    /// Evaluates the content of a `struct`.
    fn eval_struct_content(
        &mut self,
        content: &[StructContent],
        cursor: &mut Cursor,
        struct_ctx: &mut StructContext,
    ) -> Result<()> {
        for content in content {
            if let Err(err) = self.eval_single_struct_content(content, cursor, struct_ctx) {
                return struct_ctx.recover(cursor, err);
            }
        }

        Ok(())
    }
}

/// Indicates that something is impossible because of static analysis.
#[track_caller]
fn static_analysis_impossible() -> ! {
    unreachable!("impossible because of static analysis")
}

/// An extension trait to unwrap with a message that a situation should be impossible because of
/// static analysis
trait StaticAnalysisImpossible {
    /// The type that is unwrapped to.
    type Target;

    /// Unwraps a value with a message telling that the value must exist because of static
    /// analysis.
    fn static_analysis_expect(self) -> Self::Target;
}

impl<T> StaticAnalysisImpossible for Option<T> {
    type Target = T;

    #[track_caller]
    fn static_analysis_expect(self) -> Self::Target {
        self.expect("impossible because of static analysis")
    }
}

impl<T, E: fmt::Debug> StaticAnalysisImpossible for Result<T, E> {
    type Target = T;

    #[track_caller]
    fn static_analysis_expect(self) -> Self::Target {
        self.expect("impossible because of static analysis")
    }
}
