//! Implements highlighting of byte ranges in the hex view.

use std::ops::RangeInclusive;

use egui::{
    Color32, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, Ui,
    epaint::{ColorMode, PathStroke, QuadraticBezierShape},
    pos2,
};

use crate::gui::{color, settings::Settings};

/// Renders the selection polygon on screen.
pub(crate) fn highlight(
    ui: &mut Ui,
    mut range: RangeInclusive<u64>,
    inner_color: Color32,
    border_color: Color32,
    file_size: u64,
    screen_start_offset_in_rows: u64,
    rows_onscreen: u64,
    settings: &Settings,
) {
    let inner_color = color::lerp(
        inner_color,
        Color32::from_rgba_unmultiplied(inner_color.r(), inner_color.g(), inner_color.b(), 0),
        0.85,
    );

    let screen_start_offset = screen_start_offset_in_rows * 16;
    let screen_end_offset =
        std::cmp::min(screen_start_offset + (rows_onscreen + 1) * 16, file_size);

    if *range.start() > screen_end_offset || *range.end() < screen_start_offset {
        // the selection is off-screen and does not need to be rendered
        return;
    }

    if *range.start() < screen_start_offset.saturating_sub(16) {
        range = screen_start_offset - 16..=*range.end();
    }
    if *range.end() > screen_end_offset.saturating_add(16) {
        range = *range.start()..=screen_end_offset;
    }

    let screen_rect = ui.max_rect().intersect(ui.cursor());
    let char_height = settings.char_height();
    let char_width = settings.char_width();
    let large_space = settings.large_space();
    let small_space = settings.small_space();

    let row_start = |offset: u64| {
        let row = offset / 16;
        let start_row = screen_start_offset_in_rows;
        let row_offset = (row as i64 - start_row as i64).clamp(-1, rows_onscreen as i64 + 1);

        screen_rect.min.y + row_offset as f32 * char_height
    };
    let col_start_hex = |offset: u64| {
        let col = offset % 16;
        let start_offset = (16.0 * char_width) + large_space;
        let col_width = (2.0 * char_width) + small_space;
        let middle_offset = (col >= 8) as u8 as f32 * small_space;

        screen_rect.min.x + start_offset + middle_offset + col as f32 * col_width
    };
    let col_start_glyph = |offset: u64| {
        let col = offset % 16;
        let start_offset = (48.0 * char_width) + (2.0 * large_space) + (16.0 * small_space);
        let col_width = char_width;
        let middle_offset = (col >= 8) as u8 as f32 * small_space;

        screen_rect.min.x + start_offset + middle_offset + col as f32 * col_width
    };

    let mut points_hex = Vec::new();
    let mut points2_hex = Vec::new();
    let mut points_glyph = Vec::new();
    let mut points2_glyph = Vec::new();
    let mut rects = Vec::new();

    // y positions of selection start and end
    let start_y = row_start(*range.start());
    let end_y = row_start(*range.end()) + char_height;
    // x positions of selection start and end for both hex display and glyph display
    let start_x = (
        col_start_hex(*range.start()),
        col_start_glyph(*range.start()),
    );
    let end_x = (
        col_start_hex(*range.end()) + (2.0 * char_width),
        col_start_glyph(*range.end()) + char_width,
    );
    // x positions of first and last column for both hex display and glyph display
    let first_x = (col_start_hex(0), col_start_glyph(0));
    let last_x = (
        col_start_hex(15) + (2.0 * char_width),
        col_start_glyph(15) + char_width,
    );

    let mut add_point = |x: (f32, f32), y: f32| {
        points_hex.push(pos2(x.0, y));
        points_glyph.push(pos2(x.1, y));
    };
    let mut add_point2 = |x: (f32, f32), y: f32| {
        points2_hex.push(pos2(x.0, y));
        points2_glyph.push(pos2(x.1, y));
    };
    let mut add_rect = |start_x: (f32, f32), end_x: (f32, f32), start_y: f32, end_y: f32| {
        rects.push(Rect::from_min_max(
            pos2(start_x.0, start_y),
            pos2(end_x.0, end_y),
        ));
        rects.push(Rect::from_min_max(
            pos2(start_x.1, start_y),
            pos2(end_x.1, end_y),
        ));
    };

    if *range.start() / 16 == range.end() / 16 {
        // single row case
        add_point(start_x, start_y);
        add_point(end_x, start_y);
        add_point(end_x, end_y);
        add_point(start_x, end_y);

        add_rect(start_x, end_x, start_y, end_y);
    } else if (*range.start() / 16) + 1 == *range.end() / 16 && range.clone().count() <= 16 {
        // split two-row case
        add_point(start_x, start_y);
        add_point(last_x, start_y);
        add_point(last_x, start_y + char_height);
        add_point(start_x, start_y + char_height);

        add_point2(first_x, end_y - char_height);
        add_point2(end_x, end_y - char_height);
        add_point2(end_x, end_y);
        add_point2(first_x, end_y);

        add_rect(start_x, last_x, start_y, start_y + char_height);
        add_rect(first_x, end_x, end_y - char_height, end_y);
    } else {
        // joined multi-row case
        add_point(start_x, start_y);
        add_point(last_x, start_y);
        if *range.end() % 16 != 15 {
            add_point(last_x, end_y - char_height);
            add_point(end_x, end_y - char_height);
        }
        add_point(end_x, end_y);
        add_point(first_x, end_y);
        if *range.start() % 16 != 0 {
            add_point(first_x, start_y + char_height);
            add_point(start_x, start_y + char_height);
        }

        add_rect(start_x, last_x, start_y, start_y + char_height);
        if *range.start() / 16 + 1 != *range.end() / 16 {
            add_rect(first_x, last_x, start_y + char_height, end_y - char_height);
        }
        add_rect(first_x, end_x, end_y - char_height, end_y);
    }

    let painter = ui.painter();
    for rect in rects {
        painter.rect_filled(rect, 0.0, inner_color);
    }

    let corner_radius = settings.corner_radius();
    let stroke_width = settings.stroke_width();
    let trace_path = |points: Vec<Pos2>| {
        trace_path(painter, &points, stroke_width, corner_radius, border_color);
    };

    trace_path(points_hex);
    trace_path(points_glyph);
    if !points2_hex.is_empty() {
        trace_path(points2_hex);
        trace_path(points2_glyph);
    }
}

/// Traces the given points as a path.
pub fn trace_path(
    painter: &Painter,
    points: &[Pos2],
    stroke_width: f32,
    corner_radius: f32,
    color: Color32,
) {
    for (idx, &point) in points.iter().enumerate() {
        let next_point = points[(idx + 1) % points.len()];
        let second_next_point = points[(idx + 2) % points.len()];

        let towards_next = (next_point - point).normalized() * corner_radius;
        let near_point = point + towards_next;
        let before_next_point = next_point - towards_next;
        let after_next_point =
            next_point + (second_next_point - next_point).normalized() * corner_radius;

        painter.add(Shape::LineSegment {
            points: [near_point, before_next_point],
            stroke: Stroke::new(stroke_width, color),
        });
        painter.add(QuadraticBezierShape {
            points: [before_next_point, next_point, after_next_point],
            closed: false,
            fill: Color32::TRANSPARENT,
            stroke: PathStroke {
                width: stroke_width,
                color: ColorMode::Solid(color),
                kind: StrokeKind::Middle,
            },
        });
    }
}
