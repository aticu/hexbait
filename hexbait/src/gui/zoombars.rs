//! Implements zoombars to zoom in on the content of a file.

use std::{
    hash::{Hash as _, Hasher as _},
    ops::RangeInclusive,
};

use egui::{
    Align, Color32, Context, FontId, Layout, PointerButton, Pos2, Rect, Sense, Ui, UiBuilder,
    show_tooltip_at_pointer, vec2,
};

use crate::{
    IDLE_TIME, data::DataSource, state::Settings, statistics::StatisticsHandler, window::Window,
};

use super::{
    cached_image::CachedImage,
    color,
    marking::{MarkedLocations, render_locations_on_bar},
};

/// The selection of a zoombar that selected nothing.
const NULL_SELECTION: RangeInclusive<f32> = 0.0..=1.0;

/// Zoombars are a GUI component to narrow in on parts of a file.
pub struct Zoombars {
    /// Whether or not a selection is in progress.
    selecting: bool,
    /// The zoombars to render.
    bars: Vec<Zoombar>,
    /// The height of the zoombars in the previous frame.
    prev_height: f32,
    /// The selection state in the previous frame.
    prev_selection_state: u64,
}

impl Zoombars {
    /// Creates new zoombars.
    pub fn new() -> Zoombars {
        Zoombars {
            selecting: false,
            bars: Vec::new(),
            // the previous height is irrelevant for the first frame
            prev_height: 0.0,
            // the initial selection state is irrelevant for the first frame
            prev_selection_state: 0,
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
        handler: &StatisticsHandler,
        render_hex: impl FnOnce(&mut Ui, &mut Source, u64, &mut MarkedLocations),
        render_overview: impl FnOnce(&mut Ui, Window),
    ) {
        let rect = ui.max_rect().intersect(ui.cursor());

        self.prev_height = rect.height();

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

        let mut window = Window::new(0, file_size);
        let mut show_hex = false;

        let mut new_hovered_location = None;
        let currently_hovered = marked_locations.hovered_location_mut().clone();

        let mut tmp_rearrange_flag = false;

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
                    rect.set_width(16.0 * settings.bar_width_multiplier() as f32 + 3.0);
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

                    let min_selection_size = min_selection_size(window, rect.height());

                    let mut require_repaint = false;
                    let hovered_row_window = bar.render(
                        ui,
                        rect,
                        &mut selecting,
                        window,
                        min_selection_size,
                        marked_locations.hovered().is_none(),
                        |row_window| {
                            if let Some((entropy, quality)) = handler
                                .get_entropy(row_window)
                                .into_result_with_quality()
                                .unwrap()
                            {
                                // TODO: investigate why calculations sometimes get stuck
                                if quality < 1.0 {
                                    require_repaint = true;
                                }
                                let color = settings.entropy_color(entropy);
                                let secondary_color = if quality < 1.0 {
                                    Color32::RED
                                } else {
                                    Color32::GREEN
                                };
                                (color, secondary_color)
                            } else {
                                require_repaint = true;
                                let color = settings.missing_color();
                                (color, color)
                            }
                        },
                    );
                    bar.cached_image.require_repaint(require_repaint);

                    if require_repaint {
                        ui.ctx().request_repaint_after(IDLE_TIME);
                    }

                    render_locations_on_bar(
                        ui,
                        rect,
                        window,
                        marked_locations,
                        &mut new_hovered_location,
                        currently_hovered.clone(),
                    );

                    if let Some(location) = marked_locations.hovered()
                        && ui.input(|input| {
                            input
                                .pointer
                                .latest_pos()
                                .map(|pos| rect.contains(pos))
                                .unwrap_or(false)
                        })
                    {
                        let offset = location.window().start();
                        show_tooltip_at_pointer(
                            ui.ctx(),
                            ui.layer_id(),
                            "position_highlight_hover".into(),
                            |ui| {
                                ui.label(format!("{}", offset));
                            },
                        );
                        if ui.input(|input| input.pointer.primary_clicked()) {
                            self.rearrange_bars_for_point(
                                rect.height(),
                                file_size,
                                i,
                                offset,
                                total_bytes,
                            );
                            tmp_rearrange_flag = true;
                            show_hex = true;
                            break;
                        }
                    } else if let Some(row_window) = hovered_row_window {
                        show_tooltip_at_pointer(
                            ui.ctx(),
                            ui.layer_id(),
                            "zoombar_tooltip".into(),
                            |ui| {
                                if let Some((entropy, quality)) = handler
                                    .get_entropy(row_window)
                                    .into_result_with_quality()
                                    .unwrap()
                                {
                                    if quality < 1.0 {
                                        ui.label(format!(
                                            "Entropy: {entropy:.02} (Estimation quality: {:.2}%)",
                                            quality * 100.0
                                        ));
                                    } else {
                                        ui.label(format!("Entropy: {entropy:.02}"));
                                    }
                                } else {
                                    ui.label("Entropy unknown");
                                }
                            },
                        );
                    }

                    window = bar.selection_window(window, rect.height());

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
                    render_overview(ui, window);
                }
            },
        );

        *marked_locations.hovered_location_mut() = new_hovered_location;
    }

    /// Rearranges the zoombars to focus on the given point.
    ///
    /// Bars before `start_bar` remain unchanged, if `point` lies within them, otherwise they are
    /// shifted.
    pub fn rearrange_bars_for_point(
        &mut self,
        bar_height: f32,
        file_size: u64,
        start_bar: usize,
        point: u64,
        total_bytes: u64,
    ) {
        let from_center_len = |center: f32, len: f32| -> RangeInclusive<f32> {
            let tentative_start = center - (len / 2.0);
            let start = if tentative_start < 0.0 {
                0.0
            } else if tentative_start + len > 1.0 {
                1.0 - len
            } else {
                tentative_start
            };

            start..=start + len
        };

        let mut window = Window::new(0, file_size);
        for bar in self.bars.iter_mut().take(start_bar + 1) {
            let selected_window = bar.selection_window(window, bar_height);
            if selected_window.contains(point) {
                window = selected_window;
                continue;
            }

            let min_selection_size = min_selection_size(window, bar_height);
            let selection = bar.selection(min_selection_size);

            let selection_len = selection.end() - selection.start();
            let selection_center = (point - window.start()) as f32 / window.size() as f32;

            bar.set_selection(from_center_len(selection_center, selection_len));

            window = bar.selection_window(window, bar_height);
        }
        self.bars.drain(start_bar + 1..);

        // if the current bar is full, re-do it instead
        if self.bars[start_bar].selected == NULL_SELECTION {
            self.bars.remove(start_bar);
        }

        while window.size() > total_bytes {
            let min_selection_size = min_selection_size(window, bar_height);
            let selection_len = 0.05f32.max(min_selection_size);
            let selection_center = (point - window.start()) as f32 / window.size() as f32;

            let mut bar = Zoombar::new();
            bar.set_selection(from_center_len(selection_center, selection_len));
            window = bar.selection_window(window, bar_height);

            self.bars.push(bar);
        }

        // the algorithm expects a full bar at the end, so provide it
        self.bars.push(Zoombar::new());
    }

    /// Creates a hash of the zoombar selection state.
    pub fn selection_state(&self) -> u64 {
        let mut hasher = std::hash::DefaultHasher::new();

        self.prev_height.to_ne_bytes().hash(&mut hasher);
        self.bars.len().hash(&mut hasher);
        for bar in &self.bars {
            bar.selected.start().to_ne_bytes().hash(&mut hasher);
            bar.selected.end().to_ne_bytes().hash(&mut hasher);
        }

        hasher.finish()
    }

    /// Determines if the zoombar selection state changed since the last call to this method.
    pub fn changed(&mut self) -> bool {
        let state = self.selection_state();
        let prev_state = self.prev_selection_state;
        self.prev_selection_state = state;

        prev_state != state
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

    /// Sets the selection to the given one.
    fn set_selection(&mut self, selection: RangeInclusive<f32>) {
        self.selected = selection;
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

    /// Computes the window that this zoombar selected.
    fn selection_window(&self, prev_window: Window, bar_height: f32) -> Window {
        let min_selection_size = min_selection_size(prev_window, bar_height);
        let selection = self.selection(min_selection_size);

        let new_start_offset = (prev_window.size() as f32 * selection.start()) as u64;

        let start = prev_window.start() + new_start_offset;
        let selection_size =
            ((selection.end() - selection.start()) as f64 * prev_window.size() as f64) as u64;

        Window::new(
            start,
            std::cmp::min(start + selection_size, prev_window.end()),
        )
    }

    /// Handles manipulating the selection on the zoombar.
    fn handle_selection(
        &mut self,
        rect: Rect,
        selecting: &mut bool,
        min_selection_size: f32,
        allow_selection: bool,
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
                && allow_selection
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
        allow_selection: bool,
        mut row_color: impl FnMut(Window) -> (Color32, Color32),
    ) -> Option<Window> {
        let total_rows = rect.height().trunc() as u64;

        self.handle_selection(
            rect,
            selecting,
            min_selection_size,
            allow_selection,
            ui.ctx(),
        );

        let selection = self.selection(min_selection_size);
        let selection_start = (rect.height() * *selection.start()).trunc() as usize;
        let selection_end = (rect.height() * *selection.end()).trunc() as usize;

        let bytes_per_row = (window.size() as f64 / total_rows as f64).round() as u64;

        let side_start = (rect.width() - 2.0) as usize;

        self.cached_image.paint_at(
            ui,
            rect,
            (self.selection(min_selection_size), window),
            |x, y| {
                let relative_offset = y as f64 / total_rows as f64;
                let offset_within_range = (relative_offset * window.size() as f64) as u64;

                let row_window =
                    Window::from_start_len(window.start() + offset_within_range, bytes_per_row);

                let (raw_color, side_color) = row_color(row_window);

                const HIGHLIGHT_STRENGTH: f64 = 0.4;

                if x >= side_start {
                    side_color
                } else if x == side_start - 1 {
                    Color32::BLACK
                } else {
                    if selection_start <= y && y <= selection_end {
                        //color::lerp(raw_color, egui::Color32::WHITE, HIGHLIGHT_STRENGTH)
                        raw_color
                    } else {
                        color::lerp(raw_color, egui::Color32::BLACK, HIGHLIGHT_STRENGTH)
                    }
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

/// The minimum size of a selection for the given current window and bar height.
fn min_selection_size(window: Window, bar_height: f32) -> f32 {
    let total_rows = (bar_height.trunc() as u64).max(1);
    let total_bytes = total_rows * 16;

    let maximum_min_selection_size = (total_rows - 1) as f32 / total_rows as f32;

    (total_bytes as f32 / window.size() as f32).min(maximum_min_selection_size)
}
