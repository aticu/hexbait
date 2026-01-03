//! Renders hexdumps in the GUI.

use egui::{Align, Color32, Layout, Rect, ScrollArea, Sense, Ui, UiBuilder, Vec2, vec2};
use hexbait_common::{AbsoluteOffset, Input, Len, RelativeOffset};
use hexbait_lang::{View, ir::File};
use highlighting::highlight;

use crate::{
    gui::color,
    state::{ScrollState, SelectionState, State},
    window::Window,
};

pub mod highlighting;
mod inspector;
mod parse_result;
mod primitives;

use inspector::render_inspector;

pub use primitives::{render_glyph, render_hex, render_offset};

use super::marking::{MarkedLocation, MarkingKind, render_locations_on_bar};

/// Renders a hexdump to the GUI.
pub fn render(
    ui: &mut Ui,
    state: &mut State,
    input: &mut Input,
    parse_type: Option<&File>,
    parse_offset: &mut Option<AbsoluteOffset>,
) {
    let start = state.scroll_state.hex_start();
    let start_row = start.as_u64() / 16;

    let rect = ui.max_rect().intersect(ui.cursor());
    let height = ui.available_height();
    let window_size = height.trunc() as u64 * 16;
    let rows_onscreen = (height / state.settings.char_height()).trunc() as u64;

    // add 16 more to show one row "beyond the screen"
    let mut buf = vec![0; window_size as usize + 16];

    let file_size = input.len();

    let window = match input.window_at(start, &mut buf) {
        Ok(window) => window,
        Err(err) => {
            ui.label("hex display is experiencing issues:");
            ui.label(format!("{err}"));
            ui.spinner();
            return;
        }
    };

    let bar_width = (16 * state.settings.bar_width_multiplier()) as f32;
    let offset_chars = 16;
    let hex_chars = 16;
    let hex_rect_width = bar_width
        + ui.spacing().item_spacing.x
        + ((offset_chars + hex_chars * 3) as f32 * state.settings.char_width())
        + (2.0 * state.settings.large_space())
        + (17.0 * state.settings.small_space());

    let scroll_rect = rect.with_max_x(rect.min.x + hex_rect_width);

    // determine how many rows we can at most scroll down
    let max_height = (window.len() as u64).min(window_size).div_ceil(16);
    let max_scroll = max_height.saturating_sub(rows_onscreen);

    handle_scrolling(
        ui,
        &mut state.scroll_state,
        scroll_rect,
        max_scroll,
        rows_onscreen,
    );

    if ui.ctx().input(|input| !input.pointer.primary_down()) {
        state.selection_state.handle_mouse_release();
    }

    ui.scope_builder(
        UiBuilder::new()
            .max_rect(rect)
            .layout(Layout::left_to_right(Align::Min)),
        |ui| {
            let max_rect = ui.max_rect();

            render_sidebar(ui, state, window, rows_onscreen, max_scroll, start_row);

            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing = Vec2::ZERO;

                state
                    .marked_locations
                    .remove_where(|location| location.kind() == MarkingKind::Selection);
                if let Some(selection) = state.selection_state.selected_window() {
                    state
                        .marked_locations
                        .add(MarkedLocation::new(selection, MarkingKind::Selection));
                }

                for location in state.marked_locations.iter_window(Window::from_start_len(
                    start,
                    Len::from(window.len() as u64),
                )) {
                    let Some(range) = location.window().range_inclusive() else {
                        continue;
                    };
                    highlight(
                        ui,
                        range,
                        location.inner_color(),
                        location.border_color(),
                        file_size,
                        start_row + state.scroll_state.hex_scroll_offset,
                        rows_onscreen,
                        &state.settings,
                    );
                }

                for (i, row) in window
                    .chunks(16)
                    .enumerate()
                    .skip(state.scroll_state.hex_scroll_offset as usize)
                    .take(rows_onscreen as usize + 1)
                {
                    render_row(ui, state, start + Len::from(i as u64 * 16), row, file_size);
                }
            });
            let mut selected_buf;
            let selected_buf = if let Some(selection) = state.selection_state.selected_window() {
                selected_buf = vec![0; selection.size().as_u64() as usize];
                input.window_at(selection.start(), &mut selected_buf).ok()
            } else {
                None
            };

            // TODO: handle case where this is too small
            let rest_rect = max_rect.intersect(ui.cursor());
            let half_height = rest_rect.height() / 2.0;

            let top_rect = Rect::from_min_size(rest_rect.min, vec2(rest_rect.width(), half_height));

            let bottom_rect = Rect::from_min_size(
                rest_rect.min + vec2(0.0, half_height),
                vec2(rest_rect.width(), half_height),
            );

            ui.scope_builder(
                UiBuilder::new()
                    .max_rect(top_rect)
                    .layout(Layout::left_to_right(Align::Min)),
                |ui| {
                    ScrollArea::vertical()
                        .id_salt("inspector_scroll")
                        .max_height(half_height)
                        .show(ui, |ui| {
                            ui.vertical(|ui| {
                                render_inspector(ui, state, selected_buf);
                            });
                        });
                },
            );

            ui.scope_builder(
                UiBuilder::new()
                    .max_rect(bottom_rect)
                    .layout(Layout::left_to_right(Align::Min)),
                |ui| {
                    ScrollArea::both()
                        .id_salt("parser_scroll")
                        .max_height(half_height)
                        .show(ui, |ui| {
                            state.marked_locations.remove_where(|location| {
                                location.kind() == MarkingKind::HoveredParsed
                                    || location.kind() == MarkingKind::HoveredParseErr
                            });

                            let current_parse_offset = *parse_offset;
                            if let Some(window) = state.selection_state.selected_window() {
                                *parse_offset = Some(window.start());
                            }
                            let Some(parse_offset) = current_parse_offset else {
                                return;
                            };

                            let Some(parse_type) = parse_type else { return };
                            let view = View::Input(input.clone());
                            let view = View::Subview {
                                view: &view,
                                valid_range: parse_offset.to_relative()
                                    ..RelativeOffset::from(view.len().as_u64()),
                            };
                            let result =
                                hexbait_lang::eval_ir(parse_type, view, RelativeOffset::ZERO);
                            let hovered = parse_result::show_value(
                                ui,
                                state,
                                hexbait_lang::ir::path::Path::new(),
                                None,
                                &result.value,
                                &result.errors,
                            );

                            match hovered {
                                parse_result::HoverInfo::Nothing => (),
                                parse_result::HoverInfo::Value { path } => {
                                    if let Some(value) = result.value.subvalue_at_path(&path) {
                                        for range in value.provenance.byte_ranges() {
                                            state.marked_locations.add(MarkedLocation::new(
                                                (AbsoluteOffset::from(*range.start())
                                                    ..=AbsoluteOffset::from(*range.end()))
                                                    .into(),
                                                MarkingKind::HoveredParsed,
                                            ));
                                        }
                                    }
                                }
                                parse_result::HoverInfo::Error { id } => {
                                    for range in
                                        result.errors[id.raw_idx()].provenance.byte_ranges()
                                    {
                                        state.marked_locations.add(MarkedLocation::new(
                                            (AbsoluteOffset::from(*range.start())
                                                ..=AbsoluteOffset::from(*range.end()))
                                                .into(),
                                            MarkingKind::HoveredParseErr,
                                        ));
                                    }
                                }
                            }
                        });
                },
            );
        },
    );

    let copy_event = ui.input(|input| input.events.contains(&egui::Event::Copy));

    if copy_event
        && let Some(selection) = state.selection_state.selected_window()
        && let Ok(size) = usize::try_from(selection.size().as_u64())
    {
        let mut buf = vec![0; size];
        if let Ok(window) = input.window_at(selection.start(), &mut buf)
            && let Ok(as_text) = std::str::from_utf8(window)
        {
            ui.ctx().copy_text(as_text.to_string());
        }
    }
}

/// Handles scrolling of the hex view.
fn handle_scrolling(
    ui: &mut Ui,
    scroll_state: &mut ScrollState,
    scroll_rect: Rect,
    max_scroll: u64,
    rows_onscreen: u64,
) {
    if ui.rect_contains_pointer(scroll_rect) {
        let raw_scroll_delta = ui.ctx().input(|input| input.smooth_scroll_delta).y;
        let scroll_delta = (-raw_scroll_delta / 2.0).trunc() as i64;
        if scroll_delta < 0 {
            let scroll_delta = (-scroll_delta) as u64;

            if scroll_delta > scroll_state.hex_scroll_offset {
                let diff = scroll_delta - scroll_state.hex_scroll_offset;
                scroll_state.scroll_up(scroll_state.scrollbars.len() - 1, diff * 16);

                scroll_state.hex_scroll_offset = 0;
            } else {
                scroll_state.hex_scroll_offset -= scroll_delta;
            }
        } else {
            let scroll_delta = scroll_delta as u64;

            if scroll_state.hex_scroll_offset + scroll_delta > max_scroll {
                let diff = (scroll_state.hex_scroll_offset + scroll_delta) - max_scroll;
                scroll_state.scroll_down(
                    scroll_state.scrollbars.len() - 1,
                    diff * 16,
                    Len::from(rows_onscreen),
                );

                scroll_state.hex_scroll_offset = max_scroll;
            } else {
                scroll_state.hex_scroll_offset += scroll_delta;
            }
            scroll_state.hex_scroll_offset =
                (scroll_state.hex_scroll_offset).saturating_add(scroll_delta);
        }
    }

    // ensure that nothing scrolls too far
    if scroll_state.hex_scroll_offset > max_scroll {
        scroll_state.hex_scroll_offset = max_scroll;
    }
}

/// Renders a single row in a hexdump.
fn render_row(ui: &mut Ui, state: &mut State, offset: AbsoluteOffset, row: &[u8], file_size: Len) {
    let interact_with_offset =
        |ui: &Ui, offset, response: &egui::Response, selection_state: &mut SelectionState| {
            if let Some(origin) = ui.input(|input| input.pointer.latest_pos())
                && response.rect.contains(origin)
            {
                selection_state
                    .handle_interaction(offset, ui.input(|input| input.pointer.primary_pressed()));
            }
        };

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = Vec2::ZERO;

        let render_offset_info =
            |ui: &mut Ui, byte_offset: AbsoluteOffset, selection: Option<Window>| {
                ui.label(format!(
                    "offset from file start: 0x{:x} ({byte_offset:?})",
                    byte_offset.as_u64(),
                ));
                if let Some(selection) = selection {
                    let selection_offset =
                        byte_offset.as_u64() as i64 - selection.start().as_u64() as i64;
                    ui.label(format!(
                        "offset from selection start: {}0x{:x} ({selection_offset})",
                        if selection_offset < 0 { "-" } else { "" },
                        selection_offset.abs()
                    ));
                }
            };

        // offset
        render_offset(ui, &state.settings, Sense::hover(), offset).on_hover_ui(|ui| {
            let percentage = offset.as_u64() as f64 / file_size.as_u64() as f64 * 100.0;
            ui.label(format!("{percentage:.02}% of file"));
        });
        ui.add_space(state.settings.large_space());

        // hex values
        for (i, &byte) in row.iter().enumerate() {
            if i == 8 {
                ui.add_space(state.settings.small_space());
            }

            let byte_offset = offset + Len::from(i as u64);

            let response = render_hex(ui, &state.settings, Sense::hover(), byte);
            interact_with_offset(ui, byte_offset, &response, &mut state.selection_state);

            response.on_hover_ui(|ui| {
                render_glyph(ui, &state.settings, Sense::hover(), byte);
                render_offset_info(ui, byte_offset, state.selection_state.selected_window());
            });

            if i < 15 {
                ui.add_space(state.settings.small_space());
            }
        }

        // ensure non-full rows are still aligned
        if row.len() < 16 {
            let mut space = 0.0;
            // add the separator in the middle
            if row.len() < 9 {
                space += state.settings.small_space();
            }

            // add space for the characters
            space += (16 - row.len()) as f32 * state.settings.char_width() * 2.0;

            // add space between the characters
            space += (15 - row.len()) as f32 * state.settings.small_space();

            ui.add_space(space);
        }

        ui.add_space(state.settings.large_space());

        for (i, &byte) in row.iter().enumerate() {
            if i == 8 {
                ui.add_space(state.settings.small_space());
            }

            let byte_offset = offset + Len::from(i as u64);

            let response = render_glyph(ui, &state.settings, Sense::click(), byte);
            interact_with_offset(ui, byte_offset, &response, &mut state.selection_state);

            response.on_hover_ui(|ui| {
                render_hex(ui, &state.settings, Sense::hover(), byte);
                render_offset_info(ui, byte_offset, state.selection_state.selected_window());
            });
        }
    });
}

/// Shows a "minimap" of the hexview to show the context around it.
fn render_sidebar(
    ui: &mut Ui,
    state: &mut State,
    window: &[u8],
    rows_onscreen: u64,
    max_scroll: u64,
    start: u64,
) {
    let bar_width_multiplier = state.settings.bar_width_multiplier();

    let mut rect = ui.max_rect().intersect(ui.cursor());
    rect.set_width(16.0 * bar_width_multiplier as f32);

    let num_rows = window.len().div_ceil(16);
    rect.set_height(rect.height().min(num_rows as f32));

    let response = ui.allocate_rect(rect, Sense::click_and_drag());

    if let Some(pos) = response.interact_pointer_pos() {
        state.scroll_state.hex_scroll_offset =
            ((pos.y - rect.min.y).round() as u64).saturating_sub(rows_onscreen / 2);

        if state.scroll_state.hex_scroll_offset > max_scroll {
            state.scroll_state.hex_scroll_offset = max_scroll;
        }
    }

    let highlight_row_range =
        state.scroll_state.hex_scroll_offset..state.scroll_state.hex_scroll_offset + rows_onscreen;

    state.scroll_state.hex_sidebar_cached_image.paint_at(
        ui,
        rect,
        (
            start,
            state.scroll_state.hex_scroll_offset,
            state.settings.linear_byte_colors(),
        ),
        |x, y| {
            let x = x / bar_width_multiplier;
            if let Some(&byte) = window.get(y * 16 + x) {
                if highlight_row_range.contains(&(y as u64)) {
                    state.settings.byte_color(byte)
                } else {
                    color::lerp(state.settings.byte_color(byte), Color32::BLACK, 0.5)
                }
            } else {
                Color32::TRANSPARENT
            }
        },
    );

    render_locations_on_bar(
        ui,
        rect,
        Window::from_start_len(
            AbsoluteOffset::from(start * 16),
            Len::from(window.len() as u64),
        ),
        &mut state.marked_locations,
    );
}
