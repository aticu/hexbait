//! Performs static analysis on the IR to ensure that the input is well formed.

use crate::compile::{
    Diagnostic, Diagnostics, Span,
    diagnostics::Label,
    ir::{
        LocalStructId, ReplStatement, Struct,
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
    let mut struct_states = Vec::from_iter(
        (0..file.structs.len()).map(|i| StructLoweringState::NotStarted(&file.structs[i])),
    );

    let mut ctx = AnalysisCtx::new(diagnostics, &mut struct_states);

    let mut i = 0;
    while i < ctx.struct_states.len() {
        ctx.ensure_lowered(LocalStructId { idx: i });

        i += 1;
    }
}

/// Checks if a REPL statement is well formed.
pub fn check_repl_statement(repl_statement: &ReplStatement, diagnostics: &mut Diagnostics) {
    let mut struct_states = Vec::from_iter(
        (0..repl_statement.structs.len())
            .map(|i| StructLoweringState::NotStarted(&repl_statement.structs[i])),
    );

    let mut ctx = AnalysisCtx::new(diagnostics, &mut struct_states);
    let env = Env {
        parent: None,
        values: StructTy::empty(),
        // the endianness in expression context comes directly from the GUI
        endianness_state: EndiannessState::Set,
    };

    env.check_expr(&mut ctx, repl_statement.expr(), &ExtraExprCtx::none());
}

/// The different lowering states a struct can be in.
enum StructLoweringState<'ir> {
    /// Lowering of the struct was not started.
    NotStarted(&'ir Struct),
    /// Lowering was started but is not yet finished.
    Started,
    /// Lowering was finished.
    Finished {
        /// The type of the struct.
        ty: StructTy,
    },
}

/// The analysis context.
struct AnalysisCtx<'ctx> {
    /// The diagnostics.
    diagnostics: &'ctx mut Diagnostics,
    /// The lowering states of all structs.
    struct_states: &'ctx mut Vec<StructLoweringState<'ctx>>,
}

impl<'ctx> AnalysisCtx<'ctx> {
    /// Creates a new analysis context.
    fn new(
        diagnostics: &'ctx mut Diagnostics,
        struct_states: &'ctx mut Vec<StructLoweringState<'ctx>>,
    ) -> AnalysisCtx<'ctx> {
        AnalysisCtx {
            diagnostics,
            struct_states,
        }
    }

    /// Adds a diagnostic to the analysis context.
    fn add_diagnostic(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.add_diagnostic(diagnostic);
    }

    /// Ensures that the struct with the given struct id is lowered.
    fn ensure_lowered(&mut self, id: LocalStructId) -> &StructTy {
        let idx = id.idx;

        match &self.struct_states[idx] {
            StructLoweringState::NotStarted(_) => {
                let StructLoweringState::NotStarted(r#struct) =
                    std::mem::replace(&mut self.struct_states[idx], StructLoweringState::Started)
                else {
                    unreachable!()
                };

                let mut env = Env::new_for_struct();
                for content in &r#struct.content {
                    env.check_struct_content(self, content);
                }
                self.struct_states[idx] = StructLoweringState::Finished { ty: env.values };
            }
            StructLoweringState::Started => unreachable!(),
            StructLoweringState::Finished { .. } => (),
        }

        match &self.struct_states[idx] {
            StructLoweringState::Finished { ty } => ty,
            _ => unreachable!(),
        }
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
    /// Creates a new environment for a struct.
    fn new_for_struct() -> Env<'parent> {
        Env {
            parent: None,
            values: StructTy::empty(),
            endianness_state: EndiannessState::Undefined,
        }
    }

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
        /// Whether the unset branch is a missing else branch.
        unset_is_missing_else: bool,
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
                unset_branch_span,
                unset_is_missing_else,
                ..
            } => Some(Label::new(
                if *unset_is_missing_else {
                    "endianness is not set if this is `false`"
                } else {
                    "endianness is not set in this branch"
                },
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
