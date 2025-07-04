//! Implements display logic for parsing results.

use crate::{
    gui::settings::Settings,
    parsing::{
        eval::{Path, PathComponent, Value, ValueKind},
        language::ast::Symbol,
    },
};

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
        ValueKind::Integer(_) | ValueKind::Float(_) | ValueKind::Bytes(_) => {
            let hovered = ui
                .label(
                    egui::RichText::new(format!("{name_prefix}{:?},", value.kind))
                        .size(settings.font_size()),
                )
                .hovered();

            this_hovered |= hovered;
        }
        ValueKind::Struct(children) => {
            ui.vertical(|ui| {
                let hovered = ui
                    .label(
                        egui::RichText::new(format!("{name_prefix}{{")).size(settings.font_size()),
                    )
                    .hovered();

                this_hovered |= hovered;

                let mut child_rect = ui.cursor().intersect(ui.max_rect());
                child_rect.min.x += settings.font_size();
                ui.allocate_new_ui(
                    egui::UiBuilder::new()
                        .max_rect(child_rect)
                        .layout(egui::Layout::top_down(egui::Align::Min)),
                    |ui| {
                        for (name, value) in children {
                            let mut path = path.clone();
                            path.push(PathComponent::FieldAccess(name.clone()));

                            let hovered = show_value(ui, path, Some(name), value, settings);
                            if hovered.is_some() {
                                child_hovered = hovered;
                            }
                        }
                    },
                );

                let hovered = ui
                    .label(egui::RichText::new("},").size(settings.font_size()))
                    .hovered();

                this_hovered |= hovered;
            });
        }
        ValueKind::Array(children) => {
            ui.vertical(|ui| {
                let hovered = ui
                    .label(
                        egui::RichText::new(format!("{name_prefix}[")).size(settings.font_size()),
                    )
                    .hovered();

                this_hovered |= hovered;

                let mut child_rect = ui.cursor().intersect(ui.max_rect());
                child_rect.min.x += settings.font_size();
                ui.allocate_new_ui(
                    egui::UiBuilder::new()
                        .max_rect(child_rect)
                        .layout(egui::Layout::top_down(egui::Align::Min)),
                    |ui| {
                        for (i, value) in children.iter().enumerate() {
                            let mut path = path.clone();
                            path.push(PathComponent::Indexing(i));

                            let hovered = show_value(ui, path, None, value, settings);
                            if hovered.is_some() {
                                child_hovered = hovered;
                            }
                        }
                    },
                );

                let hovered = ui
                    .label(egui::RichText::new("],").size(settings.font_size()))
                    .hovered();

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
