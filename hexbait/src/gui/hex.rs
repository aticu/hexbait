//! Renders hexdumps in the GUI.

use egui::{Align, Color32, Layout, Rect, ScrollArea, Sense, Ui, UiBuilder, Vec2, vec2};
use hexbait_lang::{View, ir::File};
use highlighting::highlight;
use selection::SelectionContext;

use crate::{data::DataSource, gui::color, model::Endianness, state::Settings, window::Window};

pub mod highlighting;
mod inspector;
mod parse_result;
mod primitives;
mod selection;

use inspector::render_inspector;

pub use primitives::{render_glyph, render_hex, render_offset};

use super::{
    cached_image::CachedImage,
    marking::{MarkedLocation, MarkedLocations, MarkingKind, render_locations_on_bar},
};

/// A hexdump viewer widget.
///
/// Contains the context necessary to render a hexdump.
pub struct HexdumpView {
    /// The number of rows that have been scrolled down from the start offset.
    scroll_offset: u64,
    /// The selection context of the hexview.
    selection_context: SelectionContext,
    /// The cached image for the sidebar in the hex view.
    sidebar_cached_image: CachedImage<(u64, u64, bool)>,
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
        endianness: &mut Endianness,
        start: u64,
        (parse_type, parse_offset): (Option<&File>, &mut Option<u64>),
        marked_locations: &mut MarkedLocations,
    ) {
        // start is in rows
        let start_in_bytes = start * 16;

        let rect = ui.max_rect().intersect(ui.cursor());
        let height = ui.available_height();
        let window_size = height.trunc() as u64 * 16;
        let rows_onscreen = (height / settings.char_height()).trunc() as u64;

        // add 16 more to show one row "beyond the screen"
        let mut buf = vec![0; window_size as usize + 16];

        let file_size = source.len();

        if let Ok(window) = source.window_at(start_in_bytes, &mut buf) {
            let file_size = file_size.unwrap_or_else(|_| start_in_bytes + window.len() as u64);

            let hex_rect_width = (16 * settings.bar_width_multiplier()) as f32
                + ui.spacing().item_spacing.x
                + ((16 + 32 + 16) as f32 * settings.char_width())
                + (2.0 * settings.large_space())
                + (17.0 * settings.small_space());

            let scroll_rect = rect.with_max_x(rect.min.x + hex_rect_width);

            // determine how many rows we can at most scroll down
            let max_height = (window.len() as u64).min(window_size).div_ceil(16);
            let max_scroll = max_height.saturating_sub(rows_onscreen);

            // handle scrolling centrally here
            if ui.rect_contains_pointer(scroll_rect) {
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
                    let max_rect = ui.max_rect();

                    self.render_sidebar(
                        ui,
                        window,
                        rows_onscreen,
                        max_scroll,
                        start,
                        marked_locations,
                        settings,
                    );

                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::ZERO;

                        marked_locations
                            .remove_where(|location| location.kind() == MarkingKind::Selection);
                        if let Some(selection) = self.selection_context.selection() {
                            marked_locations.add(MarkedLocation::new(
                                selection.into(),
                                MarkingKind::Selection,
                            ));
                        }

                        for location in marked_locations.iter_window(Window::from_start_len(
                            start_in_bytes,
                            window.len() as u64,
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
                                start + self.scroll_offset,
                                rows_onscreen,
                                settings,
                            );
                        }

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
                        selected_buf = vec![0; selection.size() as usize];
                        source.window_at(selection.start(), &mut selected_buf).ok()
                    } else {
                        None
                    };

                    // TODO: handle case where this is too small
                    let rest_rect = max_rect.intersect(ui.cursor());
                    let half_height = rest_rect.height() / 2.0;

                    let top_rect =
                        Rect::from_min_size(rest_rect.min, vec2(rest_rect.width(), half_height));

                    let bottom_rect = Rect::from_min_size(
                        rest_rect.min + vec2(0.0, half_height),
                        vec2(rest_rect.width(), half_height),
                    );

                    ui.allocate_new_ui(
                        UiBuilder::new()
                            .max_rect(top_rect)
                            .layout(Layout::left_to_right(Align::Min)),
                        |ui| {
                            ScrollArea::vertical()
                                .id_salt("inspector_scroll")
                                .max_height(half_height)
                                .show(ui, |ui| {
                                    ui.vertical(|ui| {
                                        render_inspector(ui, selected_buf, endianness, settings);
                                    });
                                });
                        },
                    );

                    ui.allocate_new_ui(
                        UiBuilder::new()
                            .max_rect(bottom_rect)
                            .layout(Layout::left_to_right(Align::Min)),
                        |ui| {
                            ScrollArea::both()
                                .id_salt("parser_scroll")
                                .max_height(half_height)
                                .show(ui, |ui| {
                                    marked_locations.remove_where(|location| {
                                        location.kind() == MarkingKind::HoveredParsed
                                    });

                                    let current_parse_offset = *parse_offset;
                                    if let Some(window) = self.selection() {
                                        *parse_offset = Some(window.start());
                                    }
                                    let Some(parse_offset) = current_parse_offset else {
                                        return;
                                    };

                                    let Some(parse_type) = parse_type else { return };
                                    let Ok(view) = source.as_view() else {
                                        eprintln!("TODO: implement better error handling");
                                        return;
                                    };
                                    let view = View::Subview {
                                        view: &view,
                                        valid_range: parse_offset..view.len(),
                                    };
                                    let result = hexbait_lang::eval_ir(&parse_type, view, 0);
                                    let hovered = parse_result::show_value(
                                        ui,
                                        hexbait_lang::ir::path::Path::new(),
                                        None,
                                        &result.value,
                                        settings,
                                    );

                                    if let Some(hovered) = hovered
                                        && let Some(value) = result.value.subvalue_at_path(&hovered)
                                    {
                                        for range in value.provenance.byte_ranges() {
                                            marked_locations.add(MarkedLocation::new(
                                                range.into(),
                                                MarkingKind::HoveredParsed,
                                            ));
                                        }
                                    }
                                });
                        },
                    );
                },
            );

            let copy_event =
                ui.input(|input| input.events.iter().any(|event| *event == egui::Event::Copy));

            if copy_event
                && let Some(selection) = self.selection()
                && let Ok(size) = usize::try_from(selection.size())
            {
                let mut buf = vec![0; size];
                if let Ok(window) = source.window_at(selection.start(), &mut buf) {
                    if let Ok(as_text) = std::str::from_utf8(window) {
                        ui.ctx().copy_text(as_text.to_string());
                    }
                }
            }
        } else {
            ui.label("hex display is experiencing issues");
            ui.spinner();
        }
    }

    /// Returns the current selection.
    fn selection(&self) -> Option<Window> {
        self.selection_context
            .selection()
            .map(|selection| Window::new(*selection.start(), *selection.end() + 1))
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

            let render_offset_info = |ui: &mut Ui, byte_offset: u64, selection: Option<Window>| {
                ui.label(format!(
                    "offset from file start: 0x{byte_offset:x} ({byte_offset})"
                ));
                if let Some(selection) = selection {
                    let selection_offset = byte_offset as i64 - selection.start() as i64;
                    ui.label(format!(
                        "offset from selection start: {}0x{:x} ({selection_offset})",
                        if selection_offset < 0 { "-" } else { "" },
                        selection_offset.abs()
                    ));
                }
            };

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

                let response = render_hex(ui, settings, Sense::hover(), byte, settings.hex_font());
                self.selection_context
                    .handle_selection(ui.ctx(), &response, byte_offset);

                response.on_hover_ui(|ui| {
                    render_glyph(ui, settings, Sense::hover(), byte);
                    render_offset_info(ui, byte_offset, self.selection());
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
                    render_hex(ui, settings, Sense::hover(), byte, settings.hex_font());
                    render_offset_info(ui, byte_offset, self.selection());
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
        marked_locations: &mut MarkedLocations,
        settings: &Settings,
    ) {
        let bar_width_multiplier = settings.bar_width_multiplier();

        let mut rect = ui.max_rect().intersect(ui.cursor());
        rect.set_width(16.0 * bar_width_multiplier as f32);

        let num_rows = (window.len() + 15) / 16;
        rect.set_height(rect.height().min(num_rows as f32));

        let response = ui.allocate_rect(rect, Sense::click_and_drag());

        if let Some(pos) = response.interact_pointer_pos() {
            self.scroll_offset =
                ((pos.y - rect.min.y).round() as u64).saturating_sub(rows_onscreen / 2);

            if self.scroll_offset > max_scroll {
                self.scroll_offset = max_scroll;
            }
        }

        let highlight_row_range = self.scroll_offset..self.scroll_offset + rows_onscreen;

        self.sidebar_cached_image.paint_at(
            ui,
            rect,
            (start, self.scroll_offset, settings.linear_byte_colors()),
            |x, y| {
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
            },
        );

        let mut new_hovered_location = None;
        let currently_hovered = marked_locations.hovered_location_mut().clone();
        render_locations_on_bar(
            ui,
            rect,
            Window::from_start_len(start * 16, window.len() as u64),
            marked_locations,
            &mut new_hovered_location,
            currently_hovered,
        );
        *marked_locations.hovered_location_mut() = new_hovered_location;
    }
}

impl Default for HexdumpView {
    fn default() -> Self {
        HexdumpView::new()
    }
}
