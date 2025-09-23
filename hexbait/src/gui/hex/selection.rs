//! Handles selection related things.

use std::ops::RangeInclusive;

use egui::{Context, Response};

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

    /// Checks if the selection process should end.
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
        self.selection.as_ref().map(|selection| {
            if selection.start() <= selection.end() {
                selection.clone()
            } else {
                *selection.end()..=*selection.start()
            }
        })
    }
}
