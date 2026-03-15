//! Implements the classification information module.

use egui::{Color32, Label, Rect, RichText, Sense, Ui, pos2, vec2};
use hexbait_common::Input;

use crate::state::State;

/// Shows classification information in the GUI.
pub fn show(ui: &mut Ui, state: &mut State, _: &Input) {
    if let Some(result) = &state.classification_state.classification_results {
        for class in result {
            ui.horizontal(|ui| {
                let font_size = state.settings.font_size();

                let (response, painter) =
                    ui.allocate_painter(vec2(font_size * 10.0, font_size), Sense::hover());

                let color = if class.score < class.min_score {
                    Color32::RED
                } else {
                    Color32::GREEN
                };

                let rect = response.rect;
                let width = rect.width();
                painter.rect_filled(
                    Rect::from_two_pos(
                        pos2(rect.min.x, rect.min.y),
                        pos2(rect.min.x + class.score * width, rect.max.y),
                    ),
                    0.0,
                    color,
                );
                painter.rect_filled(
                    Rect::from_two_pos(
                        pos2(rect.min.x + (class.min_score * width).trunc(), rect.min.y),
                        pos2(
                            rect.min.x + (class.min_score * width).trunc() + 1.0,
                            rect.max.y,
                        ),
                    ),
                    0.0,
                    Color32::WHITE,
                );

                ui.add_sized(
                    [font_size * 5.0, font_size],
                    Label::new(RichText::new(format!("{:.03}", class.score)).color(color)),
                );
                ui.label(RichText::new(class.name).color(color));
            });
        }
    }
}
