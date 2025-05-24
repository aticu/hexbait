use egui::Ui;

use crate::statistics::Signature;

pub fn render_signature_display(ui: &mut Ui, signature: &Signature) {
    const SIDE_LEN: f32 = 4.0;
    let rect = egui::Rect::from_min_size(
        ui.cursor().left_top(),
        egui::vec2(SIDE_LEN * 256.0, SIDE_LEN * 256.0),
    );
    let response = ui.allocate_rect(rect, egui::Sense::hover());

    for first in 0..=255 {
        for second in 0..=255 {
            let rect = egui::Rect::from_min_size(
                rect.left_top() + egui::vec2(SIDE_LEN * first as f32, SIDE_LEN * second as f32),
                egui::vec2(SIDE_LEN, SIDE_LEN),
            );
            let painter = ui.painter().with_clip_rect(rect);

            let intensity = signature.tuple(first, second);
            let color = crate::gui::color::VIRIDIS[intensity as usize];

            if let Some(pos) = response.hover_pos() {
                if rect.contains(pos) {
                    egui::show_tooltip_at_pointer(
                        ui.ctx(),
                        ui.layer_id(),
                        "overview_hover".into(),
                        |ui| {
                            ui.vertical(|ui| {
                                ui.horizontal(|ui| {
                                    crate::gui::hex::render_hex(
                                        ui,
                                        20.0,
                                        egui::Sense::hover(),
                                        first,
                                    );
                                    crate::gui::hex::render_hex(
                                        ui,
                                        20.0,
                                        egui::Sense::hover(),
                                        second,
                                    );

                                    ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
                                    ui.add_space(30.0);

                                    crate::gui::hex::render_glyph(
                                        ui,
                                        20.0,
                                        egui::Sense::hover(),
                                        first,
                                    );
                                    crate::gui::hex::render_glyph(
                                        ui,
                                        20.0,
                                        egui::Sense::hover(),
                                        second,
                                    );
                                });
                                ui.label(
                                    egui::RichText::new(format!(
                                        "Relative Density: {:0.02}%",
                                        intensity as f64 / 2.55,
                                    ))
                                    .color(color),
                                );
                            });
                        },
                    );
                }
            }

            painter.rect_filled(rect, 0.0, color);
        }
    }
}
