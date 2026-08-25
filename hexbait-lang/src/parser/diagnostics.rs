//! Implements diagnostics for parsing the hexbait language.
//!
//! This is different from diagnostics that occur while evaluating the language.

use crate::Span;

/// A diagnostic that occurred while parsing the hexbait language.
pub struct Diagnostic {
    /// The level of the diagnostic.
    level: DiagnosticLevel,
    /// The message of this diagnostic.
    message: String,
    /// The main label of this diagnostic.
    main_label: Label,
    /// The additional labels of this diagnostic.
    additional_labels: Vec<Label>,
}

impl Diagnostic {
    /// Creates a new diagnostic.
    pub fn new(level: DiagnosticLevel, message: impl ToString, main_label: Label) -> Diagnostic {
        Diagnostic {
            level,
            message: message.to_string(),
            main_label,
            additional_labels: Vec::new(),
        }
    }

    /// Returns the diagnostic with the added label.
    pub fn with_label(mut self, label: Label) -> Self {
        self.add_label(label);
        self
    }

    /// Adds a label to the diagnostic.
    pub fn add_label(&mut self, label: Label) {
        self.additional_labels.push(label);
    }

    /// The level of this diagnostic.
    pub fn level(&self) -> DiagnosticLevel {
        self.level
    }

    /// The message of this diagnostic.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// The main label of this diagnostic.
    pub fn main_label(&self) -> &Label {
        &self.main_label
    }

    /// The additional labels of this diagnostic.
    pub fn additional_labels(&self) -> impl Iterator<Item = &Label> {
        self.additional_labels.iter()
    }
}

/// The level of a diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticLevel {
    /// The diagnostic is a warning.
    Warning,
    /// The diagnostic is an error.
    Error,
}

/// An label in a diagnostic.
#[derive(Debug)]
pub struct Label {
    /// The span for this label.
    span: Span,
    /// The message for this label.
    message: String,
}

impl Label {
    /// Creates a new label.
    pub fn new(message: impl ToString, span: Span) -> Label {
        Label {
            span,
            message: message.to_string(),
        }
    }

    /// The span of this label.
    pub fn span(&self) -> Span {
        self.span
    }

    /// The message of this label.
    pub fn message(&self) -> &str {
        &self.message
    }
}
