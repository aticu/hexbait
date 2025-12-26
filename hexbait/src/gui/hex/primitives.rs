//! Implements the primitives for showing hex views.

use egui::{Align2, Color32, Response, Sense, Ui, vec2};
use hexbait_common::AbsoluteOffset;

use crate::state::Settings;

/// Shows the given offset.
pub fn render_offset(
    ui: &mut Ui,
    settings: &Settings,
    sense: Sense,
    offset: AbsoluteOffset,
) -> Response {
    let mut rect = ui.cursor();
    rect.max.x = rect.min.x + settings.char_width() * 16.0;
    rect.max.y = rect.min.y + settings.char_height();
    let painter = ui.painter().with_clip_rect(rect);

    painter.text(
        ui.cursor().min,
        Align2::LEFT_TOP,
        format!("{:016x}", offset.as_u64()),
        settings.hex_font(),
        Color32::from_rgb(100, 100, 100),
    );

    ui.allocate_rect(rect, sense)
}

/// Show the given byte in hex.
pub fn render_hex(ui: &mut Ui, settings: &Settings, sense: Sense, byte: u8) -> Response {
    let font = settings.hex_font();

    // TODO: replace this with a better calculation
    let char_width = font.size * 0.6;
    let mut rect = ui.cursor();
    rect.max.x = rect.min.x + char_width * 2.0;
    rect.max.y = rect.min.y + settings.char_height();
    let painter = ui.painter().with_clip_rect(rect);

    let color = settings.byte_color(byte);

    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        format!("{byte:02x}"),
        font,
        color,
    );

    ui.allocate_rect(rect, sense)
}

/// Show the given byte as a glyph.
pub fn render_glyph(ui: &mut Ui, settings: &Settings, sense: Sense, byte: u8) -> Response {
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
    rect.max.x = rect.min.x + settings.char_width();
    rect.max.y = rect.min.y + settings.char_height();
    let painter = ui.painter().with_clip_rect(rect);

    let color = settings.byte_color(byte);

    if let Some(c) = as_char {
        painter.text(
            ui.cursor().min,
            Align2::LEFT_TOP,
            format!("{c}"),
            settings.hex_font(),
            color,
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

                painter.circle_filled(rect.min + vec2(col_pos, row_pos), radius, color);
            }
        }
    }

    ui.allocate_rect(rect, sense)
}
