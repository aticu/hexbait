//! Renders a search screen in the GUI.

use egui::{Align2, Rect, ScrollArea, Sense, Slider, TextStyle, Ui, Vec2, vec2};
use hexbait_common::{Input, Len};

use crate::{gui::primitives::render_hex, marking::MarkType, state::State};

/// Shows the search screen in the GUI.
pub fn show(ui: &mut Ui, state: &mut State, input: &Input) {
    ui.vertical(|ui| {
        ui.add(
            Slider::new(state.format_discovery.len_mut(), 2..=1024)
                .text("Length")
                .logarithmic(true),
        );

        ScrollArea::both().auto_shrink(false).show(ui, |ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;

            render_header(ui, state, state.format_discovery.len());

            let ty = MarkType::UserMark {
                name: state.format_discovery.mark_name().to_string(),
            };
            if let Some(mark_iter) = state.marked_locations.iter_marks_of_type(&ty) {
                let len = state.format_discovery.len();
                let mut buf = Vec::with_capacity(len as usize);

                for mark in mark_iter {
                    ui.horizontal(|ui| {
                        match input.read_at(mark.window.start(), Len::from(len), Some(&mut buf)) {
                            Ok(buf) => {
                                for &byte in &*buf {
                                    render_hex(ui, &state.settings, Sense::hover(), byte);
                                    ui.add_space(state.settings.small_space());
                                }
                            }
                            Err(err) => {
                                ui.label(format!(
                                    "could not read data at mark {:?}: {err}",
                                    mark.window.start()
                                ));
                            }
                        }
                    });
                }
            }
        });
    });
}

/// Renders the header of the table.
fn render_header(ui: &mut Ui, state: &mut State, len: u64) {
    let text_color = ui.visuals().text_color();
    let mut small_font = TextStyle::Small.resolve(ui.style());
    // decrease the size slightly to actually fit everything
    small_font.size *= 0.8;

    let hex_space = state.settings.char_width() * 2.0;
    let row_height = small_font.size * 2.0;
    let small_space = state.settings.small_space();

    ui.horizontal(|ui| {
        for i in 0..len {
            let rect = Rect::from_min_size(ui.cursor().min, vec2(hex_space, row_height));

            ui.painter().text(
                rect.center_top(),
                Align2::CENTER_TOP,
                format!("{i}"),
                small_font.clone(),
                text_color,
            );
            ui.painter().text(
                rect.center_bottom(),
                Align2::CENTER_BOTTOM,
                format!("0x{i:x}"),
                small_font.clone(),
                text_color,
            );

            ui.add_space(hex_space + small_space);
        }
    });

    ui.add_space(row_height);
}
