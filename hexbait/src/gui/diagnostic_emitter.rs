//! Implements diagnostic emitting to egui.

use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, FontId, Stroke, TextStyle, Ui};
use hexbait_lang::compile::{DiagnosticEmitter, Diagnostics, RgbColor, Style};

/// Emits the given diagnostic into the [`Ui`].
pub fn emit_diagnostics(ui: &mut Ui, diagnostics: &Diagnostics) {
    for diagnostic in diagnostics {
        let mut emitter = LayoutJobEmitter::new(
            TextStyle::Monospace.resolve(ui.style()),
            ui.visuals().text_color(),
        );

        diagnostic.emit(
            &mut emitter,
            diagnostics.source_name(),
            diagnostics.source_text(),
        );

        ui.label(emitter.into_job());
    }
}

/// Accumulates emitted diagnostics into a `LayoutJob` for rendering with egui.
struct LayoutJobEmitter {
    /// The layout job into which will be emitted.
    job: LayoutJob,
    /// The format applied to subsequently written text.
    format: TextFormat,
    /// The format to fall back to for unset style fields.
    base: TextFormat,
}

impl LayoutJobEmitter {
    /// Creates a new emitter.
    fn new(font: FontId, text_color: Color32) -> Self {
        let base = TextFormat {
            font_id: font,
            color: text_color,
            ..Default::default()
        };
        let mut job = LayoutJob::default();
        // Codespan aligns carets by column, so wrapping must be disabled.
        job.wrap.max_width = f32::INFINITY;
        Self {
            job,
            format: base.clone(),
            base,
        }
    }

    /// Returns the accumulated job, ready to be passed to `egui::Label::new`.
    fn into_job(self) -> LayoutJob {
        self.job
    }
}

/// Converts the color to an egui color.
fn to_color32(color: RgbColor) -> Color32 {
    Color32::from_rgb(color.r, color.g, color.b)
}

impl DiagnosticEmitter for LayoutJobEmitter {
    fn set_style(&mut self, style: Style) {
        let color = style.fg.map_or(self.base.color, to_color32);
        self.format = TextFormat {
            // egui has no bold monospace by default; approximate with brightness.
            color: if style.bold {
                color
            } else {
                color.gamma_multiply(0.85)
            },
            background: style.bg.map_or(Color32::TRANSPARENT, to_color32),
            underline: if style.underline {
                Stroke::new(1.0, color)
            } else {
                Stroke::NONE
            },
            ..self.base.clone()
        };
    }

    fn write(&mut self, text: &str) {
        self.job.append(text, 0.0, self.format.clone());
    }
}
