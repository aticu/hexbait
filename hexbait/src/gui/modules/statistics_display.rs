//! Implements display of statistics of the input data.

use egui::{Align2, Color32, FontId, PopupAnchor, Rect, Sense, Tooltip, Ui, Vec2, vec2};
use hexbait_common::Input;

use crate::{
    IDLE_TIME,
    gui::primitives::{render_glyph, render_hex},
    state::{Settings, State, StatisticsDisplayState},
    statistics::Statistics,
    window::Window,
};

/// Shows the statistics display module.
pub fn show(ui: &mut Ui, state: &mut State, _: &Input) {
    let window = state.scroll_state.selected_window();
    let (statistics, quality) = state
        .statistics_handler
        .get_bigram(window)
        .into_result_with_quality()
        .unwrap()
        .unwrap_or_else(|| (Statistics::empty_for_window(window), 0.0));
    let rect = ui.max_rect().intersect(ui.cursor());

    render(
        &mut state.statistics_display_state,
        ui,
        rect,
        window,
        &statistics,
        quality,
        &state.settings,
    );
}

/// Converts the given statistics to a grid that can be displayed.
fn statistics_to_grid(statistics: &Statistics) -> Box<[[u8; 256]; 256]> {
    let mut grid = Box::new([[0; 256]; 256]);

    // first calculate some statistics
    let mut nonzero_count = 0;
    let mut sum = 0;
    let mut max = 0;

    for (_, _, count) in statistics.iter_non_zero() {
        if count > max {
            max = count;
        }
        nonzero_count += 1;
        sum += count;
    }

    // the mean scaled as a value between 0 and 1
    let mean = sum as f64 / nonzero_count as f64 / max as f64;

    // compute gamma such that the mean will get a middle color
    let gamma = 0.5f64.log2() / mean.log2();
    let gamma = if gamma.is_normal() { gamma } else { 1.0 };

    for first in 0..=255 {
        for second in 0..=255 {
            // scale the number as a value between 0 and 1
            let num = statistics.follow(first, second) as f64 / max as f64;

            // apply gamma correction
            let scaled_num = num.powf(gamma);

            // save the output
            grid[first as usize][second as usize] = (scaled_num * 255.0).round() as u8;
        }
    }

    grid
}

/// Renders the statistics into the given rect.
fn render(
    statistics_display_state: &mut StatisticsDisplayState,
    ui: &mut Ui,
    rect: Rect,
    window: Window,
    statistics: &Statistics,
    quality: f32,
    settings: &Settings,
) {
    let grid = statistics_to_grid(statistics);

    let side_len_x = (rect.width().trunc() / 256.0).trunc();
    let side_len_y = (rect.height().trunc() / 256.0).trunc();
    let side_len = side_len_x.min(side_len_y);

    let rect = Rect::from_min_size(
        ui.cursor().left_top(),
        vec2(side_len * 256.0, side_len * 256.0),
    );

    statistics_display_state.cached_image.paint_at(
        ui,
        rect,
        (window, quality, settings.color_map()),
        |x, y| {
            let first = x / side_len as usize;
            let second = y / side_len as usize;

            let intensity = grid[first][second];

            settings.scale_color_u8(intensity)
        },
    );
    ui.advance_cursor_after_rect(rect);

    if quality < 1.0 {
        ui.ctx().request_repaint_after(IDLE_TIME);

        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            format!("Loading: {:6.2}%", quality * 100.0),
            FontId::proportional(settings.font_size()),
            Color32::WHITE,
        );
    }

    let hover_positions = ui.ctx().input(|input| {
        if let Some(pos) = input.pointer.latest_pos()
            && rect.contains(pos)
        {
            let first = ((pos - rect.min).x / side_len) as u8;
            let second = ((pos - rect.min).y / side_len) as u8;

            Some((first, second))
        } else {
            None
        }
    });

    if let Some((first, second)) = hover_positions {
        let intensity = grid[first as usize][second as usize];

        Tooltip::always_open(
            ui.ctx().clone(),
            ui.layer_id(),
            "statistics_display_tooltip".into(),
            PopupAnchor::Pointer,
        )
        .show(|ui| {
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    render_hex(ui, settings, Sense::hover(), first);
                    render_hex(ui, settings, Sense::hover(), second);

                    ui.spacing_mut().item_spacing = Vec2::ZERO;
                    ui.add_space(settings.large_space());

                    render_glyph(ui, settings, Sense::hover(), first);
                    render_glyph(ui, settings, Sense::hover(), second);
                });
                ui.label(format!(
                    "Relative Density: {:0.02}%",
                    intensity as f64 / 2.55,
                ));
            });
        });
    }
}
