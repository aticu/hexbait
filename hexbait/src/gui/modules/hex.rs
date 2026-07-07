//! Renders hexdumps in the GUI.

use egui::{Color32, Rect, RichText, Sense, Ui, Vec2};
use hexbait_common::{AbsoluteOffset, Input, Len};

use crate::{
    gui::{
        color,
        highlighting::highlight,
        marking::{hover_marking, render_locations_on_bar},
        modules::bars::{SIDE_BAR_WIDTH, highest_aligned_value},
        primitives::{render_glyph, render_hex, render_offset},
    },
    marking::MarkType,
    state::{ScrollState, State},
    window::Window,
};

/// Shows a hexdump in the GUI.
pub fn show(ui: &mut Ui, state: &mut State, input: &Input) {
    let start = state.scroll_state.hex_start();
    let start_row = start.as_u64() / 16;

    let rect = ui.max_rect().intersect(ui.cursor());
    let height = ui.available_height();
    let window_size = height.trunc() as u64 * 16;
    let rows_onscreen = (height / state.settings.char_height()).trunc() as u64;

    // add 16 more to show one row "beyond the screen"
    let read_len = Len::from(window_size + 16);

    let file_size = input.len();

    let window = match input.read_at(start, read_len, None) {
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

    render_sidebar(ui, state, &window, rows_onscreen, max_scroll, start_row);

    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing = Vec2::ZERO;

        state
            .marked_locations
            .clear_marks_of_type(MarkType::Selection);
        if let Some(selection) = state.selection_state.selected_window() {
            state.marked_locations.add(selection, MarkType::Selection);
        }

        state.marked_locations.iter_marks_in_window(
            Window::from_start_len(start, Len::from(window.len() as u64)),
            |mark| {
                let Some(range) = mark.window.range_inclusive() else {
                    return;
                };
                highlight(
                    ui,
                    range,
                    mark.ty.inner_color(),
                    mark.ty.border_color(),
                    file_size,
                    start_row + state.scroll_state.hex_scroll_offset,
                    rows_onscreen,
                    &state.settings,
                );
            },
        );

        for (i, row) in window
            .chunks(16)
            .enumerate()
            .skip(state.scroll_state.hex_scroll_offset as usize)
            .take(rows_onscreen as usize + 1)
        {
            render_row(
                ui,
                state,
                input,
                start + Len::from(i as u64 * 16),
                row,
                file_size,
            );
        }
    });

    if ui.input(|input| input.events.contains(&egui::Event::Copy))
        && let Some(selection) = state.selection_state.selected_window()
        && let Ok(window) = input.read_at(selection.start(), selection.size(), None)
        && let Ok(as_text) = std::str::from_utf8(&window)
    {
        ui.ctx().copy_text(as_text.to_string());
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

/// Renders the context menu for a byte at the given offset.
fn byte_context_menu(ui: &mut Ui, state: &mut State, input: &Input, offset: AbsoluteOffset) {
    ui.set_min_width(100.0);
    let is_marked = state.marked_locations.user_mark_at_pos(offset).is_some();

    #[expect(clippy::collapsible_else_if, reason = "code reads cleaner this way")]
    if is_marked {
        if ui.button("Unmark").clicked() {
            state.marked_locations.remove_where(None, |mark| {
                matches!(mark.ty, MarkType::UserMark { .. }) && mark.window.start() == offset
            });
        }
    } else {
        if ui.button("Mark").clicked() {
            state.marked_locations.add(
                Window::from_start_len(offset, Len::from(1)),
                MarkType::UserMark {
                    name: state.marked_locations.current_mark_name.clone(),
                },
            );
        }
    }

    if ui.button("Copy offset").clicked() {
        ui.ctx().copy_text(format!("{}", offset.as_u64()));
    }

    if let Some(selected_window) = state.selection_state.selected_window()
        && selected_window.contains(offset)
    {
        let selection = || input.read_at(selected_window.start(), selected_window.size(), None);
        let render_selection = |render_byte: fn(u8) -> String, separator| {
            let mut out = String::new();
            if let Ok(selection) = selection() {
                let mut first = true;
                for &byte in &*selection {
                    if first {
                        first = false;
                    } else if let Some(separator) = separator {
                        out.push_str(separator);
                    }
                    out.push_str(&render_byte(byte));
                }
            }
            out
        };

        ui.menu_button("Copy as", |ui| {
            if let Ok(selection) = selection()
                && let Ok(as_str) = std::str::from_utf8(&selection)
                && ui.button("Text").clicked()
            {
                ui.ctx().copy_text(as_str.to_string());
            }
            if ui.button("Escaped hex").clicked() {
                ui.ctx().copy_text(render_selection(
                    |byte| match byte {
                        0x20..=0x7e => format!("{}", byte as char),
                        _ => {
                            format!("\\x{byte:02x}")
                        }
                    },
                    None,
                ));
            }
            if ui.button("Spaced hex").clicked() {
                ui.ctx()
                    .copy_text(render_selection(|byte| format!("{byte:02x}"), Some(" ")));
            }
            if ui.button("Joined hex").clicked() {
                ui.ctx()
                    .copy_text(render_selection(|byte| format!("{byte:02x}"), None));
            }
            if ui.button("Base 64 (RFC4648)").clicked()
                && let Ok(selection) = selection()
            {
                use base64::prelude::*;
                ui.ctx().copy_text(BASE64_STANDARD.encode(&*selection));
            }
            if ui.button("Base 64 (URL safe)").clicked()
                && let Ok(selection) = selection()
            {
                use base64::prelude::*;
                ui.ctx().copy_text(BASE64_URL_SAFE.encode(&*selection));
            }
        });
    }
}

/// Renders a single row in a hexdump.
fn render_row(
    ui: &mut Ui,
    state: &mut State,
    input: &Input,
    offset: AbsoluteOffset,
    row: &[u8],
    file_size: Len,
) {
    let interact_with_offset = |ui: &Ui, offset, response: &egui::Response, state: &mut State| {
        if let Some(origin) = ui.input(|input| input.pointer.latest_pos())
            && response.rect.contains(origin)
        {
            let (primary_pressed, shift_pressed, ctrl_pressed) = ui.input(|input| {
                (
                    input.pointer.primary_pressed(),
                    input.modifiers.shift,
                    input.modifiers.ctrl,
                )
            });

            let primary_pressed = primary_pressed && response.is_pointer_button_down_on();

            if ctrl_pressed {
                let is_marked = state.marked_locations.user_mark_at_pos(offset).is_some();

                if is_marked {
                    if primary_pressed {
                        state.marked_locations.remove_where(None, |mark| {
                            matches!(mark.ty, MarkType::UserMark { .. })
                                && mark.window.start() == offset
                        });
                    } else {
                        response.clone().on_hover_ui(|ui| {
                            ui.label("unmark");
                        });
                    }
                } else if primary_pressed {
                    state.marked_locations.add(
                        Window::from_start_len(offset, Len::from(1)),
                        MarkType::UserMark {
                            name: state.marked_locations.current_mark_name.clone(),
                        },
                    );
                } else {
                    response.clone().on_hover_ui(|ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 0.0;
                            ui.label("mark as ");
                            if state.marked_locations.current_mark_name.is_empty() {
                                ui.label(RichText::new("unnamed").italics());
                            } else {
                                ui.label(state.marked_locations.current_mark_name.clone());
                            }
                        });
                    });
                }
            } else {
                state
                    .selection_state
                    .handle_interaction(offset, primary_pressed, shift_pressed);
            }
        }
    };

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = Vec2::ZERO;

        let render_offset_info =
            |ui: &mut Ui, byte_offset: AbsoluteOffset, selection: Option<Window>| {
                ui.label(format!(
                    "offset from file start: 0x{:x} ({byte_offset:?}, {}B)",
                    byte_offset.as_u64(),
                    size_format::SizeFormatterBinary::new(byte_offset.as_u64())
                ));
                if let Some(selection) = selection {
                    let selection_offset =
                        byte_offset.as_u64() as i64 - selection.start().as_u64() as i64;
                    ui.label(format!(
                        "offset from selection start: {sign}0x{:x} ({selection_offset}, {sign}{}B)",
                        selection_offset.unsigned_abs(),
                        size_format::SizeFormatterBinary::new(selection_offset.unsigned_abs()),
                        sign = if selection_offset < 0 { "-" } else { "" },
                    ));
                }
            };

        // offset
        render_offset(ui, &state.settings, Sense::hover(), offset).on_hover_ui(|ui| {
            let percentage = offset.as_u64() as f64 / file_size.as_u64() as f64 * 100.0;
            ui.label(format!(
                "{} ({}B) {percentage:.02}% of file",
                offset.as_u64(),
                size_format::SizeFormatterBinary::new(offset.as_u64())
            ));
        });
        ui.add_space(state.settings.large_space());

        // hex values
        for (i, &byte) in row.iter().enumerate() {
            if i == 8 {
                ui.add_space(state.settings.small_space());
            }

            let byte_offset = offset + Len::from(i as u64);

            let response = render_hex(ui, &state.settings, Sense::click(), byte);
            interact_with_offset(ui, byte_offset, &response, state);

            response.context_menu(|ui| {
                byte_context_menu(ui, state, input, byte_offset);
            });

            response.on_hover_ui(|ui| {
                render_glyph(ui, &state.settings, Sense::hover(), byte);
                render_offset_info(ui, byte_offset, state.selection_state.selected_window());
                if let Some(mark) = state.marked_locations.mark_at_pos(byte_offset) {
                    ui.separator();
                    hover_marking(ui, mark);
                }
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
            interact_with_offset(ui, byte_offset, &response, state);

            response.context_menu(|ui| {
                byte_context_menu(ui, state, input, byte_offset);
            });

            response.on_hover_ui(|ui| {
                render_hex(ui, &state.settings, Sense::hover(), byte);
                render_offset_info(ui, byte_offset, state.selection_state.selected_window());
                if let Some(mark) = state.marked_locations.mark_at_pos(byte_offset) {
                    ui.separator();
                    hover_marking(ui, mark);
                }
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
    rect.set_width(16.0 * bar_width_multiplier as f32 + 1.0 + SIDE_BAR_WIDTH as f32);

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
        || (),
        |_, x, y| {
            let x = x / bar_width_multiplier;

            if x == 16 {
                Color32::BLACK
            } else if x > 16 {
                let start_offset = (start + y as u64) * 16;
                let alignment = highest_aligned_value(start_offset, start_offset + 16);

                state
                    .settings
                    .alignment_marker_color(AbsoluteOffset::from(alignment))
                    .unwrap_or(Color32::BLACK)
            } else if let Some(&byte) = window.get(y * 16 + x) {
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
