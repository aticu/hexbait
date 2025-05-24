//! Implements zoombars to zoom in on the content of a file.

use std::ops::RangeInclusive;

use egui::{Align, Context, Layout, Rect, Ui, UiBuilder, vec2};

use crate::data::DataSource;

use super::color;

const NULL_SELECTION: RangeInclusive<f32> = 0.0..=1.0;

/// Zoombars are a GUI component to narrow in on parts of a file.
pub struct Zoombars {
    /// Whether or not a selection is in progress.
    selecting: bool,
    bars: Vec<Zoombar>,
}

impl Zoombars {
    /// Creates new zoombars.
    pub fn new() -> Zoombars {
        Zoombars {
            selecting: false,
            bars: Vec::new(),
        }
    }

    /// Renders the zoombars.
    pub fn render<Source: DataSource>(
        &mut self,
        ui: &mut Ui,
        file_size: u64,
        source: &mut Source,
        render_hex: impl FnOnce(&mut Ui, &mut Source, u64),
        render_overview: impl FnOnce(&mut Ui, &mut Source, RangeInclusive<u64>),
    ) {
        let size = 3.0;

        let rect = ui.max_rect().intersect(ui.cursor());
        let total_rows = rect.height().trunc() as u64;
        let total_points = total_rows * 16;

        if total_points >= file_size {
            // TODO: calculate start correctly here
            render_hex(ui, source, 0);
            return;
        } else if self.bars.is_empty() {
            self.bars.push(Zoombar::new());
        }

        let mut file_range = 0..=file_size;
        let mut show_hex = false;

        ui.allocate_new_ui(
            UiBuilder::new()
                .max_rect(rect)
                .layout(Layout::left_to_right(Align::Min)),
            |ui| {
                let last_bar = self.bars.len() - 1;
                for (i, bar) in self.bars.iter_mut().enumerate() {
                    let is_second_last = i + 1 == last_bar;

                    let mut rect = ui.max_rect().intersect(ui.cursor());
                    rect.set_width(16.0 * size);
                    let rect = rect;

                    let mut selecting = self.selecting && is_second_last;
                    let was_selecting = selecting;

                    let range_len = file_range.clone().count();
                    let min_selection_size = total_rows as f32 / range_len as f32;
                    if (min_selection_size - 1.0).abs() < 0.005 {
                        show_hex = true;
                        break;
                    }

                    bar.render(
                        ui,
                        rect,
                        &mut selecting,
                        file_range.clone(),
                        min_selection_size,
                        file_size,
                    );
                    let range_len = file_range.clone().count();
                    let min_selection_size = total_rows as f32 / range_len as f32;
                    let new_start_offset =
                        (range_len as f32 * bar.selection(min_selection_size).start()) as u64;
                    let new_end_offset =
                        (range_len as f32 * bar.selection(min_selection_size).end()) as u64;
                    file_range = *file_range.start() + new_start_offset
                        ..=*file_range.start() + new_end_offset;

                    if !was_selecting && selecting {
                        self.selecting = true;
                        self.bars.truncate(i + 1);
                        self.bars.push(Zoombar::new());
                        break;
                    } else if !selecting && is_second_last {
                        self.selecting = false;
                    }
                }

                if show_hex {
                    // TODO: be less naive here and ensure that the end is actually guaranteed to
                    // be visible
                    let start = *file_range.start() / 16;
                    render_hex(ui, source, start);
                } else {
                    render_overview(ui, source, file_range);
                }
            },
        );
    }
}

/// Represents a single zoombar.
struct Zoombar {
    /// The selected range of the bar.
    selected: RangeInclusive<f32>,
    /// Whether or not the user is currently dragging the selection.
    dragging: bool,
}

impl Zoombar {
    /// Creates a new zoombar.
    fn new() -> Zoombar {
        Zoombar {
            selected: NULL_SELECTION,
            dragging: false,
        }
    }

    /// The selection of this zoombar.
    fn selection(&self, min_selection_size: f32) -> RangeInclusive<f32> {
        let size = (*self.selected.start() - *self.selected.end())
            .abs()
            .clamp(min_selection_size, 1.0);

        if self.selected != NULL_SELECTION {
            let start = if self.selected.start() > self.selected.end() {
                (*self.selected.start() - size).min(*self.selected.end())
            } else {
                *self.selected.start()
            };

            if start < 0.0 {
                0.0..=size
            } else if start + size > 1.0 {
                1.0 - size..=1.0
            } else {
                start..=start + size
            }
        } else {
            NULL_SELECTION
        }
    }

    /// Handles manipulating the selection on the zoombar.
    fn handle_selection(
        &mut self,
        rect: Rect,
        selecting: &mut bool,
        min_selection_size: f32,
        ctx: &Context,
    ) {
        ctx.input(|input| {
            if *selecting {
                if let Some(pos) = input.pointer.latest_pos()
                    && rect.expand2(vec2(f32::INFINITY, 0.0)).contains(pos)
                {
                    let start = *self.selected.start();
                    let current = (pos.y - rect.min.y) / rect.height();
                    self.selected = start..=current;
                }
                if !input.pointer.primary_down() {
                    *selecting = false;
                }
            } else if input.pointer.primary_pressed()
                && let Some(pos) = input.pointer.latest_pos()
                && rect.contains(pos)
                && !self.dragging
            {
                *selecting = true;
                let current = (pos.y - rect.min.y) / rect.height();
                self.selected = current..=current;
            } else if self.dragging {
                if let Some(pos) = input.pointer.latest_pos()
                    && rect.expand2(vec2(f32::INFINITY, 0.0)).contains(pos)
                {
                    let selection = self.selection(min_selection_size);

                    let current = (pos.y - rect.min.y) / rect.height();
                    let selection_size = *selection.end() - *selection.start();
                    let tentative_start = current - (selection_size / 2.0);
                    let new_start = if tentative_start >= 0.0 {
                        if tentative_start + selection_size <= 1.0 {
                            tentative_start
                        } else {
                            1.0 - selection_size
                        }
                    } else {
                        0.0
                    };
                    self.selected = new_start..=new_start + selection_size;
                }
                if !input.pointer.secondary_down() {
                    self.dragging = false;
                }
            } else if input.pointer.secondary_pressed()
                && let Some(pos) = input.pointer.latest_pos()
                && rect.contains(pos)
                && !*selecting
            {
                self.dragging = true;
            }
        });
    }

    /// Renders a single zoombar.
    fn render(
        &mut self,
        ui: &mut Ui,
        rect: Rect,
        selecting: &mut bool,
        file_range: RangeInclusive<u64>,
        min_selection_size: f32,
        file_size: u64,
    ) {
        let total_points = rect.height().ceil() as u64 * 16;

        let selection = self.selection(min_selection_size);
        let selection_start = (rect.height() * *selection.start()).trunc() as u64;
        let selection_end = (rect.height() * *selection.end()).trunc() as u64;

        self.handle_selection(rect, selecting, min_selection_size, ui.ctx());

        let size = rect.width() / 16.0;
        for y in 0..rect.height().ceil() as u64 {
            for x in 0..16 {
                let rect = egui::Rect::from_min_size(
                    rect.min + egui::vec2(x as f32 * size, y as f32),
                    egui::vec2(size, 1.0),
                );

                let relative_offset = (y * 16 + x) as f64 / total_points as f64;
                let start = *file_range.start();
                let range_len = file_range.clone().count();
                let offset_within_range = (relative_offset * range_len as f64) as u64;
                let offset_within_file = (start + offset_within_range) as f64 / file_size as f64;
                let raw_color =
                    color::VIRIDIS[(offset_within_file.clamp(0.0, 1.0) * 255.0) as usize];

                let color = if selection_start <= y && y <= selection_end {
                    raw_color
                } else {
                    color::lerp(raw_color, egui::Color32::BLACK, 0.5)
                };

                ui.painter()
                    .with_clip_rect(rect)
                    .rect_filled(rect, 0.0, color);
            }
        }

        ui.advance_cursor_after_rect(rect);
    }
}
