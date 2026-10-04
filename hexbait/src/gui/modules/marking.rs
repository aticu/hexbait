//! Renders the marking menu in the GUI.

use egui::Ui;
use hexbait_common::{AbsoluteOffset, Input, Len};

use crate::{marking::MarkType, state::State, window::Window};

/// Shows the marking menu in the GUI.
pub fn show(ui: &mut Ui, state: &mut State, _: &Input) {
    ui.label("Mark name:");
    ui.text_edit_singleline(&mut state.marked_locations.current_mark_name);

    if state
        .marked_locations
        .contains_marks_of_type(MarkType::SearchResult)
        && ui.button("convert search results to marks").clicked()
    {
        state.marked_locations.convert_marks_to(
            MarkType::SearchResult,
            MarkType::UserMark {
                name: state.marked_locations.current_mark_name.clone(),
            },
        );
    }

    if state
        .marked_locations
        .contains_marks_of_type(MarkType::UserMark {
            name: state.marked_locations.current_mark_name.clone(),
        })
        && ui.button("copy all marks of the current name").clicked()
    {
        let mut out = String::new();
        for mark in state
            .marked_locations
            .iter_marks_of_type(&MarkType::UserMark {
                name: state.marked_locations.current_mark_name.clone(),
            })
            .unwrap()
        {
            out.push_str(&format!("{}\n", mark.window.start()));
        }
        // remove the final newline
        out.pop();
        ui.ctx().copy_text(out);
    }

    if state.marked_locations.iter_user_marks().next().is_some()
        && ui.button("copy all user marks").clicked()
    {
        let mut out = String::new();
        for mark in state.marked_locations.iter_user_marks() {
            let MarkType::UserMark { name } = mark.ty else {
                unreachable!()
            };

            out.push_str(&format!("{}:{name}\n", mark.window.start()));
        }
        // remove the final newline
        out.pop();
        ui.ctx().copy_text(out);
    }

    if ui.button("paste marks").clicked()
        && let Ok(text) = state.clipboard.get_text()
    {
        for (window, ty) in parse_marks(&text) {
            state.marked_locations.add(window, ty);
        }
    }

    if !state.format_discovery.is_in_format_discovery_mode()
        && ui.button("enter format discovery mode").clicked()
    {
        state
            .format_discovery
            .enter(state.marked_locations.current_mark_name.clone());
    }
    if state.format_discovery.is_in_format_discovery_mode()
        && ui.button("leave format discovery mode").clicked()
    {
        state.format_discovery.exit();
    }
}

/// Parses marks from the give string.
fn parse_marks(input: &str) -> Vec<(Window, MarkType)> {
    let mut out = Vec::new();

    for line in input.lines() {
        let Some((start, name)) = line.rsplit_once(':') else {
            continue;
        };
        let Ok(start) = start.parse::<u64>() else {
            continue;
        };

        out.push((
            Window::from_start_len(AbsoluteOffset::from(start), Len::from(1)),
            MarkType::UserMark {
                name: name.to_string(),
            },
        ));
    }

    out
}
