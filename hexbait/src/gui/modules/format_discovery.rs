//! Renders a search screen in the GUI.

use egui::{
    Align2, CornerRadius, Popup, PopupCloseBehavior, Rect, ScrollArea, Sense, Slider, Stroke,
    StrokeKind, TextStyle, Ui, Vec2, scroll_area::ScrollSource, vec2,
};
use hexbait_common::{Input, Len};

use crate::{
    gui::primitives::render_hex,
    marking::MarkType,
    state::{ColumnType, State},
};

/// Shows the search screen in the GUI.
pub fn show(ui: &mut Ui, state: &mut State, input: &Input) {
    ui.vertical(|ui| {
        let min_len = state
            .format_discovery
            .type_state_mut()
            .columns_mut()
            .last()
            .map(|col| col.covered_range().end)
            .unwrap_or(2);

        ui.add(
            Slider::new(
                &mut state.format_discovery.type_state_mut().len,
                min_len..=1024,
            )
            .text("Length")
            .logarithmic(true),
        );

        ScrollArea::both()
            .auto_shrink(false)
            .scroll_source(ScrollSource::SCROLL_BAR | ScrollSource::MOUSE_WHEEL)
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing = Vec2::ZERO;

                let len = state.format_discovery.type_state_mut().len;

                render_header(ui, state, len);

                let ty = MarkType::UserMark {
                    name: state.format_discovery.mark_name().to_string(),
                };
                if let Some(mark_iter) = state.marked_locations.iter_marks_of_type(&ty) {
                    let mut buf = Vec::with_capacity(len as usize);

                    for mark in mark_iter {
                        ui.horizontal(|ui| {
                            match input.read_at(mark.window.start(), Len::from(len), Some(&mut buf))
                            {
                                Ok(buf) => {
                                    let line = |ui: &mut Ui| {
                                        let start = ui.cursor().min;
                                        ui.painter().line_segment(
                                            [
                                                start,
                                                start + vec2(0.0, state.settings.char_height()),
                                            ],
                                            Stroke::new(
                                                1.0,
                                                state
                                                    .settings
                                                    .format_discovery_column_border_color(),
                                            ),
                                        );
                                    };

                                    let cols =
                                        state.format_discovery.type_state_mut().columns_mut();
                                    let mut col_idx = 0;

                                    for (i, &byte) in buf.iter().enumerate() {
                                        if let Some(col) = cols.get(col_idx)
                                            && col.covered_range().start == i as u64
                                        {
                                            line(ui);
                                        }

                                        render_hex(ui, &state.settings, Sense::hover(), byte);

                                        if let Some(col) = cols.get(col_idx)
                                            && col.covered_range().end == i as u64 + 1
                                        {
                                            line(ui);
                                            col_idx += 1;
                                        }

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
    let cell_space = hex_space + small_space;

    let row_rect = Rect::from_min_size(
        ui.cursor().min,
        vec2(len as f32 * cell_space - small_space, row_height),
    );

    let response = ui.allocate_rect(row_rect, Sense::click_and_drag());
    let type_state = state.format_discovery.type_state_mut();

    ui.horizontal(|ui| {
        let painter = ui.painter();
        for col in type_state.columns_mut() {
            let range = col.covered_range();
            let rect = Rect::from_min_size(
                row_rect.min + vec2(range.start as f32 * cell_space, 0.0),
                vec2(
                    range.count() as f32 * cell_space - small_space + 1.0,
                    row_height,
                ),
            );
            let mut corner_radius = CornerRadius::from(row_height * 0.25);
            corner_radius.sw = CornerRadius::ZERO.sw;
            corner_radius.se = CornerRadius::ZERO.se;

            painter.rect(
                rect,
                corner_radius,
                state.settings.format_discovery_column_background_color(),
                Stroke::new(1.0, state.settings.format_discovery_column_border_color()),
                StrokeKind::Inside,
            );
        }

        for i in 0..len {
            let rect = Rect::from_min_size(
                row_rect.min + vec2(i as f32 * cell_space, 0.0),
                vec2(hex_space, row_height),
            );

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

            if response.drag_started() && ui.rect_contains_pointer(rect) {
                type_state.start_interaction_at(i);
            }

            if let Some(pointer_pos) = ui.input(|input| input.pointer.latest_pos())
                && rect.expand2(vec2(0.0, f32::INFINITY)).contains(pointer_pos)
            {
                type_state.signal_current_offset(i);
            }
        }

        let mut col_to_remove = None;
        let mut context_menu_idx = type_state.context_menu_idx;
        for (i, col) in type_state.columns_mut().iter_mut().enumerate() {
            let range = col.covered_range();
            let rect = Rect::from_min_size(
                row_rect.min + vec2(range.start as f32 * cell_space, 0.0),
                vec2(
                    range.count() as f32 * cell_space - small_space + 1.0,
                    row_height,
                ),
            );

            if (!response.context_menu_opened() && ui.rect_contains_pointer(rect))
                || (response.context_menu_opened() && context_menu_idx == Some(i))
            {
                Popup::context_menu(&response)
                    .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
                    .show(|ui| {
                        context_menu_idx = Some(i);

                        ui.text_edit_singleline(col.name.get_or_insert_default());
                        if col.name.as_ref().is_some_and(|name| name.is_empty()) {
                            col.name = None;
                        }

                        if ui.button("Remove").clicked() {
                            col_to_remove = Some(i);
                            context_menu_idx = None;
                        }
                        ui.menu_button("Set type", |ui| {
                            ui.set_min_size(vec2(150.0, 0.0));
                            for ty in ColumnType::iter_all_types() {
                                let response = if ty == col.ty {
                                    ui.button(format!("✔ {}", ty.name()))
                                } else {
                                    ui.button(ty.name())
                                };

                                if response.clicked() {
                                    col.ty = ty;
                                }
                            }
                        });
                    });
            }
        }
        if !response.context_menu_opened() {
            context_menu_idx = None;
        }
        type_state.context_menu_idx = context_menu_idx;

        if let Some(col) = col_to_remove {
            type_state.remove_col(col);
        }
    });

    if !ui.input(|input| input.pointer.primary_down()) {
        type_state.stop_interaction();
    }
}
