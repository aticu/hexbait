//! Implements the state container for the scrollbars.

use std::hash::{Hash as _, Hasher as _};

use hexbait_common::{ChangeState, Len, RelativeOffset};

use crate::{
    data::{DataSource, Input},
    gui::cached_image::CachedImage,
    window::Window,
};

/// The suggestion of what to display in the main content view.
pub enum DisplaySuggestion {
    /// With the current scroll selection state an overview should be shown.
    Overview,
    /// With the current scroll selection state a hexview should be shown.
    Hexview,
}

/// The state of the scrolling.
pub struct ScrollState {
    /// The scrollbars.
    pub scrollbars: Vec<Scrollbar>,
    /// What to display in the main content view.
    pub display_suggestion: DisplaySuggestion,
    /// How the user is currently interacting with the scrollbars.
    pub interaction_state: InteractionState,
    /// The height of the zoombars in the current frame.
    height: f32,
    /// The selection state in the previous frame.
    prev_selection_state: u64,
}

impl ScrollState {
    /// Initializes the scrolling state.
    pub fn new(input: &Input) -> ScrollState {
        ScrollState {
            scrollbars: vec![Scrollbar::new(input.len())],
            display_suggestion: DisplaySuggestion::Overview,
            interaction_state: InteractionState::None,
            // the height is irrelevant for the first frame since we draw anyway
            height: 0.0,
            // the previous selection state is irrelevant for the first frame since we draw anyway
            prev_selection_state: 0,
        }
    }

    /// Creates a hash of the zoombar selection state.
    fn selection_state(&self) -> u64 {
        let mut hasher = std::hash::DefaultHasher::new();

        self.height.to_ne_bytes().hash(&mut hasher);
        self.scrollbars.len().hash(&mut hasher);
        for bar in &self.scrollbars {
            bar.selection_start.hash(&mut hasher);
            bar.selection_len.hash(&mut hasher);
        }

        hasher.finish()
    }

    /// Determines if the zoombar selection state changed since the last call to this method.
    pub fn changed(&mut self, height: f32) -> ChangeState {
        self.height = height;

        let state = self.selection_state();
        let prev_state = self.prev_selection_state;
        self.prev_selection_state = state;

        match prev_state == state {
            true => ChangeState::Unchanged,
            false => ChangeState::Changed,
        }
    }
}

/// The state of a single scrollbar.
#[derive(Debug)]
pub struct Scrollbar {
    /// The start offset of the selection relative to the previous scrollbar's start.
    pub selection_start: RelativeOffset,
    /// The length of the selected window.
    pub selection_len: Len,
    /// The cached image for this scrollbar.
    ///
    /// This depends on the selection of the scrollbar as well as the full window of the bar.
    pub cached_image: CachedImage<(RelativeOffset, Len, Window)>,
}

impl Scrollbar {
    /// Creates a new scrollbar for the given length.
    pub fn new(len: Len) -> Scrollbar {
        Scrollbar {
            selection_start: RelativeOffset::ZERO,
            selection_len: len,
            cached_image: CachedImage::new(),
        }
    }

    /// Returns the window of this scrollbar given the parent window.
    pub fn window(&self, parent_window: Window, min_size: Len) -> Window {
        let len = self.selection_len.clamp(min_size, parent_window.size());

        let tentative_window =
            Window::from_start_len(parent_window.start() + self.selection_start, len);

        if tentative_window.end() > parent_window.end() {
            Window::from_start_len(parent_window.end() - len, len)
        } else {
            tentative_window
        }
    }

    /// Centers this bar around the given center position.
    pub fn center_around(&mut self, center: RelativeOffset, window: Window) {
        let half_len = self.selection_len / 2;

        if center < RelativeOffset::from(half_len.as_u64()) {
            self.selection_start = RelativeOffset::ZERO;
        } else if center + half_len > RelativeOffset::from(window.size().as_u64()) {
            self.selection_start =
                RelativeOffset::from(window.size().as_u64()) - self.selection_len;
        } else {
            self.selection_start = center - half_len;
        }
    }
}

/// The state of user interactions with the bars.
#[derive(Debug)]
pub enum InteractionState {
    /// The bars are currently not interacted with.
    None,
    /// A window is currently selected on a bar.
    WindowSelection {
        /// The relative offset within the bar where the selection started.
        ///
        /// This may be larger than `end` if selecting "upwards".
        start: RelativeOffset,
        /// The relative offset within the bar where the selection ended.
        ///
        /// This may be smaller than `start` if selecting "upwards".
        end: RelativeOffset,
        /// The index of the bar that is being selected.
        bar_idx: usize,
    },
    /// A window on a bar is currently being dragged.
    Dragging {
        /// The index of the bar that is being dragged.
        bar_idx: usize,
    },
}

impl InteractionState {
    /// Determines if the `i`th bar is being selected on.
    pub fn selecting_bar(&self, i: usize) -> bool {
        match self {
            InteractionState::WindowSelection { bar_idx, .. } => *bar_idx == i,
            _ => false,
        }
    }
}
