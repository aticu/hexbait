//! Implements zoombars to zoom in on the content of a file.

use std::ops::RangeInclusive;

use egui::{Align, Context, Layout, PointerButton, Rect, Ui, UiBuilder, vec2};

use crate::data::DataSource;

use super::{cached_image::CachedImage, color, settings::Settings};

const NULL_SELECTION: RangeInclusive<f32> = 0.0..=1.0;

/// Zoombars are a GUI component to narrow in on parts of a file.
pub struct Zoombars {
    /// Whether or not a selection is in progress.
    selecting: bool,
    /// The zoombars to render.
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
        settings: &Settings,
        render_hex: impl FnOnce(&mut Ui, &mut Source, u64),
        render_overview: impl FnOnce(&mut Ui, &mut Source, RangeInclusive<u64>),
    ) {
        let rect = ui.max_rect().intersect(ui.cursor());
        let total_rows = (rect.height().trunc() as u64).max(1);
        let total_bytes = total_rows * 16;

        if total_bytes >= file_size {
            render_hex(ui, source, 0);
            return;
        } else if self.bars.is_empty() {
            self.bars.push(Zoombar::new());
        }

        let maximum_min_selection_size = (total_rows - 1) as f32 / total_rows as f32;

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
                    rect.set_width(16.0 * settings.bar_width_multiplier() as f32);
                    let rect = rect;

                    let mut selecting = self.selecting && is_second_last;
                    let was_selecting = selecting;

                    let range_len = file_range.clone().count() as u64 - 1;
                    if range_len <= total_bytes {
                        show_hex = true;
                        break;
                    }

                    let min_selection_size =
                        (total_bytes as f32 / range_len as f32).min(maximum_min_selection_size);

                    bar.render(
                        ui,
                        rect,
                        &mut selecting,
                        file_range.clone(),
                        min_selection_size,
                        file_size,
                        settings,
                    );

                    let min_selection_size =
                        (total_bytes as f32 / range_len as f32).min(maximum_min_selection_size);
                    let selection = bar.selection(min_selection_size);

                    let new_start_offset = (range_len as f32 * selection.start()) as u64;

                    let start = *file_range.start() + new_start_offset;
                    let selection_size =
                        ((selection.end() - selection.start()) as f64 * range_len as f64) as u64;

                    file_range = start..=std::cmp::min(start + selection_size, file_size);

                    if !was_selecting && selecting {
                        self.selecting = true;
                        self.bars.truncate(i + 1);
                        self.bars.push(Zoombar::new());
                        break;
                    } else if !selecting && is_second_last {
                        self.selecting = false;
                    }
                }

                // keep bars consistent in case of double clicks
                let last_bar = self.bars.len() - 1;
                for (i, bar) in self.bars.iter_mut().enumerate() {
                    if bar.selected == NULL_SELECTION && i != last_bar {
                        // remove other bars behind this one
                        self.bars.truncate(i + 1);
                        break;
                    }
                }

                if show_hex {
                    let raw_start_in_bytes = *file_range.start();
                    let raw_end_in_bytes = *file_range.end();
                    let start = if raw_start_in_bytes == 0 {
                        // ensure that the correction below does not make the start invisible

                        0
                    } else if raw_end_in_bytes > file_size - 16 {
                        // over-correct towards the end to ensure it's guaranteed to be visible

                        let rounded_up_size = if file_size % 16 == 0 {
                            file_size
                        } else {
                            file_size - (file_size % 16) + 16
                        };

                        (rounded_up_size - total_bytes) / 16
                    } else {
                        raw_start_in_bytes / 16
                    };

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
    /// A cached image of the zoombar.
    cached_image: CachedImage<(RangeInclusive<f32>, RangeInclusive<u64>)>,
}

impl Zoombar {
    /// Creates a new zoombar.
    fn new() -> Zoombar {
        Zoombar {
            selected: NULL_SELECTION,
            dragging: false,
            cached_image: CachedImage::new(),
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

                    // double click resets selection
                    if input.pointer.button_double_clicked(PointerButton::Primary) {
                        self.selected = NULL_SELECTION;
                    }
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

            if let Some(pos) = input.pointer.latest_pos()
                && rect.contains(pos)
                && input.smooth_scroll_delta.y != 0.0
            {
                let raw_scroll_delta = input.smooth_scroll_delta.y;
                let scroll_delta = -raw_scroll_delta / 500.0;

                let selection = self.selection(min_selection_size);
                let selection_size = *selection.end() - *selection.start();
                let tentative_start = *selection.start() + scroll_delta;
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
        settings: &Settings,
    ) {
        let total_points = rect.height().ceil() as u64 * 16;

        self.handle_selection(rect, selecting, min_selection_size, ui.ctx());

        let selection = self.selection(min_selection_size);
        let selection_start = (rect.height() * *selection.start()).trunc() as usize;
        let selection_end = (rect.height() * *selection.end()).trunc() as usize;

        let size = rect.width().trunc() as usize / 16;

        self.cached_image.paint_at(
            ui,
            rect,
            (self.selection(min_selection_size), file_range.clone()),
            |x, y| {
                let x = x / size;

                let relative_offset = (y * 16 + x) as f64 / total_points as f64;
                let start = *file_range.start();
                let range_len = file_range.clone().count();
                let offset_within_range = (relative_offset * range_len as f64) as u64;
                let offset_within_file = (start + offset_within_range) as f64 / file_size as f64;
                let raw_color = settings.scale_color_f32(offset_within_file as f32);

                if selection_start <= y && y <= selection_end {
                    raw_color
                } else {
                    color::lerp(raw_color, egui::Color32::BLACK, 0.5)
                }
            },
        );
        ui.advance_cursor_after_rect(rect);
    }
}
