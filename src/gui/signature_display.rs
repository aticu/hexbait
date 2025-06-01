use std::ops::Range;

use egui::{Rect, Sense, Ui, Vec2, show_tooltip_at_pointer, vec2};

use crate::statistics::Signature;

use super::{
    cached_image::CachedImage,
    hex::{render_glyph, render_hex},
    settings::Settings,
};

/// Displays a data signature as an image of 2-gram probabilities.
pub struct SignatureDisplay {
    /// The image of the displayed signature.
    cached_image: CachedImage<Range<u64>>,
}

impl SignatureDisplay {
    /// Creates a new signature display.
    pub fn new() -> SignatureDisplay {
        SignatureDisplay {
            cached_image: CachedImage::new(),
        }
    }

    /// Renders the signature into the given rect.
    pub fn render(
        &mut self,
        ui: &mut Ui,
        rect: Rect,
        range: Range<u64>,
        signature: &Signature,
        settings: &Settings,
    ) {
        let side_len_x = (rect.width().trunc() / 256.0).trunc();
        let side_len_y = (rect.height().trunc() / 256.0).trunc();
        let side_len = side_len_x.min(side_len_y);

        let rect = Rect::from_min_size(
            ui.cursor().left_top(),
            vec2(side_len * 256.0, side_len * 256.0),
        );

        self.cached_image.paint_at(ui, rect, range, |x, y| {
            let first = x / side_len as usize;
            let second = y / side_len as usize;

            let intensity = signature.tuple(first as u8, second as u8);
            settings.scale_color_u8(intensity)
        });
        ui.advance_cursor_after_rect(rect);

        let hover_positions = ui.ctx().input(|input| {
            if let Some(pos) = input.pointer.latest_pos()
                && rect.contains(pos)
            {
                let first = ((pos - rect.min).x / side_len) as u8;
                let second = ((pos - rect.min).y / side_len) as u8;

                Some((first, second))
            } else {
                None
            }
        });

        if let Some((first, second)) = hover_positions {
            let intensity = signature.tuple(first, second);

            show_tooltip_at_pointer(ui.ctx(), ui.layer_id(), "signature_display".into(), |ui| {
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        render_hex(ui, settings, Sense::hover(), first);
                        render_hex(ui, settings, Sense::hover(), second);

                        ui.spacing_mut().item_spacing = Vec2::ZERO;
                        ui.add_space(settings.large_space());

                        render_glyph(ui, settings, Sense::hover(), first);
                        render_glyph(ui, settings, Sense::hover(), second);
                    });
                    ui.label(format!(
                        "Relative Density: {:0.02}%",
                        intensity as f64 / 2.55,
                    ));
                });
            });
        }
    }
}
