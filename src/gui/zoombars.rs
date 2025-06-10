//! Implements zoombars to zoom in on the content of a file.

use std::{collections::HashMap, ops::RangeInclusive};

use egui::{
    Align, Color32, Context, FontId, Layout, PointerButton, Rect, Sense, Ui, UiBuilder,
    show_tooltip_at_pointer, vec2,
};

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

// TODO: pretty this part up
fn bin_byte_range(range: RangeInclusive<u64>) -> RangeInclusive<u64> {
    const WINDOW_SIZE: usize = 1024;
    let start = *range.start() & !(WINDOW_SIZE as u64 - 1);
    let range_len = range.count() + WINDOW_SIZE & !(WINDOW_SIZE - 1);

    start..=start + range_len as u64
}

fn entropy(source: &mut impl DataSource, range: RangeInclusive<u64>) -> Option<f32> {
    let mut buf = vec![0; (range.end() - range.start()) as usize];

    if let Ok(window) = source.window_at(*range.start(), &mut buf) {
        let mut frequencies = [0usize; 256];

        for &byte in window {
            frequencies[byte as usize] += 1;
        }

        Some(
            -frequencies
                .into_iter()
                .filter(|&count| count != 0)
                .map(|count| {
                    let p = count as f32 / window.len() as f32;
                    p * p.log2()
                })
                .sum::<f32>()
                / 8.0,
        )
    } else {
        None
    }
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

        // be deliberately small to fit more text here
        let size_text_height = settings.font_size() * 0.7;

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

        let mut entropy_cache = HashMap::new();

        ui.allocate_new_ui(
            UiBuilder::new()
                .max_rect(rect)
                .layout(Layout::left_to_right(Align::Min)),
            |ui| {
                let last_bar = self.bars.len() - 1;
                for (i, bar) in self.bars.iter_mut().enumerate() {
                    let is_second_last = i + 1 == last_bar;

                    let mut rect = ui.max_rect().intersect(ui.cursor());
                    rect.min += vec2(0.0, size_text_height);
                    rect.set_width(16.0 * settings.bar_width_multiplier() as f32);
                    let rect = rect;

                    ui.painter().text(
                        rect.min,
                        egui::Align2::LEFT_BOTTOM,
                        format!(
                            "{}B",
                            size_format::SizeFormatterBinary::new(
                                file_range.end() - file_range.start()
                            )
                        ),
                        FontId::proportional(size_text_height),
                        ui.style().noninteractive().text_color(),
                    );

                    let mut selecting = self.selecting && is_second_last;
                    let was_selecting = selecting;

                    let range_len = file_range.clone().count() as u64 - 1;
                    if range_len <= total_bytes {
                        show_hex = true;
                        break;
                    }

                    let min_selection_size =
                        (total_bytes as f32 / range_len as f32).min(maximum_min_selection_size);

                    let hovered_byte_range = bar.render(
                        ui,
                        rect,
                        &mut selecting,
                        file_range.clone(),
                        min_selection_size,
                        |byte_range| {
                            let byte_range = bin_byte_range(byte_range);
                            let entropy = entropy_cache
                                .entry(byte_range.clone())
                                .or_insert_with(|| entropy(source, byte_range));

                            if let Some(entropy) = entropy {
                                settings.entropy_color(*entropy)
                            } else {
                                todo!("pick a color to display when the info is not available")
                            }
                        },
                    );

                    if let Some(byte_range) = hovered_byte_range {
                        show_tooltip_at_pointer(
                            ui.ctx(),
                            ui.layer_id(),
                            "signature_display".into(),
                            |ui| {
                                let byte_range = bin_byte_range(byte_range);
                                let entropy = entropy_cache
                                    .entry(byte_range.clone())
                                    .or_insert_with(|| entropy(source, byte_range));

                                if let Some(entropy) = entropy {
                                    ui.label(format!("Entropy: {entropy:.02}"));
                                } else {
                                    ui.label(format!("Entropy unknown"));
                                }
                            },
                        );
                    }

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
        mut handle_byte_range: impl FnMut(RangeInclusive<u64>) -> Color32,
    ) -> Option<RangeInclusive<u64>> {
        let total_points = rect.height().trunc() as u64 * 16;

        self.handle_selection(rect, selecting, min_selection_size, ui.ctx());

        let selection = self.selection(min_selection_size);
        let selection_start = (rect.height() * *selection.start()).trunc() as usize;
        let selection_end = (rect.height() * *selection.end()).trunc() as usize;

        let size = rect.width().trunc() as usize / 16;

        let range_len = file_range.clone().count();
        let bytes_per_pixel = (range_len as f64 / total_points as f64).round() as u64;

        self.cached_image.paint_at(
            ui,
            rect,
            (self.selection(min_selection_size), file_range.clone()),
            |x, y| {
                let x = x / size;

                let relative_offset = (y * 16 + x) as f64 / total_points as f64;
                let offset_within_range = (relative_offset * range_len as f64) as u64;

                let start_in_file = *file_range.start() + offset_within_range;
                let byte_range = start_in_file..=start_in_file + bytes_per_pixel;

                let raw_color = handle_byte_range(byte_range);

                const HIGHLIGHT_STRENGTH: f64 = 0.4;

                if selection_start <= y && y <= selection_end {
                    //color::lerp(raw_color, egui::Color32::WHITE, HIGHLIGHT_STRENGTH)
                    raw_color
                } else {
                    color::lerp(raw_color, egui::Color32::BLACK, HIGHLIGHT_STRENGTH)
                }
            },
        );

        ui.allocate_rect(rect, Sense::hover())
            .hover_pos()
            .map(|pos| {
                let x = (pos.x - rect.min.x) as usize / size;
                let y = (pos.y - rect.min.y) as usize;

                let relative_offset = (y * 16 + x) as f64 / total_points as f64;
                let offset_within_range = (relative_offset * range_len as f64) as u64;

                let start_in_file = *file_range.start() + offset_within_range;
                let byte_range = start_in_file..=start_in_file + bytes_per_pixel;

                byte_range
            })
    }
}
