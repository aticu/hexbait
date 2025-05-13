//! Implements the primitives for showing hex views.

use egui::{Align2, FontId, Response, Sense, Ui, vec2};

use crate::gui::color::BYTE_COLORS;

/// Shows the given offset.
pub fn render_offset(ui: &mut Ui, scale: f32, sense: Sense, offset: u64) -> Response {
    let mut rect = ui.cursor();
    rect.max.x = rect.min.x + char_width(scale) * 16.0;
    rect.max.y = rect.min.y + char_height(scale);
    let painter = ui.painter().with_clip_rect(rect);

    painter.text(
        ui.cursor().min,
        Align2::LEFT_TOP,
        format!("{offset:016x}"),
        FontId::monospace(scale),
        BYTE_COLORS[0],
    );

    ui.allocate_rect(rect, sense)
}

/// Show the given byte in hex.
pub fn render_hex(ui: &mut Ui, scale: f32, sense: Sense, byte: u8) -> Response {
    let mut rect = ui.cursor();
    rect.max.x = rect.min.x + char_width(scale) * 2.0;
    rect.max.y = rect.min.y + char_height(scale);
    let painter = ui.painter().with_clip_rect(rect);

    let color = BYTE_COLORS[byte as usize];

    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        format!("{byte:02x}"),
        FontId::monospace(scale),
        color,
    );

    ui.allocate_rect(rect, sense)
}

/// Show the given byte as a glyph.
pub fn render_glyph(ui: &mut Ui, scale: f32, sense: Sense, byte: u8) -> Response {
    let as_char = match byte {
        // 0 is important enough to get its own glyph
        0 => Some('⋄'),
        // these appear very frequently in text and should thus be easily recognizable
        b'\t' => Some('→'),
        b'\n' => Some('↵'),
        b'\r' => Some('←'),
        // printable ASCII can just represent itself
        32..=126 => Some(byte as char),
        // there is no obvious character for the rest
        _ => None,
    };

    let mut rect = ui.cursor();
    rect.max.x = rect.min.x + char_width(scale);
    rect.max.y = rect.min.y + char_height(scale);
    let painter = ui.painter().with_clip_rect(rect);

    if let Some(c) = as_char {
        painter.text(
            ui.cursor().min,
            Align2::LEFT_TOP,
            format!("{c}"),
            FontId::monospace(scale),
            BYTE_COLORS[byte as usize],
        );
    } else {
        // render a grid of 8 dots representing the bits instead
        for bit in 0..8 {
            if (byte >> bit) & 1 != 0 {
                let radius = rect.width() / 12.0;
                let col_width = rect.width() / 3.0;
                let row_height = rect.height() / 5.0;
                let col_pos = if bit < 4 { col_width * 2.0 } else { col_width };
                let row_pos = (1.0 + ((bit % 4) as f32)) * row_height;

                painter.circle_filled(
                    rect.min + vec2(col_pos, row_pos),
                    radius,
                    BYTE_COLORS[byte as usize],
                );
            }
        }
    }

    ui.allocate_rect(rect, sense)
}

/// The width of a character.
pub(crate) fn char_width(scale: f32) -> f32 {
    scale / 1.5
}

/// The height of a character.
pub(crate) fn char_height(scale: f32) -> f32 {
    scale * 1.1
}

/// The size of a small space.
pub(crate) fn small_space(scale: f32) -> f32 {
    scale / 2.0
}

/// The size of a large space.
pub(crate) fn large_space(scale: f32) -> f32 {
    scale * 1.5
}
