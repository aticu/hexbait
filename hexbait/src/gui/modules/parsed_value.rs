//! Implements showing of a parsed value.

use egui::{FontId, Layout, Response, RichText, TextStyle, Ui, UiBuilder};
use hexbait_common::{AbsoluteOffset, Input, RelativeOffset};
use hexbait_lang::{
    ParseErr, ParseErrId, Value, ValueKind, View,
    ir::{
        Symbol,
        path::{Path, PathComponent},
    },
};

use crate::{
    gui::marking::{MarkedLocation, MarkingKind},
    state::State,
};

/// Shows the parsed value module.
pub fn show(ui: &mut Ui, state: &mut State, input: &Input) {
    state.marked_locations.remove_where(|location| {
        location.kind() == MarkingKind::HoveredParsed
            || location.kind() == MarkingKind::HoveredParseErr
    });

    let ir;
    let parse_type = if state.parse_state.parse_type == "custom parser" {
        'parse_type: {
            let Ok(content) = std::fs::read_to_string(
                state
                    .parse_state
                    .custom_parser
                    .as_ref()
                    .expect("if a custom parser is selected it should also exist"),
            ) else {
                break 'parse_type None;
            };

            let parse = hexbait_lang::parse(&content);
            if !parse.errors.is_empty() {
                break 'parse_type None;
            }

            ir = hexbait_lang::ir::lower_file(parse.ast);

            Some(&ir)
        }
    } else {
        state
            .parse_state
            .built_in_format_descriptions
            .get(state.parse_state.parse_type)
    };

    let Some(parse_type) = parse_type else { return };
    let Ok(parse_offset) = state
        .parse_state
        .parse_offset
        .parse()
        .map(AbsoluteOffset::from)
    else {
        return;
    };

    let view = View::from_input(input.clone());
    let view = view.subview(parse_offset.to_relative()..RelativeOffset::from(view.len().as_u64()));
    let result = hexbait_lang::eval_ir(parse_type, view, RelativeOffset::ZERO);
    let hovered = show_value(
        ui,
        state,
        hexbait_lang::ir::path::Path::new(),
        None,
        &result.value,
        &result.errors,
    );

    match hovered {
        HoverInfo::Nothing => (),
        HoverInfo::Value { path } => {
            if let Some(value) = result.value.subvalue_at_path(&path) {
                for range in value.provenance.byte_ranges() {
                    state.marked_locations.add(MarkedLocation::new(
                        (AbsoluteOffset::from(*range.start())..=AbsoluteOffset::from(*range.end()))
                            .into(),
                        MarkingKind::HoveredParsed,
                    ));
                }
            }
        }
        HoverInfo::Error { id } => {
            for range in result.errors[id.raw_idx()].provenance.byte_ranges() {
                state.marked_locations.add(MarkedLocation::new(
                    (AbsoluteOffset::from(*range.start())..=AbsoluteOffset::from(*range.end()))
                        .into(),
                    MarkingKind::HoveredParseErr,
                ));
            }
        }
    }
}

/// Information about what is hovered.
#[derive(Debug, PartialEq, Eq)]
pub enum HoverInfo {
    /// Nothing is hovered.
    Nothing,
    /// A value is hovered.
    Value {
        /// The path to the hovered value.
        path: Path,
    },
    /// An error is hovered.
    Error {
        /// The ID of the hovered error.
        id: ParseErrId,
    },
}

/// Displays the given [`Value`] in the GUI.
///
/// The return value is the path of the hovered value.
fn show_value(
    ui: &mut Ui,
    state: &mut State,
    path: Path,
    name: Option<&Symbol>,
    value: &Value,
    errors: &[ParseErr],
) -> HoverInfo {
    let name_prefix = if let Some(name) = name {
        format!("{name:?}: ")
    } else {
        String::new()
    };

    let mut this_hovered = false;
    let mut this_clicked = false;

    let mut handle_response = |response: Response| {
        if response.clicked() {
            this_clicked = true;
        } else if response.hovered() {
            this_hovered = true;
        }
    };

    let mut child_hovered = HoverInfo::Nothing;
    let mut hovered_err = None;

    match &value.kind {
        ValueKind::Boolean(_) | ValueKind::Integer(_) | ValueKind::Float(_) => {
            handle_response(ui.label(format!("{name_prefix}{:?},", value.kind)));
        }
        ValueKind::Bytes(bytes) => {
            ui.horizontal(|ui| {
                let old_spacing = ui.spacing_mut().item_spacing;
                ui.spacing_mut().item_spacing.x = 0.0;

                let font_size = TextStyle::Body.resolve(ui.style()).size;
                let hex_font = FontId::monospace(font_size);

                handle_response(ui.label(format!("{name_prefix}<")));
                match bytes.preview_slice() {
                    Ok(slice) => {
                        for (i, byte) in slice.iter().enumerate() {
                            handle_response(
                                ui.label(
                                    RichText::new(format!("{byte:02x}"))
                                        .font(hex_font.clone())
                                        .color(state.settings.byte_color(*byte)),
                                ),
                            );
                            if i != bytes.len() - 1 {
                                handle_response(
                                    ui.label(RichText::new(" ").font(hex_font.clone())),
                                );
                            }
                        }
                    }
                    Err((prefix, suffix)) => {
                        for byte in prefix {
                            handle_response(
                                ui.label(
                                    RichText::new(format!("{byte:02x} "))
                                        .font(hex_font.clone())
                                        .color(state.settings.byte_color(*byte)),
                                ),
                            );
                        }

                        handle_response(ui.label("..."));

                        for byte in suffix {
                            handle_response(
                                ui.label(
                                    RichText::new(format!(" {byte:02x}"))
                                        .font(hex_font.clone())
                                        .color(state.settings.byte_color(*byte)),
                                ),
                            );
                        }
                    }
                }
                handle_response(ui.label(">,"));

                ui.spacing_mut().item_spacing = old_spacing;
            });
        }
        ValueKind::Struct { fields, error } => {
            ui.vertical(|ui| {
                handle_response(ui.label(format!("{name_prefix}{{")));

                let mut child_rect = ui.cursor().intersect(ui.max_rect());
                child_rect.min.x += state.settings.font_size();
                ui.scope_builder(
                    egui::UiBuilder::new()
                        .max_rect(child_rect)
                        .layout(egui::Layout::top_down(egui::Align::Min)),
                    |ui| {
                        for (name, value) in fields {
                            let mut path = path.clone();
                            path.push(PathComponent::FieldAccess(name.clone()));

                            let hovered = show_value(ui, state, path, Some(name), value, errors);
                            if hovered != HoverInfo::Nothing {
                                child_hovered = hovered;
                            }
                        }
                        hovered_err =
                            hovered_err.or(render_error_and_return_hovered(ui, error, errors));
                    },
                );

                handle_response(ui.label("},"));
            });
        }
        ValueKind::Array { items, error } => {
            ui.vertical(|ui| {
                handle_response(ui.label(format!("{name_prefix}[")));

                let mut child_rect = ui.cursor().intersect(ui.max_rect());
                child_rect.min.x += state.settings.font_size();
                ui.scope_builder(
                    UiBuilder::new()
                        .max_rect(child_rect)
                        .layout(Layout::top_down(egui::Align::Min)),
                    |ui| {
                        for (i, value) in items.iter().enumerate() {
                            let mut path = path.clone();
                            path.push(PathComponent::Indexing(i));

                            let hovered = show_value(ui, state, path, None, value, errors);
                            if hovered != HoverInfo::Nothing {
                                child_hovered = hovered;
                            }
                        }
                        hovered_err =
                            hovered_err.or(render_error_and_return_hovered(ui, error, errors));
                    },
                );

                handle_response(ui.label("],"));
            });
        }
    }

    if this_clicked && let Some(byte_range) = value.provenance.byte_ranges().next() {
        state
            .scroll_state
            .rearrange_bars_for_point(0, AbsoluteOffset::from(*byte_range.start()));
    }

    if child_hovered != HoverInfo::Nothing {
        child_hovered
    } else if this_hovered {
        HoverInfo::Value { path: path.clone() }
    } else if let Some(err) = hovered_err {
        HoverInfo::Error { id: err }
    } else {
        HoverInfo::Nothing
    }
}

/// Renders the given error to the UI if it is present.
///
/// Returns the hovered error if it is hovered.
fn render_error_and_return_hovered(
    ui: &mut Ui,
    error: &Option<ParseErrId>,
    errors: &[ParseErr],
) -> Option<ParseErrId> {
    if let Some(err_id) = error {
        let err = &errors[err_id.raw_idx()];

        // TODO: use the error span to highlight it in a possible future editor

        if ui
            .label(
                RichText::new(format!("... parsing error: {},", err.message))
                    .color(ui.visuals().error_fg_color),
            )
            .hovered()
        {
            Some(*err_id)
        } else {
            None
        }
    } else {
        None
    }
}
