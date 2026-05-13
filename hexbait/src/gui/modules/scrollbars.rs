//! Implements scrollbars to zoom in on the content of a file and scroll around in it.

use egui::{
    Color32, Context, FontId, PointerButton, PopupAnchor, Pos2, Rect, Sense, Shape, Stroke,
    Tooltip, Ui, pos2, vec2,
};
use hexbait_common::{AbsoluteOffset, Input, Len, RelativeOffset};
use size_format::SizeFormatterBinary;

use crate::{
    IDLE_TIME,
    gui::{color, image_processing::blur_image, marking::render_locations_on_bar},
    state::{DisplayType, InteractionState, ScrollState, Scrollbar, Settings, State},
    statistics::{MetricsQuality, StatisticsMetrics},
    window::Window,
};

/// Shows the scrollbars.
pub fn show(ui: &mut Ui, state: &mut State, _: &Input) {
    let file_size = state.scroll_state.file_size();
    let rect = ui.max_rect().intersect(ui.cursor());

    state
        .scroll_state
        .update_parameters(rect.height(), &state.settings);

    // be deliberately small to fit more text here
    let size_text_height = state.settings.font_size() * 0.7;

    let total_bytes = state.scroll_state.total_hexdump_bytes();

    if total_bytes >= file_size {
        state.scroll_state.display_suggestion = DisplayType::Hexview;
        return;
    } else if state.scroll_state.scrollbars.is_empty() {
        state
            .scroll_state
            .scrollbars
            .push(Scrollbar::new(file_size));
    }

    let mut window = state.scroll_state.first_window();
    let mut show_hex = false;

    let mut old_selection = None::<[Pos2; 2]>;

    for i in 0..state.scroll_state.scrollbars.len() {
        // in case scrollbars get removed before the loop ends, we still exit early
        if i >= state.scroll_state.scrollbars.len() {
            break;
        }

        let mut rect = ui.max_rect().intersect(ui.cursor());
        rect.min += vec2(0.0, size_text_height);
        rect.set_width(16.0 * state.settings.bar_width_multiplier() as f32 + 3.0);
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
        let allow_selection = state.marked_locations.hovered().is_none();
        handle_interactions(
            rect,
            &mut state.scroll_state,
            i,
            total_bytes,
            window,
            allow_selection,
            ui.ctx(),
        );

        if let Some(old_selection) = old_selection {
            let painter = ui.painter().with_clip_rect(Rect::from_points(&[
                old_selection[0],
                old_selection[1],
                rect.left_top(),
                rect.left_bottom(),
            ]));

            let stroke = Stroke::new(1.0, state.settings.scrollbar_selection_border_color());

            painter.line_segment([old_selection[0], rect.left_top()], stroke);
            painter.line_segment([old_selection[1], rect.left_bottom()], stroke);
            painter.add(Shape::convex_polygon(
                vec![
                    old_selection[0],
                    rect.left_top(),
                    rect.left_bottom(),
                    old_selection[1],
                ],
                state
                    .settings
                    .scrollbar_selection_border_color()
                    .gamma_multiply(0.3),
                Stroke::NONE,
            ));
        }

        let selected_window = if i == state.scroll_state.scrollbars.len() - 1
            && let Some(hover_pos) = state.scroll_state.gilbert_hover_position
        {
            let size = state.scroll_state.hover_selection_size / 2.0;
            let hover_pos = hover_pos.clamp(size, 1.0 - size);

            (
                (hover_pos - size).clamp(0.0, 1.0) as f64,
                (hover_pos + size).clamp(0.0, 1.0) as f64,
            )
        } else {
            (
                state.scroll_state.scrollbars[i].relative_selection_start(window),
                state.scroll_state.scrollbars[i].relative_selection_end(window),
            )
        };
        let hovered_row_window = render_bar(
            ui,
            &mut state.scroll_state.scrollbars[i],
            &state.settings,
            rect,
            window,
            selected_window,
            |window| {
                let (metrics, quality) = state.statistics_handler.get_metrics(window);
                full_quality &= !quality.is_estimated();
                (metrics, quality)
            },
        );

        let selection_start = pos2(
            rect.max.x,
            rect.min.y
                + state.scroll_state.scrollbars[i].relative_selection_start(window) as f32
                    * rect.height(),
        );
        let selection_end = pos2(
            rect.max.x,
            rect.min.y
                + state.scroll_state.scrollbars[i].relative_selection_end(window) as f32
                    * rect.height(),
        );
        old_selection = Some([selection_start, selection_end]);

        if !full_quality {
            state.scroll_state.scrollbars[i]
                .cached_image
                .require_repaint();
            ui.ctx().request_repaint_after(IDLE_TIME);
        }

        render_locations_on_bar(ui, rect, window, &mut state.marked_locations);

        if let Some(location) = state.marked_locations.hovered()
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
                state.scroll_state.rearrange_bars_for_point(i, offset);
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
                if let (Some(metrics), quality) = state.statistics_handler.get_metrics(row_window) {
                    if quality.is_estimated() {
                        ui.label(format!(
                            "Entropy: {} (estimate based on subsampling)",
                            metrics.entropy,
                        ));
                    } else {
                        ui.label(format!("Entropy: {}", metrics.entropy));
                    }
                } else {
                    ui.label("Entropy unknown");
                }
            });
        }

        window = state.scroll_state.scrollbars[i].window(window, total_bytes);

        if state.scroll_state.interaction_state.selecting_bar(i) {
            state.scroll_state.scrollbars.truncate(i + 1);
            state
                .scroll_state
                .scrollbars
                .push(Scrollbar::new(window.size()));
        }
    }

    // keep bars consistent in case of double clicks
    state.scroll_state.enforce_no_full_bar_in_middle_invariant();

    if show_hex {
        state.scroll_state.display_suggestion = DisplayType::Hexview;
    } else {
        state.scroll_state.display_suggestion = DisplayType::Overview;
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
    selected_window: (f64, f64),
    mut metrics: impl FnMut(Window) -> (Option<StatisticsMetrics>, MetricsQuality),
) -> Option<Window> {
    let total_rows = rect.height().trunc() as u64;

    let selection_start = (selected_window.0 * rect.height() as f64).round() as usize;
    let selection_end = (selected_window.1 * rect.height() as f64).round() as usize;

    let bytes_per_row =
        Len::from((window.size().as_u64() as f64 / total_rows as f64).round() as u64);
    let bytes_per_square = bytes_per_row / 16;

    let side_start = (rect.width() - 2.0) as usize;
    let row_width = side_start / 16;

    let mut full_quality_scrollbar = true;
    let mut full_quality_row = true;

    scrollbar.cached_image.paint_at(
        ui,
        rect,
        (window, settings.fine_grained_scrollbars()),
        || (),
        |_, x, y| {
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

                let (metrics, quality) = metrics(window);

                if quality.is_estimated() {
                    full_quality_scrollbar = false;
                    full_quality_row = false;
                }

                color::metrics_color(metrics, quality, settings)
            }
        },
    );

    let selection_state = scrollbar.state_for_cached_image();
    scrollbar.selection_overlay.paint_at(
        ui,
        rect,
        (
            selection_state,
            window,
            settings.fine_grained_scrollbars(),
            selected_window,
        ),
        || {
            scrollbar
                .blurred_image
                .get((rect, window, settings.fine_grained_scrollbars()), |_| {
                    blur_image(scrollbar.cached_image.raw(), settings)
                })
        },
        |blurred_image, x, y| {
            if x >= side_start {
                return Color32::TRANSPARENT;
            }
            if selection_start < y && y < selection_end {
                return Color32::TRANSPARENT;
            }

            let dist = if y <= selection_start {
                selection_start - y
            } else {
                y - selection_end
            };

            let selection_border_size = settings.selection_border_size();
            if dist == 0 {
                Color32::TRANSPARENT
            } else if dist > selection_border_size {
                blurred_image[(x, y)]
            } else {
                let rel_dist = (selection_border_size - dist) as f32 / selection_border_size as f32;
                let border_strength = rel_dist.powi(2);

                color::lerp(
                    blurred_image[(x, y)],
                    settings.scrollbar_selection_border_color(),
                    border_strength as f64,
                )
            }
        },
    );

    if !full_quality_scrollbar {
        scrollbar.cached_image.require_repaint();
        scrollbar.blurred_image.invalidate();
        scrollbar.selection_overlay.require_repaint();
    }

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
