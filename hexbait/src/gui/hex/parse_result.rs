//! Implements display logic for parsing results.

use egui::{FontId, RichText, Sense, TextStyle};
use hexbait_lang::{
    Value, ValueKind,
    ir::{
        Symbol,
        path::{Path, PathComponent},
    },
};

use crate::{gui::hex::render_hex, state::Settings};

/// Displays the given [`Value`] in the GUI.
///
/// The return value is the path of the hovered value.
pub fn show_value(
    ui: &mut egui::Ui,
    path: Path,
    name: Option<&Symbol>,
    value: &Value,
    settings: &Settings,
) -> Option<Path> {
    let name_prefix = if let Some(name) = name {
        format!("{name:?}: ")
    } else {
        String::new()
    };

    let mut this_hovered = false;
    let mut child_hovered = None;

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

                let space = settings.small_space() * 0.6;

                this_hovered |= ui.label(format!("{name_prefix}<")).hovered();
                if bytes.len() > 16 {
                    for byte in &bytes[..8] {
                        this_hovered |=
                            render_hex(ui, settings, Sense::hover(), *byte, hex_font.clone())
                                .hovered();
                        ui.add_space(space);
                    }

                    this_hovered |= ui.label("...").hovered();

                    for byte in &bytes[bytes.len() - 8..] {
                        ui.add_space(space);
                        this_hovered |=
                            render_hex(ui, settings, Sense::hover(), *byte, hex_font.clone())
                                .hovered();
                    }
                } else {
                    for (i, byte) in bytes.iter().enumerate() {
                        this_hovered |=
                            render_hex(ui, settings, Sense::hover(), *byte, hex_font.clone())
                                .hovered();
                        if i != bytes.len() - 1 {
                            ui.add_space(space);
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
                ui.allocate_new_ui(
                    egui::UiBuilder::new()
                        .max_rect(child_rect)
                        .layout(egui::Layout::top_down(egui::Align::Min)),
                    |ui| {
                        for (name, value) in fields {
                            let mut path = path.clone();
                            path.push(PathComponent::FieldAccess(name.clone()));

                            let hovered = show_value(ui, path, Some(name), value, settings);
                            if hovered.is_some() {
                                child_hovered = hovered;
                            }
                        }
                        if let Some(err) = error {
                            // TODO: highlight the error when this is hovered
                            ui.label(
                                RichText::new(format!("... parsing error {err:?},"))
                                    .color(ui.visuals().error_fg_color),
                            )
                            .hovered();
                        }
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
                ui.allocate_new_ui(
                    egui::UiBuilder::new()
                        .max_rect(child_rect)
                        .layout(egui::Layout::top_down(egui::Align::Min)),
                    |ui| {
                        for (i, value) in items.iter().enumerate() {
                            let mut path = path.clone();
                            path.push(PathComponent::Indexing(i));

                            let hovered = show_value(ui, path, None, value, settings);
                            if hovered.is_some() {
                                child_hovered = hovered;
                            }
                        }
                        if let Some(err) = error {
                            // TODO: highlight the error when this is hovered
                            ui.label(
                                RichText::new(format!("... parsing error {err:?},"))
                                    .color(ui.visuals().error_fg_color),
                            )
                            .hovered();
                        }
                    },
                );

                let hovered = ui.label("],").hovered();

                this_hovered |= hovered;
            });
        }
    }

    if child_hovered.is_some() {
        child_hovered
    } else if this_hovered {
        Some(path.clone())
    } else {
        None
    }
}
