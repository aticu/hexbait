//! Implements scrollbars to zoom in on the content of a file and scroll around in it.

use egui::{
    Color32, Context, FontId, PointerButton, PopupAnchor, Pos2, Rect, Sense, Tooltip, Ui, vec2,
};
use hexbait_common::{AbsoluteOffset, Len, RelativeOffset};
use size_format::SizeFormatterBinary;

use crate::{
    IDLE_TIME,
    state::{DisplayType, InteractionState, ScrollState, Scrollbar, Settings},
    statistics::StatisticsHandler,
    window::Window,
};

use super::{
    color,
    marking::{MarkedLocations, render_locations_on_bar},
};

/// Renders the scrollbars.
pub fn render(
    ui: &mut Ui,
    scroll_state: &mut ScrollState,
    settings: &Settings,
    marked_locations: &mut MarkedLocations,
    handler: &StatisticsHandler,
) {
    let file_size = scroll_state.file_size();
    let rect = ui.max_rect().intersect(ui.cursor());

    scroll_state.update_parameters(rect.height(), settings);

    // be deliberately small to fit more text here
    let size_text_height = settings.font_size() * 0.7;

    let total_bytes = scroll_state.total_hexdump_bytes();

    if total_bytes >= file_size {
        scroll_state.display_suggestion = DisplayType::Hexview;
        return;
    } else if scroll_state.scrollbars.is_empty() {
        scroll_state.scrollbars.push(Scrollbar::new(file_size));
    }

    let mut window = scroll_state.first_window();
    let mut show_hex = false;

    for i in 0..scroll_state.scrollbars.len() {
        if i >= scroll_state.scrollbars.len() {
            break;
        }

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

        let mut full_quality = true;
        let allow_selection = marked_locations.hovered().is_none();
        handle_interactions(
            rect,
            scroll_state,
            i,
            total_bytes,
            window,
            allow_selection,
            ui.ctx(),
        );

        let hovered_row_window = render_bar(
            ui,
            &mut scroll_state.scrollbars[i],
            settings,
            rect,
            window,
            |window| {
                if let Some((entropy, quality)) = handler
                    .get_entropy(window)
                    .into_result_with_quality()
                    .unwrap()
                {
                    if quality < 1.0 {
                        full_quality = false;
                    }
                    (Some(entropy), quality)
                } else {
                    full_quality = false;
                    (None, 0.0)
                }
            },
        );

        if !full_quality {
            scroll_state.scrollbars[i].cached_image.require_repaint();
            ui.ctx().request_repaint_after(IDLE_TIME);
        }

        render_locations_on_bar(ui, rect, window, marked_locations);

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
                scroll_state.rearrange_bars_for_point(i, offset);
                show_hex = true;
                break;
            }
        } else if let Some(row_window) = hovered_row_window {
            Tooltip::always_open(
                ui.ctx().clone(),
                ui.layer_id(),
                "scrollbar_tooltip".into(),
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

        window = scroll_state.scrollbars[i].window(window, total_bytes);

        if scroll_state.interaction_state.selecting_bar(i) {
            scroll_state.scrollbars.truncate(i + 1);
            scroll_state.scrollbars.push(Scrollbar::new(window.size()));
        }
    }

    // keep bars consistent in case of double clicks
    scroll_state.enforce_no_full_bar_in_middle_invariant();

    if show_hex {
        scroll_state.display_suggestion = DisplayType::Hexview;
    } else {
        scroll_state.display_suggestion = DisplayType::Statistics;
    }
}

/// Handles manipulating the selection on the scrollbar.
fn handle_interactions(
    rect: Rect,
    scroll_state: &mut ScrollState,
    bar_idx: usize,
    total_bytes: Len,
    window: Window,
    allow_selection: bool,
    ctx: &Context,
) {
    ctx.input(|input| {
        let scrollbar = &mut scroll_state.scrollbars[bar_idx];

        match &mut scroll_state.interaction_state {
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
                        scrollbar.set_selection(start, len);
                    }
                } else {
                    let (start, len) = selection(*start, *end);
                    scrollbar.set_selection(start, len);

                    // the selection process finished
                    scroll_state.interaction_state = InteractionState::None;

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
                    scroll_state.interaction_state = InteractionState::None;
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
                    scroll_state.interaction_state = InteractionState::WindowSelection {
                        start: current,
                        end: current,
                        bar_idx,
                    };

                    scrollbar.set_selection(current, total_bytes);
                } else if input.pointer.secondary_pressed()
                    && let Some(pos) = input.pointer.latest_pos()
                    && rect.contains(pos)
                {
                    // start dragging
                    scroll_state.interaction_state = InteractionState::Dragging { bar_idx };
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
                scroll_state.scroll_up(bar_idx, scroll_amount);
            } else {
                scroll_state.scroll_down(bar_idx, scroll_amount, total_bytes);
            }
        }
    });
}

/// Renders a single scrollbar.
fn render_bar(
    ui: &mut Ui,
    scrollbar: &mut Scrollbar,
    settings: &Settings,
    rect: Rect,
    window: Window,
    mut entropy: impl FnMut(Window) -> (Option<f32>, f32),
) -> Option<Window> {
    let total_rows = rect.height().trunc() as u64;

    let selection_start =
        (scrollbar.relative_selection_start(window) * rect.height() as f64).round() as usize;
    let selection_end =
        (scrollbar.relative_selection_end(window) * rect.height() as f64).round() as usize;

    let bytes_per_row =
        Len::from((window.size().as_u64() as f64 / total_rows as f64).round() as u64);
    let bytes_per_square = bytes_per_row / 16;

    let side_start = (rect.width() - 2.0) as usize;
    let row_width = side_start / 16;

    let mut full_quality_row = true;

    scrollbar.cached_image.paint_at(
        ui,
        rect,
        (
            scrollbar.state_for_cached_image(),
            window,
            settings.fine_grained_scrollbars(),
        ),
        |x, y| {
            if x >= side_start {
                if full_quality_row {
                    Color32::GREEN
                } else {
                    Color32::RED
                }
            } else if x == side_start - 1 {
                Color32::BLACK
            } else {
                if x == 0 {
                    full_quality_row = true;
                }

                let relative_offset = y as f64 / total_rows as f64;
                let offset_within_range =
                    RelativeOffset::from((relative_offset * window.size().as_u64() as f64) as u64);

                let window_size = if settings.fine_grained_scrollbars() {
                    bytes_per_square
                } else {
                    bytes_per_row
                };
                let column_offset = if settings.fine_grained_scrollbars() {
                    (x / row_width) as u64 * bytes_per_square
                } else {
                    Len::ZERO
                };

                let window = Window::from_start_len(
                    window.start() + offset_within_range + column_offset,
                    window_size,
                );

                let (raw_entropy, quality) = entropy(window);

                if quality < 1.0 {
                    full_quality_row = false;
                }

                let raw_color = if let Some(entropy) = raw_entropy {
                    settings.entropy_color(entropy)
                } else {
                    settings.missing_color()
                };

                const HIGHLIGHT_STRENGTH: f64 = 0.4;

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

    if (0.0..=1.0).contains(&relative_offset) {
        let offset = ((16.0 * height) * relative_offset) as u32;
        let offset_x = offset % 16;
        let offset_y = offset / 16;

        Some(bar_rect.min + vec2(offset_x as f32 * bar_rect.width() / 16.0, offset_y as f32))
    } else {
        None
    }
}
