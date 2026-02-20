//! Implements the content module.

use egui::{Align, Layout, Ui, UiBuilder};
use hexbait_common::Input;

use crate::{
    gui::modules,
    state::{DisplayType, State, ViewKind},
};

/// Shows the input content in the GUI.
pub fn show(ui: &mut Ui, state: &mut State, input: &Input) {
    ui.scope_builder(
        UiBuilder::new()
            .max_rect(ui.max_rect().intersect(ui.cursor()))
            .layout(Layout::left_to_right(Align::Min)),
        |ui| {
            modules::scrollbars::show(ui, state, input);

            let display_type = match state.settings.view_kind() {
                ViewKind::Auto => state.scroll_state.display_suggestion,
                ViewKind::ForceHexView => DisplayType::Hexview,
                ViewKind::ForceStatisticsView => DisplayType::Statistics,
            };

            let display_fn = match display_type {
                DisplayType::Statistics => modules::statistics_display::show,
                DisplayType::Hexview => modules::hex::show,
            };

            display_fn(ui, state, input);
        },
    );
}
