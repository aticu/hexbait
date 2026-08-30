//! Implements diagnostics for parsing the hexbait language.
//!
//! This is different from diagnostics that occur while evaluating the language.

use std::io;

use crate::Span;

pub use custom_emitter::{DiagnosticEmitter, RgbColor, Style};

mod custom_emitter;

/// A diagnostic that occurred while parsing the hexbait language.
#[derive(Debug, Clone)]
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

    /// Creates a new error.
    pub fn error(message: impl ToString, main_label: Label) -> Diagnostic {
        Diagnostic {
            level: DiagnosticLevel::Error,
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

    /// Turns this diagnostic into a `codespan_reporting` diagnostic.
    fn to_codespan_reporting_diagnostic(&self) -> codespan_reporting::diagnostic::Diagnostic<()> {
        use codespan_reporting::diagnostic::Severity;

        let severity = match self.level {
            DiagnosticLevel::Warning => Severity::Warning,
            DiagnosticLevel::Error => Severity::Error,
        };

        let map_span = |span: Span| span.start..span.end;

        codespan_reporting::diagnostic::Diagnostic::new(severity)
            .with_message(self.message())
            .with_label(
                codespan_reporting::diagnostic::Label::primary(
                    (),
                    map_span(self.main_label().span()),
                )
                .with_message(self.main_label().message()),
            )
            .with_labels_iter(self.additional_labels().map(|label| {
                codespan_reporting::diagnostic::Label::secondary((), map_span(label.span()))
                    .with_message(label.message())
            }))
    }

    /// Emits the diagnostic to the given writer.
    pub fn emit_to_stderr(&self, source_name: &str, source_text: &str) -> io::Result<()> {
        let file = codespan_reporting::files::SimpleFile::new(source_name, source_text);
        let diagnostic = self.to_codespan_reporting_diagnostic();

        let config = codespan_reporting::term::Config::default();

        match codespan_reporting::term::emit_to_write_style(
            &mut codespan_reporting::term::termcolor::StandardStream::stderr(
                codespan_reporting::term::termcolor::ColorChoice::Auto,
            ),
            &config,
            &file,
            &diagnostic,
        ) {
            Ok(()) => Ok(()),
            Err(err) => match err {
                codespan_reporting::files::Error::Io(err) => Err(err),
                _ => unreachable!("this is a bug"),
            },
        }
    }

    /// Emits the diagnostic.
    pub fn emit<E: DiagnosticEmitter>(
        &self,
        emitter: &mut E,
        source_name: &str,
        source_text: &str,
    ) {
        let file = codespan_reporting::files::SimpleFile::new(source_name, source_text);
        let diagnostic = self.to_codespan_reporting_diagnostic();

        let config = codespan_reporting::term::Config::default();

        match codespan_reporting::term::emit_to_write_style(
            &mut custom_emitter::Emitter(emitter),
            &config,
            &file,
            &diagnostic,
        ) {
            Ok(()) => (),
            Err(_) => unreachable!("this is a bug"),
        }
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
#[derive(Debug, Clone)]
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
