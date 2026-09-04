//! Implements showing of a parsed value.

use egui::{FontId, Key, Layout, Response, RichText, ScrollArea, TextStyle, Ui, UiBuilder};
use hexbait_common::{AbsoluteOffset, Input, RelativeOffset};
use hexbait_lang::{
    compile::{
        CompileResult, compile_file,
        ir::{
            Symbol,
            path::{Path, PathComponent},
        },
    },
    eval::{Diagnostic, DiagnosticId, DiagnosticLevel, StructContent, Value, ValueKind, View},
};

use crate::{
    gui::diagnostic_emitter::emit_diagnostics,
    marking::MarkType,
    state::{ParseType, ScrollState, Settings, State},
};

/// Shows the parsed value module.
pub fn show(ui: &mut Ui, state: &mut State, input: &Input) {
    ui.horizontal(|ui| {
        ui.label("Parse as:");
        egui::ComboBox::new("parse_type", "")
            .selected_text(state.parse_state.parse_type.as_str())
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut state.parse_state.parse_type,
                    ParseType::None,
                    ParseType::None.as_str(),
                );
                for path in &state.parse_state.custom_parsers {
                    let value = ParseType::Custom(path.clone());
                    ui.selectable_value(
                        &mut state.parse_state.parse_type,
                        value.clone(),
                        value.as_str(),
                    );
                }
                for description in state.parse_state.built_in_format_descriptions.keys() {
                    let value = ParseType::Builtin(description);
                    ui.selectable_value(
                        &mut state.parse_state.parse_type,
                        value.clone(),
                        value.as_str(),
                    );
                }
            });

        ui.label(RichText::new("⚠").color(ui.visuals().warn_fg_color))
            .on_hover_ui(|ui| {
                ui.label("Parsed value definitions are in beta and may not treat all cases correctly. Treat with care.");
            });
    });

    ui.horizontal(|ui| {
        ui.label("Parse offset:");
        if ui
            .text_edit_singleline(&mut state.parse_state.parse_offset)
            .lost_focus()
            && ui.input(|i| i.key_pressed(Key::Enter))
            && let Ok(offset) = state
                .parse_state
                .parse_offset
                .parse()
                .map(AbsoluteOffset::from)
        {
            state.scroll_state.rearrange_bars_for_point(0, offset);
        }

        if ui
            .add_enabled(
                state.parse_state.parse_offset.parse::<u64>().is_ok(),
                egui::Button::new("Jump to offset"),
            )
            .clicked()
            && let Ok(offset) = state
                .parse_state
                .parse_offset
                .parse()
                .map(AbsoluteOffset::from)
        {
            state.scroll_state.rearrange_bars_for_point(0, offset);
        }
    });

    ui.checkbox(
        &mut state.parse_state.sync_parse_offset_to_selection_start,
        "Sync parse offset to selection start",
    );

    state
        .marked_locations
        .clear_marks_of_type(MarkType::HoveredParsed);
    state
        .marked_locations
        .clear_marks_of_type(MarkType::HoveredParseErr);

    let ir;
    let parse_type = 'parse_type: {
        match &state.parse_state.parse_type {
            ParseType::None => None,
            ParseType::Builtin(builtin) => {
                state.parse_state.built_in_format_descriptions.get(builtin)
            }
            ParseType::Custom(path) => {
                let source_name = path.display().to_string();
                let Ok(content) = std::fs::read_to_string(path) else {
                    break 'parse_type None;
                };
                match compile_file(&source_name, &content) {
                    CompileResult::NoDiagnostics { ir: compiled_ir } => {
                        ir = compiled_ir;
                    }
                    CompileResult::WithWarnings {
                        ir: compiled_ir,
                        diagnostics,
                    } => {
                        ir = compiled_ir;
                        emit_diagnostics(ui, &diagnostics);
                    }
                    CompileResult::Failure { diagnostics } => {
                        emit_diagnostics(ui, &diagnostics);
                        break 'parse_type None;
                    }
                }

                Some(&ir)
            }
        }
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
    let view = view.subview(parse_offset.to_relative()..view.len().as_relative_offset());
    let result = hexbait_lang::eval::eval_ir(parse_type, view, RelativeOffset::ZERO);

    let hovered = ScrollArea::vertical()
        .auto_shrink([false, true])
        .show(ui, |ui| {
            show_value(
                ui,
                &state.settings,
                &mut state.scroll_state,
                Path::new(),
                None,
                &result.value,
                &result.diagnostics,
            )
        })
        .inner;

    match hovered {
        HoverInfo::Nothing => (),
        HoverInfo::Value { path } => {
            if let Some(value) = result.value.subvalue_at_path(&path) {
                for range in value.provenance.byte_ranges() {
                    state.marked_locations.add(
                        (AbsoluteOffset::from(*range.start())..=AbsoluteOffset::from(*range.end()))
                            .into(),
                        MarkType::HoveredParsed,
                    );
                }
            }
        }
        HoverInfo::Error { id } => {
            for range in result.diagnostics[id.raw_idx()].provenance.byte_ranges() {
                state.marked_locations.add(
                    (AbsoluteOffset::from(*range.start())..=AbsoluteOffset::from(*range.end()))
                        .into(),
                    MarkType::HoveredParseErr,
                );
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
        id: DiagnosticId,
    },
}

/// Displays the given [`Value`] in the GUI.
///
/// The return value is the path of the hovered value.
pub fn show_value(
    ui: &mut Ui,
    settings: &Settings,
    scroll_state: &mut ScrollState,
    path: Path,
    name: Option<&Symbol>,
    value: &Value,
    diagnostics: &[Diagnostic],
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
    let mut hovered_diagnostic = None;

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
                let mut preview_buf = [0; _];
                match bytes.preview_slice(&mut preview_buf) {
                    Some(len) => {
                        let slice = &preview_buf[..len];

                        for (i, byte) in slice.iter().enumerate() {
                            handle_response(
                                ui.label(
                                    RichText::new(format!("{byte:02x}"))
                                        .font(hex_font.clone())
                                        .color(settings.byte_color(*byte)),
                                ),
                            );
                            if i != bytes.len() - 1 {
                                handle_response(
                                    ui.label(RichText::new(" ").font(hex_font.clone())),
                                );
                            }
                        }
                    }
                    None => {
                        let (prefix, suffix) = preview_buf.split_at(preview_buf.len() / 2);

                        for byte in prefix {
                            handle_response(
                                ui.label(
                                    RichText::new(format!("{byte:02x} "))
                                        .font(hex_font.clone())
                                        .color(settings.byte_color(*byte)),
                                ),
                            );
                        }

                        handle_response(ui.label("..."));

                        for byte in suffix {
                            handle_response(
                                ui.label(
                                    RichText::new(format!(" {byte:02x}"))
                                        .font(hex_font.clone())
                                        .color(settings.byte_color(*byte)),
                                ),
                            );
                        }
                    }
                }
                handle_response(ui.label(">,"));

                ui.spacing_mut().item_spacing = old_spacing;
            });
        }
        ValueKind::Struct { content } => {
            ui.vertical(|ui| {
                handle_response(ui.label(format!("{name_prefix}{{")));

                let mut child_rect = ui.cursor().intersect(ui.max_rect());
                child_rect.min.x += settings.font_size();
                ui.scope_builder(
                    egui::UiBuilder::new()
                        .max_rect(child_rect)
                        .layout(egui::Layout::top_down(egui::Align::Min)),
                    |ui| {
                        for content in content {
                            match content {
                                StructContent::Field { name, value } => {
                                    let mut path = path.clone();
                                    path.push(PathComponent::FieldAccess(name.clone()));

                                    let hovered = show_value(
                                        ui,
                                        settings,
                                        scroll_state,
                                        path,
                                        Some(name),
                                        value,
                                        diagnostics,
                                    );
                                    if hovered != HoverInfo::Nothing {
                                        child_hovered = hovered;
                                    }
                                }
                                StructContent::Diagnostic(diagnostic_id) => {
                                    hovered_diagnostic = hovered_diagnostic.or(
                                        render_diagnostic_and_return_hovered(
                                            ui,
                                            *diagnostic_id,
                                            diagnostics,
                                        ),
                                    );
                                }
                            }
                        }
                    },
                );

                handle_response(ui.label("},"));
            });
        }
        ValueKind::Array { items, error } => {
            ui.vertical(|ui| {
                handle_response(ui.label(format!("{name_prefix}[")));

                let mut child_rect = ui.cursor().intersect(ui.max_rect());
                child_rect.min.x += settings.font_size();
                ui.scope_builder(
                    UiBuilder::new()
                        .max_rect(child_rect)
                        .layout(Layout::top_down(egui::Align::Min)),
                    |ui| {
                        for (i, value) in items.iter().enumerate() {
                            let mut path = path.clone();
                            path.push(PathComponent::Indexing(i));

                            let hovered = show_value(
                                ui,
                                settings,
                                scroll_state,
                                path,
                                None,
                                value,
                                diagnostics,
                            );
                            if hovered != HoverInfo::Nothing {
                                child_hovered = hovered;
                            }
                        }
                        if let Some(diagnostic) = error {
                            hovered_diagnostic = hovered_diagnostic.or(
                                render_diagnostic_and_return_hovered(ui, *diagnostic, diagnostics),
                            );
                        }
                    },
                );

                handle_response(ui.label("],"));
            });
        }
    }

    if this_clicked && let Some(byte_range) = value.provenance.byte_ranges().next() {
        scroll_state.rearrange_bars_for_point(0, AbsoluteOffset::from(*byte_range.start()));
    }

    if child_hovered != HoverInfo::Nothing {
        child_hovered
    } else if this_hovered {
        HoverInfo::Value { path: path.clone() }
    } else if let Some(err) = hovered_diagnostic {
        HoverInfo::Error { id: err }
    } else {
        HoverInfo::Nothing
    }
}

/// Renders the given error to the UI if it is present.
///
/// Returns the hovered error if it is hovered.
fn render_diagnostic_and_return_hovered(
    ui: &mut Ui,
    diagnostic_id: DiagnosticId,
    diagnostics: &[Diagnostic],
) -> Option<DiagnosticId> {
    let diagnostic = &diagnostics[diagnostic_id.raw_idx()];

    // TODO: use the error span to highlight it in a possible future editor

    let (ty_name, color) = match diagnostic.level {
        DiagnosticLevel::Fail => ("failure", ui.visuals().error_fg_color),
        DiagnosticLevel::Warn => ("warning", ui.visuals().warn_fg_color),
    };
    if ui
        .label(RichText::new(format!("parsing {ty_name}: {},", diagnostic.message)).color(color))
        .hovered()
    {
        Some(diagnostic_id)
    } else {
        None
    }
}
