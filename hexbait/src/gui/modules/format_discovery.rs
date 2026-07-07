//! Renders a search screen in the GUI.

use egui::{Sense, Slider, Ui};
use hexbait_common::{Input, Len};

use crate::{gui::primitives::render_hex, marking::MarkType, state::State};

/// Shows the search screen in the GUI.
pub fn show(ui: &mut Ui, state: &mut State, input: &Input) {
    ui.vertical(|ui| {
        ui.add(
            Slider::new(state.format_discovery.len_mut(), 2..=1024)
                .text("Length")
                .logarithmic(true),
        );

        let ty = MarkType::UserMark {
            name: state.format_discovery.mark_name().to_string(),
        };
        if let Some(mark_iter) = state.marked_locations.iter_marks_of_type(&ty) {
            let len = state.format_discovery.len();
            let mut buf = Vec::with_capacity(len as usize);
            for mark in mark_iter {
                ui.horizontal(|ui| {
                    match input.read_at(mark.window.start(), Len::from(len), Some(&mut buf)) {
                        Ok(buf) => {
                            for &byte in &*buf {
                                render_hex(ui, &state.settings, Sense::hover(), byte);
                            }
                        }
                        Err(err) => {
                            ui.label(format!(
                                "could not read data at mark {:?}: {err}",
                                mark.window.start()
                            ));
                        }
                    }
                });
            }
        }
    });
}
