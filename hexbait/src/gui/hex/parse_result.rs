//! Implements display logic for parsing results.

use egui::{FontId, RichText, TextStyle, Ui};
use hexbait_lang::{
    ParseErr, ParseErrId, Value, ValueKind,
    ir::{
        Symbol,
        path::{Path, PathComponent},
    },
};

use crate::state::Settings;

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
pub fn show_value(
    ui: &mut Ui,
    path: Path,
    name: Option<&Symbol>,
    value: &Value,
    errors: &[ParseErr],
    settings: &Settings,
) -> HoverInfo {
    let name_prefix = if let Some(name) = name {
        format!("{name:?}: ")
    } else {
        String::new()
    };

    let mut this_hovered = false;
    let mut child_hovered = HoverInfo::Nothing;
    let mut hovered_err = None;

    match &value.kind {
        ValueKind::Boolean(_) | ValueKind::Integer(_) | ValueKind::Float(_) => {
            let hovered = ui
                .label(format!("{name_prefix}{:?},", value.kind))
                .hovered();

            this_hovered |= hovered;
        }
        ValueKind::Bytes(bytes) => {
            ui.horizontal(|ui| {
                let old_spacing = ui.spacing_mut().item_spacing;
                ui.spacing_mut().item_spacing.x = 0.0;

                let font_size = TextStyle::Body.resolve(ui.style()).size;
                let hex_font = FontId::monospace(font_size);

                this_hovered |= ui.label(format!("{name_prefix}<")).hovered();
                if bytes.len() > 16 {
                    for byte in &bytes[..8] {
                        this_hovered |= ui
                            .label(
                                RichText::new(format!("{byte:02x} "))
                                    .font(hex_font.clone())
                                    .color(settings.byte_color(*byte)),
                            )
                            .hovered();
                    }

                    this_hovered |= ui.label("...").hovered();

                    for byte in &bytes[bytes.len() - 8..] {
                        this_hovered |= ui
                            .label(
                                RichText::new(format!(" {byte:02x}"))
                                    .font(hex_font.clone())
                                    .color(settings.byte_color(*byte)),
                            )
                            .hovered();
                    }
                } else {
                    for (i, byte) in bytes.iter().enumerate() {
                        this_hovered |= ui
                            .label(
                                RichText::new(format!("{byte:02x}"))
                                    .font(hex_font.clone())
                                    .color(settings.byte_color(*byte)),
                            )
                            .hovered();
                        if i != bytes.len() - 1 {
                            this_hovered |= ui
                                .label(RichText::new(" ").font(hex_font.clone()))
                                .hovered();
                        }
                    }
                }
                this_hovered |= ui.label(">,").hovered();

                ui.spacing_mut().item_spacing = old_spacing;
            });
        }
        ValueKind::Struct { fields, error } => {
            ui.vertical(|ui| {
                let hovered = ui.label(format!("{name_prefix}{{")).hovered();

                this_hovered |= hovered;

                let mut child_rect = ui.cursor().intersect(ui.max_rect());
                child_rect.min.x += settings.font_size();
                ui.scope_builder(
                    egui::UiBuilder::new()
                        .max_rect(child_rect)
                        .layout(egui::Layout::top_down(egui::Align::Min)),
                    |ui| {
                        for (name, value) in fields {
                            let mut path = path.clone();
                            path.push(PathComponent::FieldAccess(name.clone()));

                            let hovered = show_value(ui, path, Some(name), value, errors, settings);
                            if hovered != HoverInfo::Nothing {
                                child_hovered = hovered;
                            }
                        }
                        hovered_err =
                            hovered_err.or(render_error_and_return_hovered(ui, error, errors));
                    },
                );

                let hovered = ui.label("},").hovered();

                this_hovered |= hovered;
            });
        }
        ValueKind::Array { items, error } => {
            ui.vertical(|ui| {
                let hovered = ui.label(format!("{name_prefix}[")).hovered();

                this_hovered |= hovered;

                let mut child_rect = ui.cursor().intersect(ui.max_rect());
                child_rect.min.x += settings.font_size();
                ui.scope_builder(
                    egui::UiBuilder::new()
                        .max_rect(child_rect)
                        .layout(egui::Layout::top_down(egui::Align::Min)),
                    |ui| {
                        for (i, value) in items.iter().enumerate() {
                            let mut path = path.clone();
                            path.push(PathComponent::Indexing(i));

                            let hovered = show_value(ui, path, None, value, errors, settings);
                            if hovered != HoverInfo::Nothing {
                                child_hovered = hovered;
                            }
                        }
                        hovered_err =
                            hovered_err.or(render_error_and_return_hovered(ui, error, errors));
                    },
                );

                let hovered = ui.label("],").hovered();

                this_hovered |= hovered;
            });
        }
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
