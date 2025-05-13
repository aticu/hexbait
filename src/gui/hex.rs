//! Renders hexdumps in the GUI.

use std::ops::RangeInclusive;

use egui::{Align, Color32, Layout, Rect, Sense, Ui, UiBuilder, Vec2, vec2};
use primitives::{char_height, char_width, large_space, small_space};
use selection::SelectionContext;

use crate::{
    data::DataSource,
    gui::color::{self, BYTE_COLORS},
};

mod inspector;
mod primitives;
mod selection;

use inspector::render_inspector;

pub use primitives::{render_glyph, render_hex, render_offset};

/// A hexdump viewer widget.
///
/// Contains the context necessary to render a hexdump.
pub struct HexdumpView {
    /// The row number at which the hexdump should start.
    start: u64,
    /// The number of rows that have been scrolled down from the start offset.
    scroll_offset: u64,
    /// The selection context of the hexview.
    selection_context: SelectionContext,
}

impl HexdumpView {
    /// Create a new hexdump context.
    pub fn new() -> HexdumpView {
        HexdumpView {
            start: 0,
            scroll_offset: 0,
            selection_context: SelectionContext::new(),
        }
    }

    /// Renders a hexdump to the GUI.
    pub fn render(&mut self, ui: &mut Ui, source: &mut impl DataSource, big_endian: &mut bool) {
        let scale = 20.0;
        // start is in rows
        let start = self.start * 16;

        let rect = ui.max_rect().intersect(ui.cursor());
        let height = ui.available_height();
        let window_size = height.trunc() as u64 * 16;
        let rows_onscreen = (height / char_height(scale)).trunc() as u64;

        let mut buf = vec![0; window_size as usize];

        let file_size = source.len();

        if let Ok(window) = source.window_at(start, &mut buf) {
            let file_size = file_size.unwrap_or_else(|_| start + window.len() as u64);

            // determine how many rows we can at most scroll down
            let max_scroll = (window.len().div_ceil(16) as u64).saturating_sub(rows_onscreen);

            // handle scrolling centrally here
            if ui.rect_contains_pointer(rect) {
                let raw_scroll_delta = ui.ctx().input(|input| input.smooth_scroll_delta).y;
                let scroll_delta = (-raw_scroll_delta / 2.0).trunc() as i64;
                if scroll_delta < 0 {
                    self.scroll_offset =
                        (self.scroll_offset).saturating_sub((-scroll_delta) as u64);
                } else {
                    self.scroll_offset = (self.scroll_offset).saturating_add(scroll_delta as u64);
                }
            }

            // ensure that nothing scrolls too far
            if self.scroll_offset > max_scroll {
                self.scroll_offset = max_scroll;
            }

            self.selection_context
                .check_for_selection_process_end(ui.ctx());

            ui.allocate_new_ui(
                UiBuilder::new()
                    .max_rect(rect)
                    .layout(Layout::left_to_right(Align::Min)),
                |ui| {
                    self.render_sidebar(ui, window, rows_onscreen, max_scroll);
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::ZERO;
                        self.selection_context.render_selection(
                            ui,
                            file_size,
                            self.start + self.scroll_offset,
                            rows_onscreen,
                            scale,
                        );

                        for (i, row) in window
                            .chunks(16)
                            .enumerate()
                            .skip(self.scroll_offset as usize)
                            .take(rows_onscreen as usize + 1)
                        {
                            self.render_row(ui, start + (i as u64 * 16), row, file_size, scale);
                        }
                    });
                    let mut selected_buf;
                    let selected_buf = if let Some(selection) = self.selection() {
                        selected_buf = vec![0; selection.clone().count()];
                        source.window_at(*selection.start(), &mut selected_buf).ok()
                    } else {
                        None
                    };
                    render_inspector(ui, selected_buf, big_endian, scale);
                },
            );
        } else {
            ui.label("hex display is experiencing issues");
            ui.spinner();
        }
    }

    /// Returns the current selection.
    fn selection(&self) -> Option<RangeInclusive<u64>> {
        self.selection_context.selection()
    }

    /// Renders a single row in a hexdump.
    fn render_row(&mut self, ui: &mut Ui, offset: u64, row: &[u8], file_size: u64, scale: f32) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;

            // offset
            render_offset(ui, scale, Sense::hover(), offset).on_hover_ui(|ui| {
                let percentage = offset as f64 / file_size as f64 * 100.0;
                ui.label(format!("{percentage:.02}% of file"));
            });
            ui.add_space(large_space(scale));

            // hex values
            for (i, &byte) in row.iter().enumerate() {
                if i == 8 {
                    ui.add_space(small_space(scale));
                }

                let byte_offset = offset + i as u64;

                let response = render_hex(ui, scale, Sense::hover(), byte);
                self.selection_context
                    .handle_selection(ui.ctx(), &response, byte_offset);

                response.on_hover_ui(|ui| {
                    render_glyph(ui, scale, Sense::hover(), byte);
                });

                if i < 15 {
                    ui.add_space(small_space(scale));
                }
            }

            // ensure non-full rows are still aligned
            if row.len() < 16 {
                let mut space = 0.0;
                // add the separator in the middle
                if row.len() < 9 {
                    space += small_space(scale);
                }

                // add space for the characters
                space += (16 - row.len()) as f32 * char_width(scale) * 2.0;

                // add space between the characters
                space += (15 - row.len()) as f32 * small_space(scale);

                ui.add_space(space);
            }

            ui.add_space(large_space(scale));

            for (i, &byte) in row.iter().enumerate() {
                if i == 8 {
                    ui.add_space(small_space(scale));
                }

                let byte_offset = offset + i as u64;

                let response = render_glyph(ui, scale, Sense::click(), byte);
                self.selection_context
                    .handle_selection(ui.ctx(), &response, byte_offset);

                response.on_hover_ui(|ui| {
                    render_hex(ui, scale, Sense::hover(), byte);
                });
            }
        });
    }

    /// Shows a "minimap" of the hexview to show the context around it.
    fn render_sidebar(&mut self, ui: &mut Ui, window: &[u8], rows_onscreen: u64, max_scroll: u64) {
        let size = 3.0;

        let mut rect = ui.max_rect().intersect(ui.cursor());
        rect.set_width(16.0 * size);

        let response = ui.allocate_rect(rect, Sense::click_and_drag());

        if let Some(pos) = response.interact_pointer_pos() {
            self.scroll_offset = (pos.y.round() as u64).saturating_sub(rows_onscreen / 2);

            if self.scroll_offset > max_scroll {
                self.scroll_offset = max_scroll;
            }
        }

        let highlight_row_range = self.scroll_offset..self.scroll_offset + rows_onscreen;

        for (i, &byte) in window.iter().enumerate() {
            let row = i / 16;
            let rect = Rect::from_min_size(
                rect.min + vec2((i % 16) as f32 * size, row as f32),
                vec2(size, 1.0),
            );
            let color = if highlight_row_range.contains(&(row as u64)) {
                BYTE_COLORS[byte as usize]
            } else {
                color::lerp(BYTE_COLORS[byte as usize], Color32::BLACK, 0.5)
            };

            ui.painter()
                .with_clip_rect(rect)
                .rect_filled(rect, 0.0, color);
        }
    }
}
