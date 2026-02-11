//! Implements the hexbait application.
//!
//! This is a hexadecimal viewer and analysis tool.

#![forbid(unsafe_code)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use std::path::PathBuf;

use clap::Parser;
use egui::{Align, Layout, Rect, RichText, ScrollArea, TextStyle, UiBuilder, vec2};
use hexbait::{
    state::{DisplayType, State, ViewKind},
    statistics::Statistics,
};
use hexbait_common::{AbsoluteOffset, Input};

// TODO: change font to render more characters
// TODO: implement to-disk caching for some statistic sizes to decrease re-load times
// TODO: re-use non-flat statistics for flat statistics
// TODO: fix up main file
// TODO: join polygons of adjoining marked locations
// TODO: improve hover text for marked locations
// TODO: implement more convenient escaping of byte arrays for search
// TODO: rearrange UI in a more useful way
// TODO: fix dragging across end during initial scrollbar selection
// TODO: add relative search (from here backwards/forwards)
// TODO: add screenshots to README
// TODO: add some user documentation
// TODO: make handling of usize <-> u64 conversions more consistent
// TODO: add more useful conversions between Len, AbsoluteOffset and RelativeOffset and use them
// where it makes sense

/// hexbait - Hierarchical EXploration Binary Analysis & Inspection Tool
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Config {
    /// The file to analyze
    file: Option<PathBuf>,
    /// A parser definition file to supply additional parsers
    #[arg(short, long)]
    parser_definition: Option<PathBuf>,
}

fn main() -> eframe::Result {
    let config = Config::parse();

    let input = if let Some(file_name) = &config.file {
        Input::from_path(file_name)
    } else {
        Input::from_stdin()
    }
    .expect("TODO: implement proper error handling in main");

    let file_name = if let Some(file) = &config.file {
        file.display().to_string()
    } else {
        String::from("stdin")
    };

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_maximized(true),
        ..Default::default()
    };
    eframe::run_native(
        &format!("hexbait: {file_name}"),
        options,
        Box::new(|_| {
            Ok(Box::new(MyApp {
                frame_time: std::time::Duration::ZERO,
                state: State::new(&input, config.parser_definition),
                input,
            }))
        }),
    )
}

struct MyApp {
    frame_time: std::time::Duration,
    state: State,
    input: Input,
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let start = std::time::Instant::now();
        egui::CentralPanel::default().show(ctx, |ui| {
            self.state.settings.apply_settings_to_ui(ui);

            if let Some(fps) = 1_000_000_000u128.checked_div(self.frame_time.as_nanos()) {
                ui.painter().text(
                    ui.max_rect().right_top(),
                    egui::Align2::RIGHT_TOP,
                    format!("{fps} FPS"),
                    TextStyle::Small.resolve(ui.style()),
                    ui.visuals().text_color(),
                );
            }

            let mut jump_to_offset = false;

            ui.horizontal(|ui| {
                ui.label("Parse as:");
                egui::ComboBox::new("parse_type", "")
                    .selected_text(self.state.parse_state.parse_type)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.state.parse_state.parse_type, "none", "none");
                        if self.state.parse_state.custom_parser.is_some() {
                            ui.selectable_value(
                                &mut self.state.parse_state.parse_type,
                                "custom parser",
                                "custom parser",
                            );
                        }
                        for description in
                            self.state.parse_state.built_in_format_descriptions.keys()
                        {
                            ui.selectable_value(
                                &mut self.state.parse_state.parse_type,
                                description,
                                *description,
                            );
                        }
                    });

                ui.label("Parse offset:");
                ui.text_edit_singleline(&mut self.state.parse_state.parse_offset);
                if ui
                    .add_enabled(
                        self.state.parse_state.parse_offset.parse::<u64>().is_ok(),
                        egui::Button::new("Jump to offset"),
                    )
                    .clicked()
                {
                    jump_to_offset = true;
                }
                ui.checkbox(
                    &mut self.state.parse_state.sync_parse_offset_to_selection_start,
                    "Sync parse offset to selection start",
                );

                ui.checkbox(
                    self.state.settings.linear_byte_colors_mut(),
                    "Use linear byte colors",
                );

                ui.checkbox(
                    self.state.settings.fine_grained_scrollbars_mut(),
                    "Use fine grained scrollbars",
                );

                ui.label("Show:");
                egui::ComboBox::new("view_kind", "")
                    .selected_text(self.state.settings.view_kind().display_str())
                    .show_ui(ui, |ui| {
                        for kind in [
                            ViewKind::Auto,
                            ViewKind::ForceHexView,
                            ViewKind::ForceStatisticsView,
                        ] {
                            ui.selectable_value(
                                self.state.settings.view_kind(),
                                kind,
                                kind.display_str(),
                            );
                        }
                    });
            });

            ui.scope_builder(
                UiBuilder::new()
                    .max_rect(ui.max_rect().intersect(ui.cursor()))
                    .layout(Layout::left_to_right(Align::Min)),
                |ui| {
                    hexbait::gui::scrollbars::render(
                        ui,
                        &mut self.state.scroll_state,
                        &self.state.settings,
                        &mut self.state.marked_locations,
                        &self.state.statistics_handler,
                    );

                    let display_type = match self.state.settings.view_kind() {
                        ViewKind::Auto => self.state.scroll_state.display_suggestion,
                        ViewKind::ForceHexView => DisplayType::Hexview,
                        ViewKind::ForceStatisticsView => DisplayType::Statistics,
                    };

                    match display_type {
                        DisplayType::Statistics => {
                            let window = self.state.scroll_state.selected_window();
                            let (statistics, quality) = self
                                .state
                                .statistics_handler
                                .get_bigram(window)
                                .into_result_with_quality()
                                .unwrap()
                                .unwrap_or_else(|| (Statistics::empty_for_window(window), 0.0));
                            let rect = ui.max_rect().intersect(ui.cursor());

                            ui.vertical(|ui| {
                                hexbait::gui::statistics_display::render(
                                    &mut self.state.statistics_display_state,
                                    ui,
                                    rect,
                                    window,
                                    &statistics,
                                    quality,
                                    &self.state.settings,
                                );

                                let old = ui.spacing_mut().slider_width;
                                ui.spacing_mut().slider_width =
                                    self.state.settings.font_size() * 20.0;
                                ui.add(
                                    egui::Slider::new(
                                        &mut self.state.statistics_display_state.xor_value,
                                        0..=255,
                                    )
                                    .text("xor value"),
                                );
                                ui.spacing_mut().slider_width = old;

                                ui.label(format!(
                                    "search {:.02}% complete ({} results)",
                                    self.state.search.searcher.progress() * 100.0,
                                    self.state.search.searcher.results().len()
                                ));

                                ui.horizontal(|ui| {
                                    ui.text_edit_singleline(&mut self.state.search.search_text);
                                    let search_bytes = match self.state.search.search_bytes() {
                                        Ok(bytes) => Some(bytes),
                                        Err(msg) => {
                                            ui.label(
                                                RichText::new("⚠")
                                                    .color(ui.visuals().warn_fg_color),
                                            )
                                            .on_hover_ui(|ui| {
                                                ui.label(format!("invalid string literal: {msg}"));
                                            });
                                            None
                                        }
                                    };

                                    let valid_utf8 = search_bytes
                                        .as_ref()
                                        .map(|search_bytes| {
                                            std::str::from_utf8(search_bytes).is_ok()
                                        })
                                        .unwrap_or(false);

                                    ui.checkbox(
                                        &mut self.state.search.search_ascii_case_insensitive,
                                        "ASCII case insensitive",
                                    );
                                    ui.add_enabled(
                                        valid_utf8,
                                        egui::Checkbox::new(
                                            &mut self.state.search.search_utf16,
                                            "include UTF-16",
                                        ),
                                    );
                                    if ui
                                        .add_enabled(
                                            search_bytes.as_ref().is_some_and(|search_bytes| {
                                                !search_bytes.is_empty()
                                            }),
                                            egui::Button::new("start search"),
                                        )
                                        .clicked()
                                        && let Some(search_bytes) = &search_bytes
                                    {
                                        self.state.search.searcher.start_new_search(
                                            search_bytes,
                                            self.state.search.search_ascii_case_insensitive,
                                            self.state.search.search_utf16 && valid_utf8,
                                        );
                                    }
                                });
                            });
                        }
                        DisplayType::Hexview => {
                            let rect = ui.max_rect().intersect(ui.cursor());
                            ui.scope_builder(
                                UiBuilder::new()
                                    .max_rect(rect)
                                    .layout(Layout::left_to_right(Align::Min)),
                                |ui| {
                                    hexbait::gui::hex::render(ui, &mut self.state, &self.input);

                                    let rest_rect = ui.max_rect().intersect(ui.cursor());
                                    let half_height = rest_rect.height() / 2.0;

                                    let top_rect = Rect::from_min_size(
                                        rest_rect.min,
                                        vec2(rest_rect.width(), half_height),
                                    );

                                    let bottom_rect = Rect::from_min_size(
                                        rest_rect.min + vec2(0.0, half_height),
                                        vec2(rest_rect.width(), half_height),
                                    );

                                    ui.scope_builder(
                                        UiBuilder::new()
                                            .max_rect(top_rect)
                                            .layout(Layout::left_to_right(Align::Min)),
                                        |ui| {
                                            ScrollArea::vertical()
                                                .id_salt("inspector_scroll")
                                                .max_height(half_height)
                                                .show(ui, |ui| {
                                                    ui.vertical(|ui| {
                                                        hexbait::gui::modules::inspector::show(
                                                            ui,
                                                            &mut self.state,
                                                            &self.input,
                                                        );
                                                    });
                                                });
                                        },
                                    );

                                    ui.scope_builder(
                                        UiBuilder::new()
                                            .max_rect(bottom_rect)
                                            .layout(Layout::left_to_right(Align::Min)),
                                        |ui| {
                                            ScrollArea::both()
                                                .id_salt("parser_scroll")
                                                .max_height(half_height)
                                                .show(ui, |ui| {
                                                    hexbait::gui::modules::parsed_value::show(
                                                        ui,
                                                        &mut self.state,
                                                        &self.input,
                                                    );
                                                });
                                        },
                                    );
                                },
                            );
                        }
                    }

                    if jump_to_offset
                        && let Ok(offset) = self
                            .state
                            .parse_state
                            .parse_offset
                            .parse()
                            .map(AbsoluteOffset::from)
                    {
                        self.state.scroll_state.rearrange_bars_for_point(0, offset);
                    }
                },
            );
        });
        self.frame_time = start.elapsed();
    }
}
