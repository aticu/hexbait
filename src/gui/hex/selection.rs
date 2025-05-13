//! Handles selection related things.

use std::ops::RangeInclusive;

use egui::{
    Color32, Context, Pos2, Rect, Response, Shape, Stroke, StrokeKind, Ui,
    epaint::{ColorMode, PathStroke, QuadraticBezierShape},
    pos2,
};

use crate::gui::color;

use super::primitives::{char_height, char_width, large_space, small_space};

/// Contains the necessary context to manage selections in the hex view.
pub(crate) struct SelectionContext {
    /// The selected bytes as absolute offsets.
    selection: Option<RangeInclusive<u64>>,
    /// The previous_selection.
    prev_selection: Option<RangeInclusive<u64>>,
    /// Whether or not the user is currently selecting bytes.
    selecting: bool,
}

impl SelectionContext {
    /// Creates a new selection context.
    pub(crate) fn new() -> SelectionContext {
        SelectionContext {
            selection: None,
            prev_selection: None,
            selecting: false,
        }
    }

    ///Checks if the selection process should end.
    pub(crate) fn check_for_selection_process_end(&mut self, ctx: &Context) {
        if self.selecting && ctx.input(|input| !input.pointer.primary_down()) {
            self.selecting = false;
            if let Some(selection) = &self.selection
                && selection.start() == selection.end()
                && self.selection == self.prev_selection
            {
                self.selection = None;
            }
        }
    }

    /// Handles a possible selection event with the given response for the given byte offset.
    pub(crate) fn handle_selection(
        &mut self,
        ctx: &Context,
        response: &Response,
        byte_offset: u64,
    ) {
        ctx.input(|input| {
            if self.selecting
                && let Some(origin) = input.pointer.latest_pos()
                && response.rect.contains(origin)
            {
                self.selection = Some(*self.selection.as_ref().unwrap().start()..=byte_offset);
            } else if input.pointer.primary_pressed()
                && let Some(origin) = input.pointer.press_origin()
                && response.rect.contains(origin)
            {
                self.selecting = true;
                self.prev_selection = self.selection.clone();
                self.selection = Some(byte_offset..=byte_offset);
            }
        });
    }

    /// Returns the current selection.
    pub(crate) fn selection(&self) -> Option<RangeInclusive<u64>> {
        if let Some(selection) = &self.selection {
            Some(if selection.start() <= selection.end() {
                selection.clone()
            } else {
                *selection.end()..=*selection.start()
            })
        } else {
            None
        }
    }

    /// Renders the selection polygon on screen.
    pub(crate) fn render_selection(
        &mut self,
        ui: &mut Ui,
        file_size: u64,
        screen_start_offset_in_rows: u64,
        rows_onscreen: u64,
        scale: f32,
    ) {
        let Some(mut selection) = self.selection() else {
            return;
        };

        let screen_start_offset = screen_start_offset_in_rows * 16;
        let screen_end_offset =
            std::cmp::min(screen_start_offset + (rows_onscreen + 1) * 16, file_size);

        if *selection.start() > screen_end_offset || *selection.end() < screen_start_offset {
            // the selection is off-screen and does not need to be rendered
            return;
        }

        if *selection.start() < screen_start_offset.saturating_sub(16) {
            selection = screen_start_offset - 16..=*selection.end();
        }
        if *selection.end() > screen_end_offset.saturating_add(16) {
            selection = *selection.start()..=screen_end_offset;
        }

        let screen_rect = ui.max_rect().intersect(ui.cursor());
        let char_height = char_height(scale);
        let char_width = char_width(scale);
        let large_space = large_space(scale);
        let small_space = small_space(scale);

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
        let start_y = row_start(*selection.start());
        let end_y = row_start(*selection.end()) + char_height;
        // x positions of selection start and end for both hex display and glyph display
        let start_x = (
            col_start_hex(*selection.start()),
            col_start_glyph(*selection.start()),
        );
        let end_x = (
            col_start_hex(*selection.end()) + (2.0 * char_width),
            col_start_glyph(*selection.end()) + char_width,
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

        if *selection.start() / 16 == selection.end() / 16 {
            // single row case
            add_point(start_x, start_y);
            add_point(end_x, start_y);
            add_point(end_x, end_y);
            add_point(start_x, end_y);

            add_rect(start_x, end_x, start_y, end_y);
        } else if (*selection.start() / 16) + 1 == *selection.end() / 16
            && selection.clone().count() <= 16
        {
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
            if *selection.end() % 16 != 15 {
                add_point(last_x, end_y - char_height);
                add_point(end_x, end_y - char_height);
            }
            add_point(end_x, end_y);
            add_point(first_x, end_y);
            if *selection.start() % 16 != 0 {
                add_point(first_x, start_y + char_height);
                add_point(start_x, start_y + char_height);
            }

            add_rect(start_x, last_x, start_y, start_y + char_height);
            if *selection.start() / 16 + 1 != *selection.end() / 16 {
                add_rect(first_x, last_x, start_y + char_height, end_y - char_height);
            }
            add_rect(first_x, end_x, end_y - char_height, end_y);
        }

        let painter = ui.painter();
        for rect in rects {
            painter.rect_filled(rect, 0.0, color::HIGHLIGHT_BACKGROUND_COLOR);
        }

        let corner_radius = scale * 0.15;
        let stroke_width = scale * 0.08;
        let trace_path = |points: Vec<Pos2>| {
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
                    stroke: Stroke::new(stroke_width, color::HIGHLIGHT_FOREROUND_COLOR),
                });
                painter.add(QuadraticBezierShape {
                    points: [before_next_point, next_point, after_next_point],
                    closed: false,
                    fill: Color32::TRANSPARENT,
                    stroke: PathStroke {
                        width: stroke_width,
                        color: ColorMode::Solid(color::HIGHLIGHT_FOREROUND_COLOR),
                        kind: StrokeKind::Middle,
                    },
                });
            }
        };

        trace_path(points_hex);
        trace_path(points_glyph);
        if points2_hex.len() != 0 {
            trace_path(points2_hex);
            trace_path(points2_glyph);
        }
    }
}
