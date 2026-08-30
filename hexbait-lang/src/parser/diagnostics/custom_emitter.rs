//! Implements custom emitters for diagnostics.
//!
//! This exists mainly to keep the `codespan_reporting` dependency limited to the `diagnostics` module.

use std::io;

use codespan_reporting::term::termcolor::{Color, ColorSpec, WriteColor};

/// An RGB color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RgbColor {
    /// The red part of the RGB color.
    pub r: u8,
    /// The green part of the RGB color.
    pub g: u8,
    /// The blue part of the RGB color.
    pub b: u8,
}

/// A display style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Style {
    /// The foreground color.
    pub fg: Option<RgbColor>,
    /// The background color.
    pub bg: Option<RgbColor>,
    /// Whether or not to use bold text.
    pub bold: bool,
    /// Whether or not to underline text.
    pub underline: bool,
}

/// Implemented by custom diagnostics emitters.
pub trait DiagnosticEmitter {
    /// Sets the style.
    fn set_style(&mut self, style: Style);

    /// Emits the given text using the current style.
    fn write(&mut self, text: &str);
}

/// Implements `WriteColor` so that the custom emitter can be used.
pub struct Emitter<'e, E: DiagnosticEmitter>(pub &'e mut E);

impl<E: DiagnosticEmitter> io::Write for Emitter<'_, E> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let text = std::str::from_utf8(buf)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;

        self.0.write(text);

        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<E: DiagnosticEmitter> WriteColor for Emitter<'_, E> {
    fn supports_color(&self) -> bool {
        true
    }

    fn set_color(&mut self, spec: &ColorSpec) -> io::Result<()> {
        self.0.set_style(Style {
            fg: map_color(spec.fg()),
            bg: map_color(spec.bg()),
            bold: spec.bold(),
            underline: spec.underline(),
        });

        Ok(())
    }

    fn reset(&mut self) -> io::Result<()> {
        self.0.set_style(Style::default());

        Ok(())
    }
}

/// Maps between color spaces.
fn map_color(color: Option<&Color>) -> Option<RgbColor> {
    color.and_then(|color| match *color {
        Color::Black => Some(RgbColor { r: 0, g: 0, b: 0 }),
        Color::Blue => Some(RgbColor { r: 0, g: 0, b: 255 }),
        Color::Green => Some(RgbColor { r: 0, g: 255, b: 0 }),
        Color::Red => Some(RgbColor { r: 255, g: 0, b: 0 }),
        Color::Cyan => Some(RgbColor {
            r: 0,
            g: 255,
            b: 255,
        }),
        Color::Magenta => Some(RgbColor {
            r: 255,
            g: 0,
            b: 255,
        }),
        Color::Yellow => Some(RgbColor {
            r: 255,
            g: 255,
            b: 0,
        }),
        Color::White => Some(RgbColor {
            r: 255,
            g: 255,
            b: 255,
        }),
        Color::Rgb(r, g, b) => Some(RgbColor { r, g, b }),
        _ => None,
    })
}
