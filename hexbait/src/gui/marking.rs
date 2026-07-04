//! Implements marking of locations.

use std::collections::BTreeMap;

use egui::{Color32, Rect, RichText, Stroke, Ui, pos2};
use hexbait_common::Len;

use crate::{
    gui::{highlighting::trace_path, modules::scrollbars::offset_on_bar},
    marking::{MarkRef, MarkStore, MarkType},
    window::Window,
};

use super::color;

/// Shows the hover overlay for a marked location.
pub fn hover_marking(ui: &mut Ui, mark: MarkRef) {
    let description = match &mark.ty {
        MarkType::SearchResult => "Search result",
        MarkType::UserMark { .. } => "User mark",
        MarkType::Selection => "Selection",
        MarkType::HoveredParsed => "Hovered parsed value",
        MarkType::HoveredParseErr => "Hovered parsing error",
    };

    ui.label(description);
    if let MarkType::UserMark { name } = &mark.ty {
        if name.is_empty() {
            ui.label(RichText::new("unnamed").italics());
        } else {
            ui.label(name);
        }
    }

    ui.label(format!(
        "Offset: {} ({}B)",
        mark.window.start().as_u64(),
        size_format::SizeFormatterBinary::new(mark.window.start().as_u64())
    ));
    if mark.window.size() > Len::from(1) {
        ui.label(format!(
            "Length: {} ({}B)",
            mark.window.size().as_u64(),
            size_format::SizeFormatterBinary::new(mark.window.size().as_u64())
        ));
    }
}

/// Renders the given marked locations on the given bar window.
pub fn render_locations_on_bar(
    ui: &mut Ui,
    bar_rect: Rect,
    bar_window: Window,
    marked_locations: &mut MarkStore,
) {
    // first bin locations to similar y offsets, so that they don't overlap
    let mut location_dots_by_y_bins = BTreeMap::<u32, Vec<_>>::new();

    /// The bin size where close values are displayed in one line.
    const BIN_SIZE: u32 = 5;

    /// The transparency used for the locations on the bar.
    const TRANSPARENCY: f64 = 0.5;

    let bar_start = offset_on_bar(bar_rect, bar_window, bar_window.start()).unwrap();
    let bar_end = offset_on_bar(bar_rect, bar_window, bar_window.end() - Len::from(1)).unwrap();

    marked_locations.iter_marks_in_window(bar_window, |mark| {
        let start_pos = offset_on_bar(bar_rect, bar_window, mark.window.start());
        let end_pos = offset_on_bar(bar_rect, bar_window, mark.window.end());

        let draw_range;
        let bin_size_x = bar_rect.width() / 16.0;

        match (start_pos, end_pos) {
            (None, None) => return,
            (Some(start), None) => {
                draw_range = start..bar_end;
            }
            (None, Some(end)) => {
                draw_range = bar_start..end;
            }
            (Some(start), Some(end)) => {
                if (end.y - start.y) < BIN_SIZE as f32 {
                    let mut bin = ((start.y as u32) / BIN_SIZE) * BIN_SIZE;
                    if bin < start.y as u32 {
                        bin += BIN_SIZE;
                    }

                    location_dots_by_y_bins.entry(bin).or_default().push(mark);
                    return;
                } else {
                    draw_range = start..end;
                }
            }
        }

        let round_x_pos = |x_pos: f32| {
            let relative_x = x_pos - bar_rect.min.x;
            let rounded_x = (relative_x / bin_size_x).floor() * bin_size_x;
            rounded_x + bar_rect.min.x
        };

        let start_x = round_x_pos(draw_range.start.x);
        let end_x = round_x_pos(draw_range.end.x);

        let top_rect = Rect::from_min_max(
            pos2(start_x, draw_range.start.y),
            pos2(bar_rect.max.x, draw_range.start.y + 1.0),
        );
        let middle_rect = Rect::from_min_max(
            pos2(bar_rect.min.x, draw_range.start.y + 1.0),
            pos2(bar_rect.max.x, draw_range.end.y - 1.0),
        );
        let bottom_rect = Rect::from_min_max(
            pos2(bar_rect.min.x, draw_range.end.y - 1.0),
            pos2(end_x, draw_range.end.y),
        );

        for rect in [top_rect, middle_rect, bottom_rect] {
            ui.painter().rect_filled(
                rect,
                0.0,
                color::lerp(mark.ty.inner_color(), Color32::TRANSPARENT, TRANSPARENCY),
            );
        }

        let mut points = Vec::new();
        points.push(top_rect.left_top());
        points.push(top_rect.right_top());
        points.push(middle_rect.right_bottom());
        if bottom_rect.width() > 0.0 {
            points.push(bottom_rect.right_top());
            points.push(bottom_rect.right_bottom());
            points.push(bottom_rect.left_bottom());
        } else {
            points.push(middle_rect.left_bottom());
        }
        points.push(middle_rect.left_top());
        points.push(top_rect.left_bottom());

        trace_path(ui.painter(), &points, 1.0, 0.0, mark.ty.border_color());
    });

    let mut mark_location = None;

    for (y, mut locations) in location_dots_by_y_bins {
        locations.sort_by_key(|location| (location.window.start(), location.window.end()));

        for (i, location) in locations.iter().enumerate() {
            let center = pos2(
                bar_rect.left()
                    + bar_rect.width() * ((i + 1) as f32 / (locations.len() + 1) as f32),
                y as f32,
            );

            let is_hovered = marked_locations
                .hovered()
                .is_some_and(|hovered| hovered == location);
            let radius = if is_hovered {
                bar_rect.width() / 8.0
            } else {
                bar_rect.width() / 16.0
            };

            ui.painter().circle(
                center,
                radius,
                color::lerp(
                    location.ty.inner_color(),
                    Color32::TRANSPARENT,
                    TRANSPARENCY,
                ),
                Stroke::new(radius / 4.0, location.ty.border_color()),
            );

            let hovered = ui
                .input(|input| input.pointer.latest_pos())
                .map(|pos| (pos - center).length() < radius)
                .unwrap_or(false);
            if hovered {
                mark_location = Some(*location);
            }
        }
    }

    if let Some(mark_location) = mark_location {
        marked_locations.mark_hovered(mark_location.to_owned());
    }
}
