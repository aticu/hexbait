//! Implements the `struct` context needed for parsing.

use crate::{
    DiagnosticId, Provenance, Value, ValueKind,
    ir::Symbol,
    parse::{
        RecoveryStrategy, StaticAnalysisImpossible as _,
        cursor::Cursor,
        diagnostics::{ParseErr, Result},
    },
};

/// The parsing context for a `struct`.
#[derive(Debug)]
pub struct StructContext<'parent> {
    /// The already parsed fields.
    parsed_fields: Vec<(Symbol, Value)>,
    /// The parent `struct`.
    parent: Option<&'parent StructContext<'parent>>,
    /// The recovery strategy to use if parsing fails.
    recovery_strategy: RecoveryStrategy,
    /// An error that may have occurred during parsing of this struct.
    error: Option<DiagnosticId>,
}

impl<'parent> StructContext<'parent> {
    /// Creates a new `struct` parsing context.
    pub fn new() -> StructContext<'static> {
        StructContext {
            parsed_fields: Vec::new(),
            parent: None,
            recovery_strategy: RecoveryStrategy::Fallback,
            error: None,
        }
    }

    /// Creates the context for a child `struct`.
    pub fn child<'this>(&'this self) -> StructContext<'this> {
        StructContext {
            parsed_fields: Vec::new(),
            parent: Some(self),
            recovery_strategy: RecoveryStrategy::Fallback,
            error: None,
        }
    }

    /// Returns the field named `field_name`.
    pub fn field(&self, field_name: &Symbol) -> Option<&Value> {
        for (name, val) in &self.parsed_fields {
            if name == field_name {
                return Some(val);
            }
        }

        None
    }

    /// Inserts a field with the given name into the `struct`.
    pub fn insert(&mut self, field_name: Symbol, value: Value) {
        // TODO: use resolved names here later
        self.parsed_fields.push((field_name, value));
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
        self.error = Some(err.id());

        match &self.recovery_strategy {
            RecoveryStrategy::Fallback => Err(err),
            RecoveryStrategy::SkipTo { offset } => {
                // the offset should be checked at the time of use
                cursor.set_offset(*offset).static_analysis_expect();

                Ok(())
            }
        }
    }

    /// Returns the `struct` context as a partially parsed `struct` value.
    pub fn as_value(&self) -> Value {
        let mut provenance = Provenance::empty();
        for (_, value) in &self.parsed_fields {
            provenance += &value.provenance;
        }

        Value {
            kind: ValueKind::Struct {
                fields: self.parsed_fields.clone(),
                error: self.error,
            },
            provenance,
        }
    }

    /// Turns the `struct` context into a fully parsed `struct`.
    pub fn into_value(self) -> Value {
        let mut provenance = Provenance::empty();
        for (_, value) in &self.parsed_fields {
            provenance += &value.provenance;
        }

        Value {
            kind: ValueKind::Struct {
                fields: self
                    .parsed_fields
                    .into_iter()
                    .filter(|(name, _)| !name.as_str().starts_with('_'))
                    .collect(),
                error: self.error,
            },
            provenance,
        }
    }
}
