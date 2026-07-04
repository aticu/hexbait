//! Renders the marking menu in the GUI.

use egui::Ui;
use hexbait_common::Input;

use crate::state::State;

/// Shows the marking menu in the GUI.
pub fn show(ui: &mut Ui, state: &mut State, _: &Input) {
    ui.label("Mark name:");
    ui.text_edit_singleline(&mut state.marked_locations.current_mark_name);
}
