//! Renders the marking menu in the GUI.

use egui::{Align, Button, FontSelection, RichText, Stroke, Ui, text::LayoutJob};
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

    ui.add_space(state.settings.char_height());

    mark_list(ui, state);
}

/// Displays the mark list.
fn mark_list(ui: &mut Ui, state: &mut State) {
    ui.label(format!(
        "Total marks: {}",
        state.marked_locations.total_count()
    ));
    for ty in state.marked_locations.types() {
        ui.horizontal(|ui| {
            let disabled = state.marking_menu.mark_type_is_disabled(ty);

            let label_text = |text: &str| {
                let rich_text = RichText::new(text);
                if disabled {
                    rich_text.strikethrough()
                } else {
                    rich_text
                }
            };

            let mut parts = vec![label_text(ty.description())];
            match ty.name() {
                None => {}
                Some(None) => parts.extend([
                    label_text(" ("),
                    label_text("unnamed").italics(),
                    label_text(")"),
                ]),
                Some(Some(name)) => {
                    parts.extend([label_text(" ("), label_text(name), label_text(")")])
                }
            }
            parts.push(label_text(": "));
            parts.push(label_text(
                &state.marked_locations.count_of_type(ty).to_string(),
            ));

            let mut job = LayoutJob::default();
            let style = ui.style();
            for part in parts {
                part.append_to(&mut job, style, FontSelection::Default, Align::Center);
            }

            let mut button = Button::new(job);

            if !disabled {
                button = button
                    .fill(ty.border_color())
                    .stroke(Stroke::new(1.0, ty.inner_color()));
            }

            if ui.add(button).clicked() {
                state.marking_menu.toggle_mark_type_disabled(ty.clone());
            }
        });
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
