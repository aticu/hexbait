//! Implements the state container for the scrollbars.

use std::hash::{Hash as _, Hasher as _};

use hexbait_common::{AbsoluteOffset, ChangeState, Len, RelativeOffset};

use crate::{data::Input, gui::cached_image::CachedImage, state::Settings, window::Window};

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
    /// The number of rows that have been scrolled down from the start in hex view.
    pub hex_scroll_offset: u64,
    /// Store the file size so the scroll state can independently compute windows.
    file_size: Len,
    /// The height of the zoombars in the current frame.
    height: f32,
    /// The height of a character in the hex view.
    hex_char_height: f32,
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
            hex_scroll_offset: 0,
            file_size: input.len(),
            // the height is irrelevant for the first frame since we draw anyway
            height: 0.0,
            // start with a random non-zero height
            hex_char_height: 20.0,
            // the previous selection state is irrelevant for the first frame since we draw anyway
            prev_selection_state: 0,
        }
    }

    /// Sets the height of the scroll scroll bar area.
    pub fn update_parameters(&mut self, height: f32, settings: &Settings) {
        let state = self.selection_state();
        self.prev_selection_state = state;

        self.hex_char_height = settings.char_height();
        self.height = height;
    }

    /// Returns the size of the file this scroll state is for.
    pub fn file_size(&self) -> Len {
        self.file_size
    }

    /// The window of the first bar.
    pub fn first_window(&self) -> Window {
        Window::from_start_len(AbsoluteOffset::ZERO, self.file_size())
    }

    /// Returns the window selected by the scroll state.
    pub fn selected_window(&self) -> Window {
        let mut window = self.first_window();
        let total_hexdump_bytes = self.total_hexdump_bytes();

        for bar in &self.scrollbars {
            window = bar.window(window, total_hexdump_bytes);
        }

        window
    }

    /// Returns the start offset of a hexdump showing the current window.
    pub fn hex_start(&self) -> AbsoluteOffset {
        let mut start = AbsoluteOffset::ZERO;
        for bar in &self.scrollbars {
            start += bar.selection_start;
        }
        let end = start
            + self
                .scrollbars
                .last()
                .map(|bar| bar.selection_len)
                .unwrap_or(self.file_size());

        if start.is_start_of_file() {
            // ensure that the correction below does not make the start invisible

            AbsoluteOffset::ZERO
        } else if end > AbsoluteOffset::ZERO + self.file_size() - Len::from(16) {
            // over-correct towards the end to ensure it's guaranteed to be visible

            AbsoluteOffset::from(self.file_size().round_up(16).as_u64())
                - self.total_hexdump_bytes()
        } else {
            start.align_down(16)
        }
    }

    /// The number of bytes that a hexdump can show at once.
    pub fn total_hexdump_bytes(&self) -> Len {
        let total_rows = (self.height.trunc() as u64).max(1);
        Len::from(total_rows * 16)
    }

    /// The number of bytes visible at once in hex view.
    fn hex_visible_window_size(&self) -> Len {
        Len::from((self.height / self.hex_char_height).trunc() as u64 * 16)
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
    pub fn changed(&self) -> ChangeState {
        let state = self.selection_state();

        match self.prev_selection_state == state {
            true => ChangeState::Unchanged,
            false => ChangeState::Changed,
        }
    }

    /// Rearranges the scrollbars to focus on the given point.
    ///
    /// Bars before `start_bar` remain unchanged, if `point` lies within them, otherwise they are
    /// shifted.
    pub fn rearrange_bars_for_point(&mut self, start_bar: usize, point: AbsoluteOffset) {
        let center_bar_on_point = |bar: &mut Scrollbar, window: Window| {
            let point_on_bar = point - window.start();
            let half_len = bar.selection_len / 2;

            if point_on_bar < half_len {
                bar.selection_start = RelativeOffset::ZERO;
            } else if point_on_bar + half_len > window.size() {
                bar.selection_start =
                    RelativeOffset::from((window.size() - bar.selection_len).as_u64());
            } else {
                bar.selection_start = RelativeOffset::from((point_on_bar - half_len).as_u64());
            }
        };

        let total_bytes_in_hexview = self.total_hexdump_bytes();
        let mut window = Window::from_start_len(AbsoluteOffset::ZERO, self.file_size());
        let mut parent_window = window;
        for bar in self.scrollbars.iter_mut().take(start_bar + 1) {
            let selected_window = bar.window(window, total_bytes_in_hexview);
            if selected_window.contains(point) {
                parent_window = window;
                window = selected_window;
                continue;
            }

            center_bar_on_point(bar, window);
            parent_window = window;
            window = bar.window(window, total_bytes_in_hexview);
        }
        self.scrollbars.drain(start_bar + 1..);

        // if the current bar is full, re-do it instead
        if self.scrollbars[start_bar].selection_len == parent_window.size() {
            self.scrollbars.remove(start_bar);
        }

        while window.size() > total_bytes_in_hexview {
            let selection_len = std::cmp::max(
                Len::from((0.05f64 * window.size().as_u64() as f64) as u64),
                total_bytes_in_hexview,
            );

            let mut bar = Scrollbar::new(window.size());
            bar.selection_len = selection_len;

            center_bar_on_point(&mut bar, window);
            window = bar.window(window, total_bytes_in_hexview);

            self.scrollbars.push(bar);
        }

        // the algorithm expects a full bar at the end, so provide it
        self.scrollbars.push(Scrollbar::new(window.size()));

        let hex_window_size = self.hex_visible_window_size();
        let tentative_hex_offset = Len::from(
            (point - window.start())
                .as_u64()
                .saturating_sub(hex_window_size.as_u64() / 2),
        );

        let unrounded_hex_offset =
            if tentative_hex_offset + hex_window_size > total_bytes_in_hexview {
                total_bytes_in_hexview - hex_window_size
            } else {
                tentative_hex_offset
            };

        self.hex_scroll_offset = unrounded_hex_offset.as_u64() / 16;
    }

    /// Enforces the invariant that no fully selected bar can be in the middle.
    pub fn enforce_no_full_bar_in_middle_invariant(&mut self) {
        let mut prev_len = self.file_size();
        for (i, bar) in self.scrollbars[..self.scrollbars.len() - 1]
            .iter()
            .enumerate()
        {
            if bar.selection_start == RelativeOffset::ZERO && bar.selection_len == prev_len {
                // remove other bars behind this one
                self.scrollbars.truncate(i + 1);
                break;
            }

            prev_len = bar.selection_len;
        }
    }

    /// Scrolls up by the given amount.
    pub fn scroll_up(&mut self, bar: usize, amount: u64) {
        let mut amount_left = amount;
        for i in (0..=bar).rev() {
            if amount_left == 0 {
                break;
            }

            amount_left = self.scrollbars[i].scroll_up(amount_left);
        }
    }

    /// Scrolls up by the given amount.
    pub fn scroll_down(&mut self, bar: usize, amount: u64, min_size: Len) {
        let mut parent_size = Vec::with_capacity(bar + 1);
        let mut window = Window::from_start_len(AbsoluteOffset::ZERO, self.file_size());

        parent_size.push(window.size());
        for i in 0..=bar {
            window = self.scrollbars[i].window(window, min_size);
            parent_size.push(window.size());
        }

        let mut amount_left = amount;
        for i in (0..=bar).rev() {
            if amount_left == 0 {
                break;
            }

            amount_left = self.scrollbars[i].scroll_down(amount_left, parent_size[i]);
        }
    }
}

/// The state of a single scrollbar.
#[derive(Debug)]
pub struct Scrollbar {
    /// The start offset of the selection relative to the previous scrollbar's start.
    selection_start: RelativeOffset,
    /// The length of the selected window.
    selection_len: Len,
    /// The cached image for this scrollbar.
    ///
    /// This depends on the selection of the scrollbar as well as the full window of the bar.
    pub cached_image: CachedImage<((RelativeOffset, Len), Window)>,
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

    /// Sets the selection for this scroll bar.
    pub fn set_selection(&mut self, start: RelativeOffset, len: Len) {
        self.selection_start = start;
        self.selection_len = len;
    }

    /// Returns the relative start of the selection within the window (range: `0.0..=1.0`).
    pub fn relative_selection_start(&self, window: Window) -> f64 {
        self.selection_start.as_u64() as f64 / window.size().as_u64() as f64
    }

    /// Returns the relative end of the selection within the window (range: `0.0..=1.0`).
    pub fn relative_selection_end(&self, window: Window) -> f64 {
        (self.selection_start + self.selection_len).as_u64() as f64 / window.size().as_u64() as f64
    }

    /// Scrolls the bar up by the given amount of bytes.
    ///
    /// The returned value specifies how much the bar was "overscrolled".
    pub fn scroll_up(&mut self, scroll_amount: u64) -> u64 {
        let start = self.selection_start.as_u64();

        if scroll_amount > start {
            self.selection_start = RelativeOffset::ZERO;

            scroll_amount - start
        } else {
            self.selection_start = RelativeOffset::from(start - scroll_amount);

            0
        }
    }

    /// Scrolls the bar down by the given amount of bytes.
    ///
    /// The returned value specifies how much the bar was "overscrolled".
    pub fn scroll_down(&mut self, scroll_amount: u64, bar_len: Len) -> u64 {
        let start = self.selection_start.as_u64();
        let last_possible_position = (bar_len - self.selection_len).as_u64();

        if start.saturating_add(scroll_amount) > last_possible_position {
            self.selection_start = RelativeOffset::from(last_possible_position);

            start.saturating_add(scroll_amount) - last_possible_position
        } else {
            self.selection_start = RelativeOffset::from(start + scroll_amount);

            0
        }
    }

    /// Returns state used by the cached image.
    pub fn state_for_cached_image(&self) -> (RelativeOffset, Len) {
        (self.selection_start, self.selection_len)
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
