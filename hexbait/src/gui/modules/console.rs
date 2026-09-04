//! Implements the console module.

use egui::Ui;
use hexbait_common::{AbsoluteOffset, Input, Len};
use hexbait_lang::{
    compile::{CompileResult, compile_expr, ir::path::Path},
    eval::{ValueKind, View, eval_expr},
};

use crate::{
    gui::{diagnostic_emitter::emit_diagnostics, modules::parsed_value::show_value},
    state::{ConsoleEntry, State},
};

/// Shows a hexbait console in the GUI.
pub fn show(ui: &mut Ui, state: &mut State, input: &Input) {
    for i in 0..state.console.history().len() {
        let entry = &state.console.history()[i];

        ui.label(&entry.query);

        if let Some(diagnostics) = &entry.diagnostics {
            emit_diagnostics(ui, diagnostics);
        }

        if let Some(result) = &entry.result {
            show_value(
                ui,
                &state.settings,
                &mut state.scroll_state,
                Path::new(),
                None,
                &result.value,
                &result.diagnostics,
            );

            if let ValueKind::Integer(int) = &result.value.kind
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

    if ui
        .text_edit_singleline(state.console.current_text())
        .lost_focus()
    {
        let query = state.console.current_text().to_string();
        state.console.current_text().clear();

        if !query.is_empty() {
            let (ir, diagnostics) = match compile_expr("console_input", &query) {
                CompileResult::NoDiagnostics { ir } => (Some(ir), None),
                CompileResult::WithWarnings { ir, diagnostics } => (Some(ir), Some(diagnostics)),
                CompileResult::Failure { diagnostics } => (None, Some(diagnostics)),
            };

            let result = if let Some(ir) = &ir {
                let view = View::from_input(input.clone());

                Some(eval_expr(ir, state.endianness, view))
            } else {
                None
            };

            state.console.add_history_entry(ConsoleEntry {
                query,
                diagnostics,
                result,
            });
        }
    }
}
