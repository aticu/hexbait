//! Implements the structures storing the state of the hexbait application.

pub use scroll_state::{InteractionState, ScrollState, Scrollbar};
pub use search_state::SearchState;
pub use selection_state::SelectionState;
pub use settings::{Settings, ViewKind};

use crate::{
    data::Input,
    gui::{cached_image::CachedImage, marking::MarkedLocations},
    window::Window,
};

mod scroll_state;
mod search_state;
mod selection_state;
mod settings;

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
    /// The marked locations.
    pub marked_locations: MarkedLocations,
    /// The cached signature display image.
    pub cached_signature_display: CachedImage<(Window, u8, f32)>,
}

impl State {
    /// Creates new state for the hexbait application.
    pub fn new(input: &Input) -> State {
        State {
            settings: Settings::new(),
            search: SearchState::new(input),
            scroll_state: ScrollState::new(input),
            selection_state: SelectionState::new(),
            marked_locations: MarkedLocations::new(),
            cached_signature_display: CachedImage::new(),
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
