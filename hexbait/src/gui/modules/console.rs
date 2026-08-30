//! Implements the console module.

use egui::Ui;
use hexbait_common::{AbsoluteOffset, Input, Len};
use hexbait_lang::{ValueKind, View, ir::lower_expr, parse_expr};

use crate::{
    gui::{diagnostic_emitter::emit_diagnostics, modules::parsed_value::show_value},
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
            let parse = parse_expr("console_input", &query);
            emit_diagnostics(ui, &parse);
            if !parse.diagnostics.is_empty() {
                parse.emit_diagnostics_to_stderr();
                std::process::exit(1);
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

        if let ValueKind::Integer(int) = &entry.result.value.kind
            && let Ok(int) = u64::try_from(int)
            && Len::from(int) < input.len()
            && ui.button("Jump to offset").clicked()
        {
            state
                .scroll_state
                .rearrange_bars_for_point(0, AbsoluteOffset::from(int));
        }
    }
}
