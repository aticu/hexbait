//! Implements zoombars to zoom in on the content of a file.

use egui::{
    Color32, Context, FontId, PointerButton, PopupAnchor, Pos2, Rect, Sense, Tooltip, Ui, vec2,
};
use hexbait_common::{AbsoluteOffset, ChangeState, Len, RelativeOffset};
use size_format::SizeFormatterBinary;

use crate::{
    IDLE_TIME,
    data::DataSource,
    state::{DisplaySuggestion, InteractionState, ScrollState, Scrollbar, Settings},
    statistics::StatisticsHandler,
    window::Window,
};

use super::{
    color,
    marking::{MarkedLocations, render_locations_on_bar},
};

/// Zoombars are a GUI component to narrow in on parts of a file.
pub struct Zoombars {}

impl Zoombars {
    /// Creates new zoombars.
    pub fn new() -> Zoombars {
        Zoombars {}
    }

    /// Renders the zoombars.
    pub fn render<Source: DataSource>(
        &mut self,
        ui: &mut Ui,
        source: &mut Source,
        scroll_state: &mut ScrollState,
        settings: &Settings,
        marked_locations: &mut MarkedLocations,
        handler: &StatisticsHandler,
        render_hex: impl FnOnce(&mut Ui, &mut Source, u64, &mut MarkedLocations),
        render_overview: impl FnOnce(&mut Ui, Window),
    ) -> ChangeState {
        let file_size = source.len();
        let rect = ui.max_rect().intersect(ui.cursor());

        // be deliberately small to fit more text here
        let size_text_height = settings.font_size() * 0.7;

        let total_rows = (rect.height().trunc() as u64).max(1);
        let total_bytes = Len::from(total_rows * 16);

        if total_bytes >= file_size {
            render_hex(ui, source, 0, marked_locations);
            scroll_state.display_suggestion = DisplaySuggestion::Hexview;
            return ChangeState::Unchanged;
        } else if scroll_state.scrollbars.is_empty() {
            scroll_state.scrollbars.push(Scrollbar::new(file_size));
        }

        let mut window = source.full_window();
        let mut show_hex = false;

        let mut new_hovered_location = None;
        let currently_hovered = marked_locations.hovered_location_mut().clone();

        for i in 0..scroll_state.scrollbars.len() {
            let Some(bar) = scroll_state.scrollbars.get_mut(i) else {
                break;
            };

            let mut rect = ui.max_rect().intersect(ui.cursor());
            rect.min += vec2(0.0, size_text_height);
            rect.set_width(16.0 * settings.bar_width_multiplier() as f32 + 3.0);
            let rect = rect;

            ui.painter().text(
                rect.min,
                egui::Align2::LEFT_BOTTOM,
                format!("{}B", SizeFormatterBinary::new(window.size().as_u64())),
                FontId::proportional(size_text_height),
                ui.style().noninteractive().text_color(),
            );

            if window.size() <= total_bytes {
                show_hex = true;
                break;
            }

            let mut require_repaint = false;
            let allow_selection = marked_locations.hovered().is_none();
            handle_interactions(
                rect,
                bar,
                i,
                &mut scroll_state.interaction_state,
                total_bytes,
                window,
                allow_selection,
                ui.ctx(),
            );

            let hovered_row_window = render_bar(ui, bar, rect, window, |row_window| {
                if let Some((entropy, quality)) = handler
                    .get_entropy(row_window)
                    .into_result_with_quality()
                    .unwrap()
                {
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
            });

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
                Tooltip::always_open(
                    ui.ctx().clone(),
                    ui.layer_id(),
                    "position_highlight_hover".into(),
                    PopupAnchor::Pointer,
                )
                .show(|ui| {
                    ui.label(format!("{}", offset.as_u64()));
                });
                if ui.input(|input| input.pointer.primary_clicked()) {
                    self.rearrange_bars_for_point(scroll_state, file_size, i, offset, total_bytes);
                    show_hex = true;
                    break;
                }
            } else if let Some(row_window) = hovered_row_window {
                Tooltip::always_open(
                    ui.ctx().clone(),
                    ui.layer_id(),
                    "zoombar_tooltip".into(),
                    PopupAnchor::Pointer,
                )
                .show(|ui| {
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
                });
            }

            window = bar.window(window, total_bytes);

            if scroll_state.interaction_state.selecting_bar(i) {
                scroll_state.scrollbars.truncate(i + 1);
                scroll_state.scrollbars.push(Scrollbar::new(window.size()));
            }
        }

        // keep bars consistent in case of double clicks
        let mut prev_len = file_size;
        for (i, bar) in scroll_state.scrollbars[..scroll_state.scrollbars.len() - 1]
            .iter()
            .enumerate()
        {
            if bar.selection_start == RelativeOffset::ZERO && bar.selection_len == prev_len {
                // remove other bars behind this one
                scroll_state.scrollbars.truncate(i + 1);
                break;
            }

            prev_len = bar.selection_len;
        }

        if show_hex {
            let start = if window.start().is_start_of_file() {
                // ensure that the correction below does not make the start invisible

                0
            } else if window.end() > AbsoluteOffset::ZERO + file_size - Len::from(16) {
                // over-correct towards the end to ensure it's guaranteed to be visible

                let rounded_up_size = file_size.round_up(16);

                (rounded_up_size - total_bytes).as_u64() / 16
            } else {
                window.start().as_u64() / 16
            };

            render_hex(ui, source, start, marked_locations);
            scroll_state.display_suggestion = DisplaySuggestion::Hexview;
        } else {
            render_overview(ui, window);
            scroll_state.display_suggestion = DisplaySuggestion::Overview;
        }

        *marked_locations.hovered_location_mut() = new_hovered_location;

        scroll_state.changed(rect.height())
    }

    /// Rearranges the zoombars to focus on the given point.
    ///
    /// Bars before `start_bar` remain unchanged, if `point` lies within them, otherwise they are
    /// shifted.
    pub fn rearrange_bars_for_point(
        &mut self,
        scroll_state: &mut ScrollState,
        file_size: Len,
        start_bar: usize,
        point: AbsoluteOffset,
        total_bytes: Len,
    ) {
        let center_bar_on_point = |bar: &mut Scrollbar, window: Window| {
            let point_on_bar = point - window.start();
            let half_len = bar.selection_len / 2;

            if point_on_bar < half_len {
                bar.selection_start = RelativeOffset::ZERO;
            } else if point_on_bar + half_len > window.size() {
                bar.selection_start =
                    RelativeOffset::from((window.size() - bar.selection_len).as_u64());
            } else {
                bar.selection_start = RelativeOffset::from((point_on_bar - half_len).as_u64());
            }
        };

        let mut window = Window::from_start_len(AbsoluteOffset::ZERO, file_size);
        let mut parent_window = window;
        for bar in scroll_state.scrollbars.iter_mut().take(start_bar + 1) {
            let selected_window = bar.window(window, total_bytes);
            if selected_window.contains(point) {
                parent_window = window;
                window = selected_window;
                continue;
            }

            center_bar_on_point(bar, window);
            parent_window = window;
            window = bar.window(window, total_bytes);
        }
        scroll_state.scrollbars.drain(start_bar + 1..);

        // if the current bar is full, re-do it instead
        if scroll_state.scrollbars[start_bar].selection_len == parent_window.size() {
            scroll_state.scrollbars.remove(start_bar);
        }

        while window.size() > total_bytes {
            let selection_len = std::cmp::max(
                Len::from((0.05f64 * window.size().as_u64() as f64) as u64),
                total_bytes,
            );

            let mut bar = Scrollbar::new(window.size());
            bar.selection_len = selection_len;

            center_bar_on_point(&mut bar, window);
            window = bar.window(window, total_bytes);

            scroll_state.scrollbars.push(bar);
        }

        // the algorithm expects a full bar at the end, so provide it
        scroll_state.scrollbars.push(Scrollbar::new(window.size()));
    }
}

/// Handles manipulating the selection on the zoombar.
fn handle_interactions(
    rect: Rect,
    scrollbar: &mut Scrollbar,
    bar_idx: usize,
    interaction_state: &mut InteractionState,
    total_bytes: Len,
    window: Window,
    allow_selection: bool,
    ctx: &Context,
) {
    ctx.input(|input| {
        match interaction_state {
            InteractionState::WindowSelection {
                start,
                end,
                bar_idx: selecting_bar_idx,
            } if *selecting_bar_idx == bar_idx => {
                let selection = |start: RelativeOffset, end: RelativeOffset| {
                    if start <= end {
                        let len = std::cmp::max(end - start, total_bytes);

                        if start + len > RelativeOffset::from(window.size().as_u64()) {
                            (RelativeOffset::from((window.size() - len).as_u64()), len)
                        } else {
                            (start, len)
                        }
                    } else {
                        let len = std::cmp::max(start - end, total_bytes);

                        if RelativeOffset::from(len.as_u64()) > start {
                            (RelativeOffset::ZERO, len)
                        } else {
                            (start - len, len)
                        }
                    }
                };

                if input.pointer.primary_down() {
                    // continue ongoing selection
                    if let Some(pos) = input.pointer.latest_pos() {
                        let current = (pos.y - rect.min.y) / rect.height();
                        let current = RelativeOffset::from(
                            (current as f64 * window.size().as_u64() as f64) as u64,
                        );
                        *end = current;

                        let (start, len) = selection(*start, *end);
                        scrollbar.selection_start = start;
                        scrollbar.selection_len = len;
                    }
                } else {
                    let (start, len) = selection(*start, *end);
                    scrollbar.selection_start = start;
                    scrollbar.selection_len = len;

                    // the selection process finished
                    *interaction_state = InteractionState::None;

                    // double click resets selection
                    if input.pointer.button_double_clicked(PointerButton::Primary) {
                        *scrollbar = Scrollbar::new(window.size());
                    }
                }
            }
            InteractionState::Dragging {
                bar_idx: dragging_bar_idx,
            } if *dragging_bar_idx == bar_idx => {
                if input.pointer.secondary_down() {
                    // continue ongoing dragging
                    if let Some(pos) = input.pointer.latest_pos()
                        && rect.expand2(vec2(f32::INFINITY, 0.0)).contains(pos)
                    {
                        let current = (pos.y - rect.min.y).clamp(0.0, rect.height()) as f64
                            / rect.height() as f64;
                        let center =
                            RelativeOffset::from((current * window.size().as_u64() as f64) as u64);

                        scrollbar.center_around(center, window);
                    }
                } else {
                    // stop the dragging
                    *interaction_state = InteractionState::None;
                }
            }
            _ => {
                if input.pointer.primary_pressed()
                    && let Some(pos) = input.pointer.latest_pos()
                    && rect.contains(pos)
                    && allow_selection
                {
                    // Starting a new selection
                    let current = (pos.y - rect.min.y) / rect.height();
                    let current = RelativeOffset::from(
                        (current as f64 * window.size().as_u64() as f64) as u64,
                    );
                    *interaction_state = InteractionState::WindowSelection {
                        start: current,
                        end: current,
                        bar_idx,
                    };

                    scrollbar.selection_start = current;
                    scrollbar.selection_len = total_bytes;
                } else if input.pointer.secondary_pressed()
                    && let Some(pos) = input.pointer.latest_pos()
                    && rect.contains(pos)
                {
                    // start dragging
                    *interaction_state = InteractionState::Dragging { bar_idx };
                }
            }
        }

        // scroll if we are within the scroll bar
        if let Some(pos) = input.pointer.latest_pos()
            && rect.contains(pos)
            && input.smooth_scroll_delta.y != 0.0
        {
            let scroll_delta = (-input.smooth_scroll_delta.y as f64 / 2.0).trunc();
            let scroll_up = scroll_delta < 0.0;
            let scroll_delta = scroll_delta.abs();
            let row_bytes = window.size().as_u64() as f64 / rect.height() as f64;
            let scroll_amount = (scroll_delta * row_bytes) as u64;

            if scroll_up {
                scrollbar.selection_start = RelativeOffset::from(
                    scrollbar
                        .selection_start
                        .as_u64()
                        .saturating_sub(scroll_amount),
                );
            } else {
                scrollbar.selection_start = RelativeOffset::from(std::cmp::min(
                    scrollbar
                        .selection_start
                        .as_u64()
                        .saturating_add(scroll_amount),
                    (window.size() - scrollbar.selection_len).as_u64(),
                ));
            }
        }
    });
}

/// Renders a single scrollbar.
fn render_bar(
    ui: &mut Ui,
    scrollbar: &mut Scrollbar,
    rect: Rect,
    window: Window,
    mut row_color: impl FnMut(Window) -> (Color32, Color32),
) -> Option<Window> {
    let total_rows = rect.height().trunc() as u64;

    let selection_start_relative =
        scrollbar.selection_start.as_u64() as f64 / window.size().as_u64() as f64;
    let selection_end_relative = (scrollbar.selection_start + scrollbar.selection_len).as_u64()
        as f64
        / window.size().as_u64() as f64;
    let selection_start = (selection_start_relative * rect.height() as f64).round() as usize;
    let selection_end = (selection_end_relative * rect.height() as f64).round() as usize;

    let bytes_per_row =
        Len::from((window.size().as_u64() as f64 / total_rows as f64).round() as u64);

    let side_start = (rect.width() - 2.0) as usize;

    scrollbar.cached_image.paint_at(
        ui,
        rect,
        (scrollbar.selection_start, scrollbar.selection_len, window),
        |x, y| {
            let relative_offset = y as f64 / total_rows as f64;
            let offset_within_range =
                RelativeOffset::from((relative_offset * window.size().as_u64() as f64) as u64);

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
            let offset_within_range =
                RelativeOffset::from((relative_offset * window.size().as_u64() as f64) as u64);

            Window::from_start_len(window.start() + offset_within_range, bytes_per_row)
        })
}

/// Returns the position of `offset` on the bar spanning `bar_window` displayed in `bar_rect`.
pub fn offset_on_bar(bar_rect: Rect, bar_window: Window, offset: AbsoluteOffset) -> Option<Pos2> {
    if offset < bar_window.start() {
        return None;
    }

    let relative_offset =
        (offset - bar_window.start()).as_u64() as f64 / bar_window.size().as_u64() as f64;
    let height = bar_rect.height().ceil() as f64;

    if 0.0 <= relative_offset && relative_offset <= 1.0 {
        let offset = ((16.0 * height) * relative_offset) as u32;
        let offset_x = offset % 16;
        let offset_y = offset / 16;

        Some(bar_rect.min + vec2(offset_x as f32 * bar_rect.width() / 16.0, offset_y as f32))
    } else {
        None
    }
}
