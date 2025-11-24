//! Implements the structures storing the state of the hexbait application.

pub use scroll_state::{DisplaySuggestion, InteractionState, ScrollState, Scrollbar};
pub use search_state::SearchState;
pub use settings::Settings;

use crate::data::Input;

mod scroll_state;
mod search_state;
mod settings;

/// The state of the hexbait application.
pub struct State {
    /// The user settings.
    pub settings: Settings,
    /// The search state.
    pub search: SearchState,
    /// The state of the scrollbars.
    pub scroll_state: ScrollState,
}

impl State {
    /// Creates new state for the hexbait application.
    pub fn new(input: &Input) -> State {
        State {
            settings: Settings::new(),
            search: SearchState::new(input),
            scroll_state: ScrollState::new(input),
        }
    }
}
