//! Implements the inspector module.
//!
//! This modules switches between the data inspector and the statistics display.

use egui::Ui;
use hexbait_common::Input;

use crate::{
    gui::modules,
    state::{DisplayType, State, ViewKind},
};

/// Shows the input content in the GUI.
pub fn show(ui: &mut Ui, state: &mut State, input: &Input) {
    let display_type = match state.settings.view_kind() {
        ViewKind::Auto => state.scroll_state.display_suggestion,
        ViewKind::ForceHex => DisplayType::Hexview,
        ViewKind::ForceOverview => DisplayType::Overview,
    };

    let display_fn = match display_type {
        DisplayType::Overview => modules::statistics_display::show,
        DisplayType::Hexview => modules::data_inspector::show,
    };

    display_fn(ui, state, input);
}
