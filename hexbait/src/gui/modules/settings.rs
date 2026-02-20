//! Renders a settings screen in the GUI.

use egui::{ComboBox, RichText, Slider, Ui};
use hexbait_common::Input;

use crate::{
    gui::color::ColorMap,
    state::{State, ViewKind},
};

/// Shows the settings screen in the GUI.
pub fn show(ui: &mut Ui, state: &mut State, _: &Input) {
    ui.vertical(|ui| {
        ui.checkbox(
            state.settings.linear_byte_colors_mut(),
            "Use linear byte colors",
        );

        ui.checkbox(
            state.settings.fine_grained_scrollbars_mut(),
            "Use fine grained scrollbars",
        );

        ui.horizontal(|ui| {
            ui.label("Show in main content:");
            ComboBox::new("view_kind", "")
                .selected_text(state.settings.view_kind().display_str())
                .show_ui(ui, |ui| {
                    for kind in [
                        ViewKind::Auto,
                        ViewKind::ForceHexView,
                        ViewKind::ForceStatisticsView,
                    ] {
                        ui.selectable_value(state.settings.view_kind(), kind, kind.display_str());
                    }
                });
        });

        ui.horizontal(|ui| {
            ui.label("Color map:");
            ComboBox::new("color_map", "")
                .selected_text(format!("{:?}", state.settings.color_map()))
                .show_ui(ui, |ui| {
                    for kind in ColorMap::iter_all() {
                        ui.selectable_value(
                            state.settings.color_map_mut(),
                            kind,
                            format!("{kind:?}"),
                        );
                    }
                });

            if state.settings.color_map() != ColorMap::Viridis {
                ui.label(RichText::new("⚠").color(ui.visuals().warn_fg_color))
                    .on_hover_ui(|ui| {
                        ui.label("Color schemes other than Viridis are untested and may not work as well with other colors.");
                    });
            }
        });

        ui.add(Slider::new(state.settings.scale_mut(), 10.0..=50.0));
    });
}
