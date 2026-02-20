//! Renders a search screen in the GUI.

use egui::{Button, Checkbox, RichText, Ui};
use hexbait_common::Input;

use crate::state::State;

/// Shows the search screen in the GUI.
pub fn show(ui: &mut Ui, state: &mut State, _: &Input) {
    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            ui.text_edit_singleline(&mut state.search.search_text);
            let search_bytes = match state.search.search_bytes() {
                Ok(bytes) => Some(bytes),
                Err(msg) => {
                    ui.label(RichText::new("⚠").color(ui.visuals().warn_fg_color))
                        .on_hover_ui(|ui| {
                            ui.label(format!("invalid string literal: {msg}"));
                        });
                    None
                }
            };

            let valid_utf8 = search_bytes
                .as_ref()
                .map(|search_bytes| std::str::from_utf8(search_bytes).is_ok())
                .unwrap_or(false);

            ui.checkbox(
                &mut state.search.search_ascii_case_insensitive,
                "ASCII case insensitive",
            );
            ui.add_enabled(
                valid_utf8,
                Checkbox::new(&mut state.search.search_utf16, "include UTF-16"),
            );
            if ui
                .add_enabled(
                    search_bytes
                        .as_ref()
                        .is_some_and(|search_bytes| !search_bytes.is_empty()),
                    Button::new("start search"),
                )
                .clicked()
                && let Some(search_bytes) = &search_bytes
            {
                state.search.searcher.start_new_search(
                    search_bytes,
                    state.search.search_ascii_case_insensitive,
                    state.search.search_utf16 && valid_utf8,
                );
            }
        });

        ui.label(format!(
            "search {:.02}% complete ({} results)",
            state.search.searcher.progress() * 100.0,
            state.search.searcher.results().len()
        ));
    });
}
