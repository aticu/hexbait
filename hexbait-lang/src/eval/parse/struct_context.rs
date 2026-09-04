//! Implements the `struct` context needed for parsing.

use hexbait_common::RelativeOffset;

use crate::{
    compile::ir::{self, Symbol},
    eval::{
        DiagnosticId, Provenance, Value, ValueKind,
        parse::{
            StaticAnalysisImpossible as _,
            cursor::Cursor,
            diagnostics::{ParseErr, Result},
            static_analysis_impossible,
        },
        value::StructContent,
    },
};

/// The parsing context for a `struct`.
#[derive(Debug)]
pub struct StructContext<'parent> {
    /// The already parsed content.
    content: Vec<StructContent>,
    /// The parent `struct`.
    parent: Option<&'parent StructContext<'parent>>,
    /// The recovery strategy to use if parsing fails.
    recovery_strategy: RecoveryStrategy,
}

impl<'parent> StructContext<'parent> {
    /// Creates a new `struct` parsing context.
    pub fn new() -> StructContext<'static> {
        StructContext {
            content: Vec::new(),
            parent: None,
            recovery_strategy: RecoveryStrategy::Fallback,
        }
    }

    /// Creates the context for a child `struct`.
    pub fn child<'this>(&'this self) -> StructContext<'this> {
        StructContext {
            content: Vec::new(),
            parent: Some(self),
            recovery_strategy: RecoveryStrategy::Fallback,
        }
    }

    /// Returns the field named `field_name`.
    pub fn field(&self, field_name: &Symbol) -> Option<&Value> {
        self.content
            .iter()
            .find_map(|content| content.val_if_name_eq(field_name))
    }

    /// Inserts a field with the given name into the `struct`.
    pub fn insert(&mut self, field_name: Symbol, value: Value) {
        // TODO: use resolved names here later
        self.content.push(StructContent::Field {
            name: field_name,
            value,
        });
    }

    /// Pushes the given diagnostic into the `struct`.
    pub fn push_diagnostic(&mut self, diagnostic: DiagnosticId) {
        self.content.push(StructContent::Diagnostic(diagnostic));
    }

    /// Sets the recovery strategy of this `struct`.
    pub fn set_recovery_strategy(&mut self, recovery_strategy: RecoveryStrategy) {
        self.recovery_strategy = recovery_strategy;
    }

    /// Returns the parent context.
    pub fn parent(&self) -> Option<&'parent StructContext<'parent>> {
        self.parent
    }

    /// Recovers this `struct` from the given error.
    pub fn recover(&mut self, cursor: &mut Cursor, err: ParseErr) -> Result<()> {
        self.push_diagnostic(err.id());

        match &self.recovery_strategy {
            RecoveryStrategy::Fallback => Err(err),
            RecoveryStrategy::SkipTo { offset } => {
                // the offset should be checked at the time of use
                cursor.set_offset(*offset).static_analysis_expect();

                Ok(())
            }
        }
    }

    /// The provenance of the struct.
    fn provenance(&self) -> Provenance {
        let mut provenance = Provenance::empty();
        for content in &self.content {
            provenance += match content {
                StructContent::Field { name: _, value } => &value.provenance,
                StructContent::Diagnostic(_) => continue,
            }
        }

        provenance
    }

    /// Returns the `struct` context as a partially parsed `struct` value.
    pub fn as_value(&self) -> Value {
        Value {
            kind: ValueKind::Struct {
                content: self.content.clone(),
            },
            provenance: self.provenance(),
        }
    }

    /// Turns the `struct` context into a fully parsed `struct`.
    pub fn into_value(self) -> Value {
        let provenance = self.provenance();
        Value {
            kind: ValueKind::Struct {
                content: self
                    .content
                    .into_iter()
                    .filter(|content| match content {
                        StructContent::Field { name, .. } => !name.as_str().starts_with('_'),
                        StructContent::Diagnostic(_) => true,
                    })
                    .collect(),
            },
            provenance,
        }
    }

    /// Evaluates a `struct` reference.
    pub fn eval_struct_ref<'ctx>(
        &'ctx self,
        struct_ref: &ir::StructRef,
        last: Option<&'ctx Value>,
    ) -> StructRef<'ctx> {
        match struct_ref {
            ir::StructRef::Root(struct_ref_part) => match struct_ref_part {
                ir::StructRefPart::Parent => {
                    StructRef::Unfinished(self.parent().static_analysis_expect())
                }
                ir::StructRefPart::Last => {
                    StructRef::Finished(last.static_analysis_expect().kind.expect_struct())
                }
                ir::StructRefPart::Named(name) => StructRef::Finished(
                    self.field(&name.inner)
                        .static_analysis_expect()
                        .kind
                        .expect_struct(),
                ),
            },
            ir::StructRef::Chained { parent, field } => {
                let parent = self.eval_struct_ref(parent, last);

                match field {
                    ir::StructRefPart::Parent => match parent {
                        StructRef::Unfinished(struct_context) => {
                            StructRef::Unfinished(struct_context.parent().static_analysis_expect())
                        }
                        StructRef::Finished(_) => static_analysis_impossible(),
                    },
                    ir::StructRefPart::Last => static_analysis_impossible(),
                    ir::StructRefPart::Named(name) => {
                        StructRef::Finished(parent.field(&name.inner).kind.expect_struct())
                    }
                }
            }
        }
    }
}

/// The different recovery strategies.
#[derive(Debug)]
pub enum RecoveryStrategy {
    /// Divert to the recovery strategy of the parent `struct`.
    Fallback,
    /// Skips to the given offset.
    SkipTo {
        /// The offset to skip to.
        offset: RelativeOffset,
    },
}

/// A reference to a `struct` at runtime.
pub enum StructRef<'ctx> {
    /// A reference to a `struct` that finished parsing.
    Finished(&'ctx [StructContent]),
    /// A reference to a `struct` that is still being parsed.
    Unfinished(&'ctx StructContext<'ctx>),
}

impl<'ctx> StructRef<'ctx> {
    /// Returns the value of the given field.
    pub fn field(&self, field: &Symbol) -> &'ctx Value {
        let fields = match self {
            StructRef::Finished(struct_contents) => struct_contents,
            StructRef::Unfinished(struct_context) => &*struct_context.content,
        };

        fields
            .iter()
            .find_map(|content| content.val_if_name_eq(field))
            .static_analysis_expect()
    }
}
