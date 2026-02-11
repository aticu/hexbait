//! Implements the structures storing the state of the hexbait application.

use std::path::PathBuf;

use hexbait_common::{Endianness, Input};
pub use parse_state::ParseState;
pub use scroll_state::{InteractionState, ScrollState, Scrollbar};
pub use search_state::SearchState;
pub use selection_state::SelectionState;
pub use settings::{Settings, ViewKind};
pub use statistics_display_state::StatisticsDisplayState;

use crate::{
    gui::marking::{MarkedLocation, MarkedLocations, MarkingKind},
    statistics::StatisticsHandler,
};

mod parse_state;
mod scroll_state;
mod search_state;
mod selection_state;
mod settings;
mod statistics_display_state;

/// The state of the hexbait application.
pub struct State {
    /// The user settings.
    pub settings: Settings,
    /// The search state.
    pub search: SearchState,
    /// The state of the scrollbars.
    pub scroll_state: ScrollState,
    /// The state of the hex view selection.
    pub selection_state: SelectionState,
    /// The state of the statistics display.
    pub statistics_display_state: StatisticsDisplayState,
    /// The state of the hexbait parser.
    pub parse_state: ParseState,
    /// The statistics handler used to collect statistics about the input.
    pub statistics_handler: StatisticsHandler,
    /// The marked locations.
    pub marked_locations: MarkedLocations,
    /// The currently selected endianness.
    pub endianness: Endianness,
}

impl State {
    /// Creates new state for the hexbait application.
    pub fn new(input: &Input, custom_parser: Option<PathBuf>) -> State {
        State {
            settings: Settings::new(),
            search: SearchState::new(input),
            scroll_state: ScrollState::new(input),
            selection_state: SelectionState::new(),
            statistics_display_state: StatisticsDisplayState::new(),
            parse_state: ParseState::new(custom_parser),
            statistics_handler: StatisticsHandler::new(input.clone()),
            marked_locations: MarkedLocations::new(),
            endianness: Endianness::native(),
        }
    }

    /// This method is called once at the end of a frame to do necessary bookkeeping.
    pub fn end_of_frame(&mut self) {
        self.statistics_handler
            .end_of_frame(self.scroll_state.changed());

        self.marked_locations
            .remove_where(|loc| loc.kind() == MarkingKind::SearchResult);
        for result in self.search.searcher.results().iter() {
            self.marked_locations
                .add(MarkedLocation::new(*result, MarkingKind::SearchResult));
        }
        self.marked_locations.end_of_frame();

        if self.parse_state.sync_parse_offset_to_selection_start
            && let Some(selection) = self.selection_state.selection()
        {
            self.parse_state.parse_offset = selection.start().as_u64().to_string();
        }
    }
}

/// The different things that can be displayed in the main views.
#[derive(Debug, Clone, Copy)]
pub enum DisplayType {
    /// Show statistics of the selected byte window.
    Statistics,
    /// Show a hexview at the start of the selected byte window.
    Hexview,
}
