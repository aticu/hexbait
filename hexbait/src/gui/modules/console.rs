//! Implements the console module.

use egui::Ui;
use hexbait_common::Input;
use hexbait_lang::{View, ir::lower_expr, parse_expr};

use crate::{
    gui::modules::parsed_value::show_value,
    state::{ConsoleEntry, State},
};

/// Shows a hexbait console in the GUI.
pub fn show(ui: &mut Ui, state: &mut State, input: &Input) {
    if ui
        .text_edit_singleline(state.console.current_text())
        .lost_focus()
    {
        let query = state.console.current_text().to_string();
        state.console.current_text().clear();

        if !query.is_empty() {
            let parse = parse_expr(&query);
            if !parse.errors.is_empty() {
                dbg!(query, parse.errors);
                panic!();
            }
            let ir = lower_expr(parse.ast);

            let view = View::from_input(input.clone());
            let result = hexbait_lang::eval_expr(&ir, state.endianness, view);

            state
                .console
                .add_history_entry(ConsoleEntry { query, result });
        }
    }

    for i in 0..state.console.history().len() {
        let entry = state.console.history()[i].clone();

        ui.label(&entry.query);

        show_value(
            ui,
            state,
            hexbait_lang::ir::path::Path::new(),
            None,
            &entry.result.value,
            &entry.result.diagnostics,
        );
    }
}
