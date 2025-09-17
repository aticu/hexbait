//! Implements displaying of bigram signatures.

use egui::{Align2, Color32, FontId, Rect, Sense, Ui, Vec2, show_tooltip_at_pointer, vec2};

use crate::{IDLE_TIME, statistics::Signature, window::Window};

use super::{
    cached_image::CachedImage,
    hex::{render_glyph, render_hex},
    settings::Settings,
};

/// Displays a data signature as an image of bigram probabilities.
pub struct SignatureDisplay {
    /// The image of the displayed signature.
    cached_image: CachedImage<(Window, u8, f32)>,
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
        window: Window,
        signature: &Signature,
        xor_value: u8,
        quality: f32,
        settings: &Settings,
    ) {
        let side_len_x = (rect.width().trunc() / 256.0).trunc();
        let side_len_y = (rect.height().trunc() / 256.0).trunc();
        let side_len = side_len_x.min(side_len_y);

        let rect = Rect::from_min_size(
            ui.cursor().left_top(),
            vec2(side_len * 256.0, side_len * 256.0),
        );

        self.cached_image
            .paint_at(ui, rect, (window, xor_value, quality), |x, y| {
                let first = x / side_len as usize;
                let second = y / side_len as usize;

                let intensity = signature.tuple(first as u8 ^ xor_value, second as u8 ^ xor_value);

                settings.scale_color_u8(intensity)
            });
        ui.advance_cursor_after_rect(rect);

        if quality < 1.0 {
            ui.ctx().request_repaint_after(IDLE_TIME);

            ui.painter().text(
                rect.center(),
                Align2::CENTER_CENTER,
                format!("Loading: {:6.2}%", quality * 100.0),
                FontId::proportional(settings.font_size()),
                Color32::WHITE,
            );
        }

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
            let intensity = signature.tuple(first ^ xor_value, second ^ xor_value);

            show_tooltip_at_pointer(
                ui.ctx(),
                ui.layer_id(),
                "signature_display_tooltip".into(),
                |ui| {
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
                },
            );
        }
    }
}

impl Default for SignatureDisplay {
    fn default() -> Self {
        SignatureDisplay::new()
    }
}
