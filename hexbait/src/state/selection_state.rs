//! Implements the state container for the selection in the hex view.

use std::ops::RangeInclusive;

use hexbait_common::AbsoluteOffset;

use crate::window::Window;

/// The state of the selection.
pub struct SelectionState {
    /// The current selection.
    selection: Option<RangeInclusive<AbsoluteOffset>>,
    /// The previous selection.
    prev_selection: Option<RangeInclusive<AbsoluteOffset>>,
    /// Whether or not a selection is in progress.
    selecting: bool,
}

impl SelectionState {
    /// Creates a new selection state.
    pub fn new() -> Self {
        Self {
            selection: None,
            prev_selection: None,
            selecting: false,
        }
    }

    /// Starts a selection at the given offset.
    fn start_selection(&mut self, offset: AbsoluteOffset) {
        self.selecting = true;
        self.prev_selection = self.selection();
        self.selection = Some(offset..=offset);
    }

    /// Ends the current selection.
    pub fn handle_mouse_release(&mut self) {
        if self.selecting {
            self.selecting = false;
            if let Some(selection) = &self.selection
                && selection.start() == selection.end()
                && self.selection == self.prev_selection
            {
                self.selection = None;
            }
        }
    }

    /// Handles an interaction with the given byte.
    pub fn handle_interaction(&mut self, offset: AbsoluteOffset, clicked: bool) {
        if self.selecting {
            self.selection = Some(*self.selection.as_ref().unwrap().start()..=offset);
        } else if clicked {
            self.start_selection(offset);
        }
    }

    /// Returns the current selection.
    pub fn selection(&self) -> Option<RangeInclusive<AbsoluteOffset>> {
        self.selection.as_ref().map(|selection| {
            if selection.start() <= selection.end() {
                selection.clone()
            } else {
                *selection.end()..=*selection.start()
            }
        })
    }

    /// Returns the selected window.
    pub fn selected_window(&self) -> Option<Window> {
        self.selection().map(Window::from)
    }
}

impl Default for SelectionState {
    fn default() -> Self {
        SelectionState::new()
    }
}
