//! Renders hexdumps in the GUI.

use std::ops::RangeInclusive;

use egui::{Align, Color32, Layout, Sense, Ui, UiBuilder, Vec2};
use selection::SelectionContext;

use crate::{data::DataSource, gui::color};

mod inspector;
mod primitives;
mod selection;

use inspector::render_inspector;

pub use primitives::{render_glyph, render_hex, render_offset};

use super::{cached_image::CachedImage, settings::Settings};

/// A hexdump viewer widget.
///
/// Contains the context necessary to render a hexdump.
pub struct HexdumpView {
    /// The number of rows that have been scrolled down from the start offset.
    scroll_offset: u64,
    /// The selection context of the hexview.
    selection_context: SelectionContext,
    /// The cached image for the sidebar in the hex view.
    sidebar_cached_image: CachedImage<(u64, u64)>,
}

impl HexdumpView {
    /// Create a new hexdump context.
    pub fn new() -> HexdumpView {
        HexdumpView {
            scroll_offset: 0,
            selection_context: SelectionContext::new(),
            sidebar_cached_image: CachedImage::new(),
        }
    }

    /// Renders a hexdump to the GUI.
    pub fn render(
        &mut self,
        ui: &mut Ui,
        settings: &Settings,
        source: &mut impl DataSource,
        big_endian: &mut bool,
        start: u64,
    ) {
        // start is in rows
        let start_in_bytes = start * 16;

        let rect = ui.max_rect().intersect(ui.cursor());
        let height = ui.available_height();
        let window_size = height.trunc() as u64 * 16;
        let rows_onscreen = (height / settings.char_height()).trunc() as u64;

        let mut buf = vec![0; window_size as usize];

        let file_size = source.len();

        if let Ok(window) = source.window_at(start_in_bytes, &mut buf) {
            let file_size = file_size.unwrap_or_else(|_| start_in_bytes + window.len() as u64);

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
                    self.render_sidebar(ui, window, rows_onscreen, max_scroll, start, settings);
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::ZERO;
                        self.selection_context.render_selection(
                            ui,
                            file_size,
                            start + self.scroll_offset,
                            rows_onscreen,
                            settings,
                        );

                        for (i, row) in window
                            .chunks(16)
                            .enumerate()
                            .skip(self.scroll_offset as usize)
                            .take(rows_onscreen as usize + 1)
                        {
                            self.render_row(
                                ui,
                                start_in_bytes + (i as u64 * 16),
                                row,
                                file_size,
                                settings,
                            );
                        }
                    });
                    let mut selected_buf;
                    let selected_buf = if let Some(selection) = self.selection() {
                        selected_buf = vec![0; selection.clone().count()];
                        source.window_at(*selection.start(), &mut selected_buf).ok()
                    } else {
                        None
                    };
                    render_inspector(ui, selected_buf, big_endian, settings);
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
    fn render_row(
        &mut self,
        ui: &mut Ui,
        offset: u64,
        row: &[u8],
        file_size: u64,
        settings: &Settings,
    ) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;

            // offset
            render_offset(ui, settings, Sense::hover(), offset).on_hover_ui(|ui| {
                let percentage = offset as f64 / file_size as f64 * 100.0;
                ui.label(format!("{percentage:.02}% of file"));
            });
            ui.add_space(settings.large_space());

            // hex values
            for (i, &byte) in row.iter().enumerate() {
                if i == 8 {
                    ui.add_space(settings.small_space());
                }

                let byte_offset = offset + i as u64;

                let response = render_hex(ui, settings, Sense::hover(), byte);
                self.selection_context
                    .handle_selection(ui.ctx(), &response, byte_offset);

                response.on_hover_ui(|ui| {
                    render_glyph(ui, settings, Sense::hover(), byte);
                });

                if i < 15 {
                    ui.add_space(settings.small_space());
                }
            }

            // ensure non-full rows are still aligned
            if row.len() < 16 {
                let mut space = 0.0;
                // add the separator in the middle
                if row.len() < 9 {
                    space += settings.small_space();
                }

                // add space for the characters
                space += (16 - row.len()) as f32 * settings.char_width() * 2.0;

                // add space between the characters
                space += (15 - row.len()) as f32 * settings.small_space();

                ui.add_space(space);
            }

            ui.add_space(settings.large_space());

            for (i, &byte) in row.iter().enumerate() {
                if i == 8 {
                    ui.add_space(settings.small_space());
                }

                let byte_offset = offset + i as u64;

                let response = render_glyph(ui, settings, Sense::click(), byte);
                self.selection_context
                    .handle_selection(ui.ctx(), &response, byte_offset);

                response.on_hover_ui(|ui| {
                    render_hex(ui, settings, Sense::hover(), byte);
                });
            }
        });
    }

    /// Shows a "minimap" of the hexview to show the context around it.
    fn render_sidebar(
        &mut self,
        ui: &mut Ui,
        window: &[u8],
        rows_onscreen: u64,
        max_scroll: u64,
        start: u64,
        settings: &Settings,
    ) {
        let bar_width_multiplier = settings.bar_width_multiplier();

        let mut rect = ui.max_rect().intersect(ui.cursor());
        rect.set_width(16.0 * bar_width_multiplier as f32);

        let response = ui.allocate_rect(rect, Sense::click_and_drag());

        if let Some(pos) = response.interact_pointer_pos() {
            self.scroll_offset = (pos.y.round() as u64).saturating_sub(rows_onscreen / 2);

            if self.scroll_offset > max_scroll {
                self.scroll_offset = max_scroll;
            }
        }

        let highlight_row_range = self.scroll_offset..self.scroll_offset + rows_onscreen;

        self.sidebar_cached_image
            .paint_at(ui, rect, (start, self.scroll_offset), |x, y| {
                let x = x / bar_width_multiplier;
                if let Some(&byte) = window.get(y * 16 + x) {
                    if highlight_row_range.contains(&(y as u64)) {
                        settings.byte_color(byte)
                    } else {
                        color::lerp(settings.byte_color(byte), Color32::BLACK, 0.5)
                    }
                } else {
                    Color32::TRANSPARENT
                }
            });
    }
}
