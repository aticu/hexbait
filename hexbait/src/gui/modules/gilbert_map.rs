//! Implements the byte-view gilbert module.

use egui::{Color32, Sense, Ui};
use hexbait_common::{Input, Len, RelativeOffset};

use crate::{
    gui::{color, gilbert_curve::GilbertCurve, image_processing::blur_image},
    state::{Scrollbar, State},
    statistics::MetricsQuality,
};

/// How many bytes to show per pixel when there is enough data.
const OVERSAMPLE: u64 = 4;

/// Shows the gilbert curve in the GUI.
pub fn show(ui: &mut Ui, state: &mut State, input: &Input) {
    let rect = ui.max_rect().intersect(ui.cursor());
    if rect.area() <= 0.0 {
        return;
    }
    let response = ui.allocate_rect(rect, Sense::click());

    let width = rect.width().ceil() as u32;
    let height = rect.height().ceil() as u32;
    let pixel_budget = (width * height) as u64;
    state.scroll_state.gilbert_pixel_budget = pixel_budget;

    let selected_window = state.scroll_state.selected_window();
    let len = Len::from(pixel_budget * OVERSAMPLE).min(selected_window.size());
    let show_byte_colors = selected_window.size().as_u64() <= pixel_budget * OVERSAMPLE;

    let min_hover_selection_size = state.scroll_state.total_hexdump_bytes().as_u64() as f32
        / selected_window.size().as_u64() as f32;

    let gilbert_curve = state
        .scroll_state
        .gilbert_curve
        .get((width, height), |_| GilbertCurve::compute(width, height));

    if response.hovered()
        && let Some(pos) = ui.input(|input| input.pointer.hover_pos())
        && rect.contains(pos)
    {
        let pos = pos - rect.left_top();
        state.scroll_state.gilbert_hover_position = Some(
            gilbert_curve.index_from_point(pos.x as usize, pos.y as usize) as f32
                / pixel_budget as f32,
        );
    } else {
        state.scroll_state.gilbert_hover_position = None;
    }

    if response.hovered() {
        let (scroll_factor, shift_pressed) = ui.input(|input| {
            (
                -(input.smooth_scroll_delta.x + input.smooth_scroll_delta.y),
                input.modifiers.shift,
            )
        });
        let scroll_speed = 1.01f32;
        let scroll_factor = if shift_pressed {
            scroll_factor * 0.3
        } else {
            scroll_factor
        };
        state.scroll_state.hover_selection_size = (state.scroll_state.hover_selection_size
            * scroll_speed.powf(scroll_factor))
        .clamp(0.001, 0.35)
        .max(min_hover_selection_size);
    }

    let mut full_quality = true;
    if show_byte_colors {
        state.scroll_state.gilbert_map_cached_image.paint_at(
            ui,
            rect,
            (selected_window, state.settings.linear_byte_colors()),
            || input.read_at(selected_window.start(), len, None),
            |data, x, y| {
                let Ok(data) = data else {
                    return Color32::TRANSPARENT;
                };
                if data.is_empty() {
                    return Color32::TRANSPARENT;
                }

                let idx = gilbert_curve.index_from_point(x, y) as u64;
                let data_len = data.len() as u64;

                let byte_start = ((idx * data_len) / pixel_budget) as usize;
                let byte_end = (((idx + 1) * data_len) / pixel_budget) as usize;

                if byte_end <= byte_start + 1 {
                    state.settings.byte_color(data[byte_start])
                } else {
                    let (mut r, mut g, mut b) = (0, 0, 0);
                    for &byte in &data[byte_start..byte_end] {
                        let color = state.settings.byte_color(byte);
                        r += color.r() as usize;
                        g += color.g() as usize;
                        b += color.b() as usize;
                    }
                    let n = byte_end - byte_start;

                    Color32::from_rgb((r / n) as u8, (g / n) as u8, (b / n) as u8)
                }
            },
        );
    } else {
        state.scroll_state.gilbert_map_cached_image.paint_at(
            ui,
            rect,
            (selected_window, state.settings.linear_byte_colors()),
            || {
                state
                    .statistics_handler
                    .get_map_metrics_access(selected_window, pixel_budget as usize)
            },
            |stats, x, y| {
                let idx = gilbert_curve.index_from_point(x, y);

                let (metrics, quality) = stats
                    .as_ref()
                    .map(|stats| stats.get_metrics(idx))
                    .unwrap_or((None, MetricsQuality::Estimated));

                if quality.is_estimated() || metrics.is_none() {
                    full_quality = false;
                }

                color::metrics_color(metrics, quality, &state.settings)
            },
        );
    }

    if !full_quality {
        state
            .scroll_state
            .gilbert_map_cached_image
            .require_repaint();
        state.scroll_state.gilbert_map_blurred_image.invalidate();
        state
            .scroll_state
            .gilbert_map_hover_cached_image
            .require_repaint();
    }

    let hover_position = state.scroll_state.gilbert_hover_position.map(|hover_pos| {
        hover_pos.clamp(
            state.scroll_state.hover_selection_size / 2.0,
            1.0 - (state.scroll_state.hover_selection_size / 2.0),
        )
    });
    let first_selection_index = hover_position.map(|pos| {
        ((pos - state.scroll_state.hover_selection_size / 2.0).clamp(0.0, 1.0)
            * pixel_budget as f32) as usize
    });
    let selection_size = (state.scroll_state.hover_selection_size * pixel_budget as f32) as usize;

    state.scroll_state.gilbert_map_hover_cached_image.paint_at(
        ui,
        rect,
        (
            state.scroll_state.hover_selection_size,
            hover_position,
            full_quality,
        ),
        || {
            let image = state.scroll_state.gilbert_map_blurred_image.get(
                (
                    rect,
                    selected_window,
                    state.settings.linear_byte_colors(),
                    full_quality,
                ),
                |_| {
                    blur_image(
                        state.scroll_state.gilbert_map_cached_image.raw(),
                        &state.settings,
                    )
                },
            );

            let [w, h] = image.size;

            let mut sdf = vec![u8::MAX; w * h];
            let selection_border_size = state.settings.selection_border_size();
            let k = selection_border_size * 2 + 1;

            // Build kernel.
            let r = selection_border_size as i32;
            let mut kernel = vec![u8::MAX; k * k];
            for ky in 0..k {
                for kx in 0..k {
                    let dx = kx as i32 - r;
                    let dy = ky as i32 - r;
                    let dist = ((dx * dx + dy * dy) as f32).sqrt().round() as i32;
                    if dist <= r {
                        kernel[kx + ky * k] = dist as u8;
                    }
                }
            }

            let Some(start_index) = first_selection_index else {
                sdf.fill(0); // no selection → treat everything as in-selection (matches old semantics)
                return (image, sdf);
            };

            // Pass 1: zero out all selected pixels, remember their coords.
            let mut selected_coords = Vec::with_capacity(selection_size);
            for idx in start_index..start_index + selection_size {
                let p = gilbert_curve.point_from_index(idx);
                sdf[p.x as usize + p.y as usize * w] = 0;
                selected_coords.push((p.x as usize, p.y as usize));
            }

            // Pass 2: a selected pixel is on the inside boundary if any 4-neighbor
            // is non-zero (i.e., not in the selection). Image-edge neighbors
            // don't count — halos don't extend past the image.
            // write the distances to the surrounding pixels
            for &(x, y) in &selected_coords {
                let is_boundary = ((x > 0) && (sdf[(x - 1) + y * w] != 0))
                    || ((x + 1 < w) && (sdf[(x + 1) + y * w] != 0))
                    || ((y > 0) && (sdf[x + (y - 1) * w] != 0))
                    || ((y + 1 < h) && (sdf[x + (y + 1) * w] != 0));
                if !is_boundary {
                    continue;
                }

                let x_start = x.saturating_sub(selection_border_size);
                let y_start = y.saturating_sub(selection_border_size);
                let x_end = (x + selection_border_size + 1).min(w);
                let y_end = (y + selection_border_size + 1).min(h);
                let kx_start = x_start + selection_border_size - x;
                let ky_start = y_start + selection_border_size - y;

                for (row, ky) in (y_start..y_end).zip(ky_start..) {
                    let sdf_row = row * w;
                    let kernel_row = ky * k;
                    for (col, kx) in (x_start..x_end).zip(kx_start..) {
                        let kv = kernel[kx + kernel_row];
                        let sv = &mut sdf[col + sdf_row];
                        if kv < *sv {
                            *sv = kv;
                        }
                    }
                }
            }

            (image, sdf)
        },
        |(blurred_image, sdf), x, y| {
            let dist = sdf[x + y * blurred_image.size[0]] as usize;

            let selection_border_size = state.settings.selection_border_size();
            if dist == 0 {
                Color32::TRANSPARENT
            } else if dist > selection_border_size {
                blurred_image[(x, y)]
            } else {
                let rel_dist = (selection_border_size - dist) as f32 / selection_border_size as f32;
                let border_strength = rel_dist.powi(2);

                color::lerp(
                    blurred_image[(x, y)],
                    state.settings.scrollbar_selection_border_color(),
                    border_strength as f64,
                )
            }
        },
    );

    if response.clicked()
        && let Some(hover_position) = hover_position
    {
        let last_scrollbar = state.scroll_state.scrollbars.last_mut().unwrap();
        let len = last_scrollbar.selection_len().as_u64() as f64;

        let relative_start =
            (hover_position - state.scroll_state.hover_selection_size / 2.0).clamp(0.0, 1.0);
        let start = RelativeOffset::from((relative_start as f64 * len).round() as u64);
        let len = Len::from((state.scroll_state.hover_selection_size as f64 * len).round() as u64);

        last_scrollbar.set_selection(start, len);
        state.scroll_state.scrollbars.push(Scrollbar::new(len));
    }
}
