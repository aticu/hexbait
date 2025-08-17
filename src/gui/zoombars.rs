//! Implements zoombars to zoom in on the content of a file.

use std::{collections::HashMap, ops::RangeInclusive};

use egui::{
    Align, Color32, Context, FontId, Layout, PointerButton, Pos2, Rect, RichText, Sense, Ui,
    UiBuilder, show_tooltip_at_pointer, vec2,
};

use crate::{data::DataSource, window::Window};

use super::{
    cached_image::CachedImage,
    color,
    marking::{MarkedLocations, render_locations_on_bar},
    settings::Settings,
};

const NULL_SELECTION: RangeInclusive<f32> = 0.0..=1.0;

/// Zoombars are a GUI component to narrow in on parts of a file.
pub struct Zoombars {
    /// Whether or not a selection is in progress.
    selecting: bool,
    /// The zoombars to render.
    bars: Vec<Zoombar>,
}

// TODO: pretty this part up
fn entropy(source: &mut impl DataSource, window: Window) -> Option<f32> {
    let mut buf = vec![0; window.size() as usize];

    if let Ok(window) = source.window_at(window.start(), &mut buf) {
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
        marked_locations: &mut MarkedLocations,
        render_hex: impl FnOnce(&mut Ui, &mut Source, u64, &mut MarkedLocations),
        render_overview: impl FnOnce(&mut Ui, &mut Source, Window),
    ) {
        let rect = ui.max_rect().intersect(ui.cursor());

        // be deliberately small to fit more text here
        let size_text_height = settings.font_size() * 0.7;

        let total_rows = (rect.height().trunc() as u64).max(1);
        let total_bytes = total_rows * 16;

        if total_bytes >= file_size {
            render_hex(ui, source, 0, marked_locations);
            return;
        } else if self.bars.is_empty() {
            self.bars.push(Zoombar::new());
        }

        let maximum_min_selection_size = (total_rows - 1) as f32 / total_rows as f32;

        let mut window = Window::new(0, file_size);
        let mut show_hex = false;

        let mut entropy_cache = HashMap::new();

        let mut new_hovered_location = None;
        let currently_hovered = *marked_locations.hovered_location_id_mut();

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
                        format!("{}B", size_format::SizeFormatterBinary::new(window.size())),
                        FontId::proportional(size_text_height),
                        ui.style().noninteractive().text_color(),
                    );

                    let mut selecting = self.selecting && is_second_last;
                    let was_selecting = selecting;

                    if window.size() <= total_bytes {
                        show_hex = true;
                        break;
                    }

                    let min_selection_size =
                        (total_bytes as f32 / window.size() as f32).min(maximum_min_selection_size);

                    let hovered_row_window = bar.render(
                        ui,
                        rect,
                        &mut selecting,
                        window,
                        min_selection_size,
                        |row_window| {
                            let row_window = row_window.expand_to_align(1024);
                            let entropy = entropy_cache
                                .entry(row_window)
                                .or_insert_with(|| entropy(source, row_window));

                            if let Some(entropy) = entropy {
                                settings.entropy_color(*entropy)
                            } else {
                                todo!("pick a color to display when the info is not available")
                            }
                        },
                    );

                    render_locations_on_bar(
                        ui,
                        rect,
                        window,
                        marked_locations,
                        &mut new_hovered_location,
                        currently_hovered,
                    );

                    if let Some(location) = marked_locations.hovered() {
                        let offset = location.window().start();
                        show_tooltip_at_pointer(
                            ui.ctx(),
                            ui.layer_id(),
                            "position_highlight_hover".into(),
                            |ui| {
                                ui.label(
                                    RichText::new(format!("{}", offset)).size(settings.font_size()),
                                );
                            },
                        );
                    } else if let Some(row_window) = hovered_row_window {
                        show_tooltip_at_pointer(
                            ui.ctx(),
                            ui.layer_id(),
                            "zoombar_tooltip".into(),
                            |ui| {
                                let row_window = row_window.expand_to_align(1024);
                                let entropy = entropy_cache
                                    .entry(row_window)
                                    .or_insert_with(|| entropy(source, row_window));

                                if let Some(entropy) = entropy {
                                    ui.label(
                                        RichText::new(format!("Entropy: {entropy:.02}"))
                                            .size(settings.font_size()),
                                    );
                                } else {
                                    ui.label(
                                        RichText::new("Entropy unknown").size(settings.font_size()),
                                    );
                                }
                            },
                        );
                    }

                    let min_selection_size =
                        (total_bytes as f32 / window.size() as f32).min(maximum_min_selection_size);
                    let selection = bar.selection(min_selection_size);

                    let new_start_offset = (window.size() as f32 * selection.start()) as u64;

                    let start = window.start() + new_start_offset;
                    let selection_size = ((selection.end() - selection.start()) as f64
                        * window.size() as f64) as u64;

                    window = Window::new(start, std::cmp::min(start + selection_size, file_size));

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
                    let start = if window.start() == 0 {
                        // ensure that the correction below does not make the start invisible

                        0
                    } else if window.end() > file_size - 16 {
                        // over-correct towards the end to ensure it's guaranteed to be visible

                        let rounded_up_size = if file_size % 16 == 0 {
                            file_size
                        } else {
                            file_size - (file_size % 16) + 16
                        };

                        (rounded_up_size - total_bytes) / 16
                    } else {
                        window.start() / 16
                    };

                    render_hex(ui, source, start, marked_locations);
                } else {
                    render_overview(ui, source, window);
                }
            },
        );

        *marked_locations.hovered_location_id_mut() = new_hovered_location;
    }
}

impl Default for Zoombars {
    fn default() -> Self {
        Zoombars::new()
    }
}

/// Represents a single zoombar.
struct Zoombar {
    /// The selected range of the bar.
    selected: RangeInclusive<f32>,
    /// Whether or not the user is currently dragging the selection.
    dragging: bool,
    /// A cached image of the zoombar.
    cached_image: CachedImage<(RangeInclusive<f32>, Window)>,
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
        window: Window,
        min_selection_size: f32,
        mut row_color: impl FnMut(Window) -> Color32,
    ) -> Option<Window> {
        let total_rows = rect.height().trunc() as u64;

        self.handle_selection(rect, selecting, min_selection_size, ui.ctx());

        let selection = self.selection(min_selection_size);
        let selection_start = (rect.height() * *selection.start()).trunc() as usize;
        let selection_end = (rect.height() * *selection.end()).trunc() as usize;

        let bytes_per_row = (window.size() as f64 / total_rows as f64).round() as u64;

        self.cached_image.paint_at(
            ui,
            rect,
            (self.selection(min_selection_size), window),
            |_, y| {
                let relative_offset = y as f64 / total_rows as f64;
                let offset_within_range = (relative_offset * window.size() as f64) as u64;

                let row_window =
                    Window::from_start_len(window.start() + offset_within_range, bytes_per_row);

                let raw_color = row_color(row_window);

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
                let y = (pos.y - rect.min.y) as usize;

                let relative_offset = y as f64 / total_rows as f64;
                let offset_within_range = (relative_offset * window.size() as f64) as u64;

                Window::from_start_len(window.start() + offset_within_range, bytes_per_row)
            })
    }
}

impl Default for Zoombar {
    fn default() -> Self {
        Zoombar::new()
    }
}

/// Returns the position of `offset` on the bar spanning `bar_window` displayed in `bar_rect`.
pub fn offset_on_bar(bar_rect: Rect, bar_window: Window, offset: u64) -> Option<Pos2> {
    if offset < bar_window.start() {
        return None;
    }

    let relative_offset = (offset - bar_window.start()) as f32 / bar_window.size() as f32;
    let height = bar_rect.height().ceil();

    if 0.0 <= relative_offset && relative_offset <= 1.0 {
        let offset = ((16.0 * height) * relative_offset) as u32;
        let offset_x = offset % 16;
        let offset_y = offset / 16;

        Some(bar_rect.min + vec2(offset_x as f32 * bar_rect.width() / 16.0, offset_y as f32))
    } else {
        None
    }
}
