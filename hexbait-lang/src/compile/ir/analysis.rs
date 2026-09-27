//! Performs static analysis on the IR to ensure that the input is well formed.

use crate::compile::{
    Diagnostic, Diagnostics, Span,
    diagnostics::Label,
    ir::{
        Expr,
        analysis::{
            expr::ExtraExprCtx,
            ty::{StructTy, Ty, TyKind},
        },
    },
};

use super::File;

mod decl;
mod expr;
mod parse_ty;
mod struct_content;
mod ty;

// TODO: ensure that $last is only used if $len > 0
// TODO: ensure that loops must make progress
// TODO: ensure that there is no scope between a `recover` and the struct it references
// TODO: properly check that `_`-prefixed fields don't escape
// TODO: ensure that scopes, nested structs and endianness interact correctly with each other

/// Checks if the file is well formed.
pub fn check_file(file: &File, diagnostics: &mut Diagnostics) {
    let mut ctx = AnalysisCtx::new(diagnostics);
    let mut env = Env {
        parent: None,
        values: StructTy::empty(),
        endianness_state: EndiannessState::Undefined,
    };

    for content in &file.content {
        env.check_struct_content(&mut ctx, content);
    }
}

/// Checks if an expression is well formed.
pub fn check_expr(expr: &Expr, diagnostics: &mut Diagnostics) {
    let mut ctx = AnalysisCtx::new(diagnostics);
    let env = Env {
        parent: None,
        values: StructTy::empty(),
        // the endianness in expression context comes directly from the GUI
        endianness_state: EndiannessState::Set,
    };

    env.check_expr(&mut ctx, expr, &ExtraExprCtx::none());
}

/// The analysis context.
struct AnalysisCtx<'ctx> {
    /// The diagnostics.
    diagnostics: &'ctx mut Diagnostics,
}

impl AnalysisCtx<'_> {
    /// Creates a new analysis context.
    fn new(diagnostics: &mut Diagnostics) -> AnalysisCtx<'_> {
        AnalysisCtx { diagnostics }
    }

    /// Adds a diagnostic to the analysis context.
    fn add_diagnostic(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.add_diagnostic(diagnostic);
    }

    /// Adds a type error to the context.
    #[track_caller]
    fn ty_err(&mut self, message: impl ToString, ty: &Ty, span: Span) -> TyKind {
        if ty.contains_err() {
            assert!(self.diagnostics.contains_errors());
            return TyKind::Error;
        }

        if let TyKind::Indeterminate {
            first_span,
            second_span,
        } = &ty.kind
        {
            self.add_diagnostic(
                Diagnostic::error(
                    "type differs between braches",
                    Label::new("undetermined type", span),
                )
                .with_label(Label::new("first branch here", *first_span))
                .with_label(Label::new("second branch here", *second_span)),
            );
        } else {
            self.add_diagnostic(Diagnostic::error(
                message,
                Label::new(format!("unexpected type `{}`", ty.name()), span),
            ));
        }

        TyKind::Error
    }
}

/// The environment for checking IR.
struct Env<'parent> {
    /// The parent environment.
    parent: Option<&'parent Env<'parent>>,
    /// The currently available values.
    values: StructTy,
    /// The current state of the endianness.
    endianness_state: EndiannessState,
}

impl<'parent> Env<'parent> {
    /// Creates a child environment.
    fn child(&'parent self) -> Env<'parent> {
        Env {
            parent: Some(self),
            values: StructTy::empty(),
            endianness_state: self.endianness_state,
        }
    }

    /// Creates a clone context for a branch in the same context.
    ///
    /// This is meant to later be merged back into the original context.
    fn create_branch_clone(&'parent self) -> Env<'parent> {
        Env {
            parent: self.parent,
            values: self.values.clone(),
            endianness_state: self.endianness_state,
        }
    }
}

/// The current state of endianness in the environment.
#[derive(Debug, Clone, Copy)]
enum EndiannessState {
    /// The endianness was not yet defined.
    Undefined,
    /// The endianness is set at this point.
    Set,
    /// The endianness was partially set.
    PartiallySet {
        /// The span of the first branch where the endianness was set.
        set_branch_span: Span,
        /// The span of the first branch where the endianness was not set.
        unset_branch_span: Span,
    },
}

impl EndiannessState {
    /// Whether the endianness is definitely set.
    fn is_set(&self) -> bool {
        match self {
            EndiannessState::Set => true,
            EndiannessState::Undefined | EndiannessState::PartiallySet { .. } => false,
        }
    }

    /// A label to report an unset endianness.
    ///
    /// This function may return `None`, even if the endianness is unset.
    fn unset_label(&self) -> Option<Label> {
        match self {
            EndiannessState::Undefined | EndiannessState::Set => None,
            EndiannessState::PartiallySet {
                unset_branch_span, ..
            } => Some(Label::new(
                "endianness is not set in this branch",
                *unset_branch_span,
            )),
        }
    }

    /// A label to report an where endianness was set in a partially set endianness state.
    fn set_label(&self) -> Option<Label> {
        match self {
            EndiannessState::Undefined | EndiannessState::Set => None,
            EndiannessState::PartiallySet {
                set_branch_span, ..
            } => Some(Label::new(
                "endianness is set in this branch",
                *set_branch_span,
            )),
        }
    }
}
