//! Implements marking of locations.

use std::{collections::BTreeMap, num::NonZeroUsize, ops::Index};

use egui::{Color32, Rect, Stroke, Ui, pos2};

use crate::window::Window;

use super::{color, zoombars::offset_on_bar};

/// The ID of a marked location.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MarkedLocationId {
    /// The offset of the marked location.
    offset: u64,
    /// A unique id.
    id: NonZeroUsize,
}

/// Stores the marked locations to highlight.
pub struct MarkedLocations {
    /// The locations that should be highlighted.
    locations: BTreeMap<u64, Vec<MarkedLocation>>,
    /// The next ID that will be used for a marked location.
    next_id: NonZeroUsize,
    /// The currently hovered location.
    hovered_location: Option<MarkedLocationId>,
}

impl MarkedLocations {
    /// Creates a new empty list of marked locations.
    pub fn new() -> MarkedLocations {
        MarkedLocations {
            locations: BTreeMap::new(),
            next_id: NonZeroUsize::MIN,
            hovered_location: None,
        }
    }

    /// Adds a new marked location to be displayed.
    pub fn add(&mut self, mut marked_location: MarkedLocation) -> MarkedLocationId {
        let offset = marked_location.window.start();
        let id = MarkedLocationId {
            offset,
            id: self.next_id,
        };
        self.next_id = id.id.checked_add(1).unwrap_or(NonZeroUsize::MIN);

        marked_location.id = Some(id);

        self.locations
            .entry(marked_location.window.start())
            .or_default()
            .push(marked_location);

        id
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
        if let Some(id) = self.hovered_location {
            Some(&self[id])
        } else {
            None
        }
    }

    /// Returns a mutable reference to the currently hovered location id.
    pub fn hovered_location_id_mut(&mut self) -> &mut Option<MarkedLocationId> {
        &mut self.hovered_location
    }
}

impl Index<MarkedLocationId> for MarkedLocations {
    type Output = MarkedLocation;

    fn index(&self, index: MarkedLocationId) -> &Self::Output {
        self.locations
            .get(&index.offset)
            .unwrap()
            .iter()
            .find(|location| location.id == Some(index))
            .unwrap()
    }
}

/// A marked location.
pub struct MarkedLocation {
    /// The window that this location refers to.
    window: Window,
    /// The kind of the marked location.
    kind: MarkingKind,
    /// The ID of this location.
    ///
    /// This will be filled when the location is added to the marked locations.
    id: Option<MarkedLocationId>,
}

impl MarkedLocation {
    /// Creates a new marked location on the given window.
    pub fn new(window: Window, kind: MarkingKind) -> MarkedLocation {
        MarkedLocation {
            window,
            kind,
            id: None,
        }
    }

    /// The window covered by this marked location.
    pub fn window(&self) -> Window {
        self.window
    }

    /// The kind of the marked location.
    pub fn kind(&self) -> MarkingKind {
        self.kind
    }

    /// The color of this marked location.
    pub fn color(&self) -> Color32 {
        match self.kind() {
            MarkingKind::Selection => Color32::WHITE,
        }
    }
}

/// The kind of marked location.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MarkingKind {
    /// The location is marked, because the user selected it.
    Selection,
}

/// Renders the given marked locations on the given bar window.
pub fn render_locations_on_bar(
    ui: &mut Ui,
    bar_rect: Rect,
    bar_window: Window,
    marked_locations: &mut MarkedLocations,
    new_hovered: &mut Option<MarkedLocationId>,
    currently_hovered: Option<MarkedLocationId>,
) {
    // first bin locations to similar y offsets, so that they don't overlap
    let mut location_dots_by_y_bins = BTreeMap::<u32, Vec<_>>::new();

    /// The bin size where close values are displayed in one line.
    const BIN_SIZE: u32 = 5;

    /// The transparency used for the locations on the bar.
    const TRANSPARENCY: f64 = 0.7;

    let bar_start = offset_on_bar(bar_rect, bar_window, bar_window.start()).unwrap();
    let bar_end = offset_on_bar(bar_rect, bar_window, bar_window.end() - 1).unwrap();

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
                    location_dots_by_y_bins
                        .entry(((start.y as u32) / BIN_SIZE) * BIN_SIZE)
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
            pos2(end_x + (bar_rect.width() / 16.0), draw_range.end.y),
        );

        for rect in [top_rect, middle_rect, bottom_rect] {
            ui.painter().rect_filled(
                rect,
                0.0,
                color::lerp(location.color(), Color32::TRANSPARENT, TRANSPARENCY),
            );
        }

        // TODO: round x position to the nearest 16th of the bar
        // TODO: draw range in three parts: first line, last line and block in between
    }

    for (y, mut locations) in location_dots_by_y_bins {
        locations.sort_by_key(|location| (location.window().start(), location.window().end()));

        for (i, location) in locations.iter().enumerate() {
            let center = pos2(
                bar_rect.left()
                    + bar_rect.width() * ((i + 1) as f32 / (locations.len() + 1) as f32),
                y as f32,
            );

            let is_hovered = location.id == currently_hovered;
            let radius = if is_hovered {
                bar_rect.width() / 8.0
            } else {
                bar_rect.width() / 16.0
            };

            let color = location.color();

            ui.painter().circle(
                center,
                radius,
                color::lerp(color, Color32::TRANSPARENT, TRANSPARENCY),
                Stroke::new(radius / 4.0, color),
            );

            let hovered = ui
                .input(|input| input.pointer.latest_pos())
                .map(|pos| (pos - center).length() < radius)
                .unwrap_or(false);
            if hovered {
                *new_hovered = location.id;
            }
        }
    }
}
