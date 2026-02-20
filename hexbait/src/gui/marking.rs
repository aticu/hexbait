//! Implements marking of locations.

use std::collections::BTreeMap;

use egui::{Color32, Rect, Stroke, Ui, pos2};
use hexbait_common::{AbsoluteOffset, Len};

use crate::{
    gui::{highlighting::trace_path, modules::scrollbars::offset_on_bar},
    window::Window,
};

use super::color;

/// Stores the marked locations to highlight.
pub struct MarkedLocations {
    /// The locations that should be highlighted.
    locations: BTreeMap<AbsoluteOffset, Vec<MarkedLocation>>,
    /// The currently hovered location.
    hovered_location: Option<MarkedLocation>,
    /// The location that was hovered this frame.
    new_hovered_location: Option<MarkedLocation>,
}

impl MarkedLocations {
    /// Creates a new empty list of marked locations.
    pub fn new() -> MarkedLocations {
        MarkedLocations {
            locations: BTreeMap::new(),
            hovered_location: None,
            new_hovered_location: None,
        }
    }

    /// Adds a new marked location to be displayed.
    pub fn add(&mut self, marked_location: MarkedLocation) {
        self.locations
            .entry(marked_location.window.start())
            .or_default()
            .push(marked_location);
    }

    /// Remove marked locations that match the given filter.
    pub fn remove_where(&mut self, mut filter: impl FnMut(&MarkedLocation) -> bool) {
        for location_list in self.locations.values_mut() {
            location_list.retain(|location| !filter(location));
        }
    }

    /// Iterates over all marked locations that overlap with the given window in no specific order.
    pub fn iter_window(&self, window: Window) -> impl Iterator<Item = &MarkedLocation> {
        self.locations
            .iter()
            .flat_map(|(_, locations_at_start)| locations_at_start.iter())
            .filter(move |marked_location| marked_location.window().overlaps(window))
    }

    /// The currently hovered location.
    pub fn hovered(&self) -> Option<&MarkedLocation> {
        if let Some(location) = &self.hovered_location {
            self.locations
                .get(&location.window().start())
                .unwrap()
                .iter()
                .find(|&loc| location == loc)
        } else {
            None
        }
    }

    /// Returns a mutable reference to the currently hovered location.
    pub fn mark_hovered(&mut self, location: MarkedLocation) {
        self.new_hovered_location = Some(location);
    }

    /// Marks the end of the frame, updating the marked location.
    pub fn end_of_frame(&mut self) {
        self.hovered_location = self.new_hovered_location.take();
    }
}

impl Default for MarkedLocations {
    fn default() -> Self {
        MarkedLocations::new()
    }
}

/// A marked location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkedLocation {
    /// The window that this location refers to.
    window: Window,
    /// The kind of the marked location.
    kind: MarkingKind,
}

impl MarkedLocation {
    /// Creates a new marked location on the given window.
    pub fn new(window: Window, kind: MarkingKind) -> MarkedLocation {
        MarkedLocation { window, kind }
    }

    /// The window covered by this marked location.
    pub fn window(&self) -> Window {
        self.window
    }

    /// The kind of the marked location.
    pub fn kind(&self) -> MarkingKind {
        self.kind
    }

    /// The inner color of this marked location.
    pub fn inner_color(&self) -> Color32 {
        match self.kind() {
            MarkingKind::Selection => Color32::WHITE,
            MarkingKind::HoveredParsed => Color32::DARK_RED,
            MarkingKind::HoveredParseErr => Color32::WHITE,
            MarkingKind::SearchResult => Color32::BLUE,
        }
    }

    /// The border color of this marked location.
    pub fn border_color(&self) -> Color32 {
        match self.kind() {
            MarkingKind::Selection => Color32::WHITE,
            MarkingKind::HoveredParsed => Color32::GOLD,
            MarkingKind::HoveredParseErr => Color32::LIGHT_RED,
            MarkingKind::SearchResult => Color32::from_rgb(252, 15, 192),
        }
    }
}

/// The kind of marked location.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MarkingKind {
    /// The location is marked, because the user selected it.
    Selection,
    /// The location is marked, because the user hovered the parsed value.
    HoveredParsed,
    /// The location is marked, because the user hovered a parsing error.
    HoveredParseErr,
    /// The location is marked, because it was found by a search.
    SearchResult,
}

/// Renders the given marked locations on the given bar window.
pub fn render_locations_on_bar(
    ui: &mut Ui,
    bar_rect: Rect,
    bar_window: Window,
    marked_locations: &mut MarkedLocations,
) {
    // first bin locations to similar y offsets, so that they don't overlap
    let mut location_dots_by_y_bins = BTreeMap::<u32, Vec<_>>::new();

    /// The bin size where close values are displayed in one line.
    const BIN_SIZE: u32 = 5;

    /// The transparency used for the locations on the bar.
    const TRANSPARENCY: f64 = 0.5;

    let bar_start = offset_on_bar(bar_rect, bar_window, bar_window.start()).unwrap();
    let bar_end = offset_on_bar(bar_rect, bar_window, bar_window.end() - Len::from(1)).unwrap();

    for location in marked_locations.iter_window(bar_window) {
        let start_pos = offset_on_bar(bar_rect, bar_window, location.window.start());
        let end_pos = offset_on_bar(bar_rect, bar_window, location.window.end());

        let draw_range;
        let bin_size_x = bar_rect.width() / 16.0;

        match (start_pos, end_pos) {
            (None, None) => continue,
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

                    location_dots_by_y_bins
                        .entry(bin)
                        .or_default()
                        .push(location);
                    continue;
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
                color::lerp(location.inner_color(), Color32::TRANSPARENT, TRANSPARENCY),
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

        trace_path(ui.painter(), &points, 1.0, 0.0, location.border_color());
    }

    let mut mark_location = None;

    for (y, mut locations) in location_dots_by_y_bins {
        locations.sort_by_key(|location| (location.window().start(), location.window().end()));

        for (i, location) in locations.iter().enumerate() {
            let center = pos2(
                bar_rect.left()
                    + bar_rect.width() * ((i + 1) as f32 / (locations.len() + 1) as f32),
                y as f32,
            );

            let is_hovered = Some(*location) == marked_locations.hovered();
            let radius = if is_hovered {
                bar_rect.width() / 8.0
            } else {
                bar_rect.width() / 16.0
            };

            ui.painter().circle(
                center,
                radius,
                color::lerp(location.inner_color(), Color32::TRANSPARENT, TRANSPARENCY),
                Stroke::new(radius / 4.0, location.border_color()),
            );

            let hovered = ui
                .input(|input| input.pointer.latest_pos())
                .map(|pos| (pos - center).length() < radius)
                .unwrap_or(false);
            if hovered {
                mark_location = Some((*location).clone());
            }
        }
    }

    if let Some(mark_location) = mark_location {
        marked_locations.mark_hovered(mark_location);
    }
}
