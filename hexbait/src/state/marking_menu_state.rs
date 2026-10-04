//! Implements the state of the marking menu.

use std::collections::HashSet;

use crate::marking::MarkType;

/// Stores the state for the marking menu.
pub struct MarkingMenuState {
    /// The disabled mark types.
    disabled_types: HashSet<MarkType>,
}

impl MarkingMenuState {
    /// Creates a new empty marking menu state.
    pub fn new() -> MarkingMenuState {
        MarkingMenuState {
            disabled_types: HashSet::new(),
        }
    }

    /// Returns `true` if the given mark type is disabled.
    pub fn mark_type_is_disabled(&self, mark_type: &MarkType) -> bool {
        self.disabled_types.contains(mark_type)
    }

    /// Toggles the disabled state for the given mark type.
    pub fn toggle_mark_type_disabled(&mut self, mark_type: MarkType) {
        if !self.disabled_types.remove(&mark_type) {
            self.disabled_types.insert(mark_type);
        }
    }
}

impl Default for MarkingMenuState {
    fn default() -> Self {
        MarkingMenuState::new()
    }
}
