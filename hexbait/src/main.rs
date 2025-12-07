//! Implements the hexbait application.
//!
//! This is a hexadecimal viewer and analysis tool.

#![forbid(unsafe_code)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use std::{
    collections::BTreeMap,
    io::{Read, Seek as _, SeekFrom},
    path::PathBuf,
};

use clap::Parser;
use egui::{Align, Layout, RichText, TextStyle, UiBuilder};
use hexbait::{
    built_in_format_descriptions::built_in_format_descriptions,
    data::Input,
    gui::{
        hex::HexdumpView,
        marking::{MarkedLocation, MarkedLocations, MarkingKind},
        signature_display::SignatureDisplay,
    },
    model::Endianness,
    state::State,
    statistics::{Statistics, StatisticsHandler},
};
use hexbait_common::AbsoluteOffset;

// TODO: change font to render more characters
// TODO: change default font-size and refactor around that
// TODO: implement to-disk caching for some statistic sizes to decrease re-load times
// TODO: re-use non-flat statistics for flat statistics
// TODO: fix up main file
// TODO: join polygons of adjoining marked locations
// TODO: unify views and input
// TODO: improve hover text for marked locations
// TODO: refactor scrollbars.rs
// TODO: refactor hex.rs
// TODO: implement more convenient escaping of byte arrays for search
// TODO: rearrange UI in a more useful way
// TODO: figure out why entropy calculations are sometimes so slow
// TODO: fix dragging across end during initial scrollbar selection
// TODO: add relative search (from here backwards/forwards)
// TODO: add screenshots to README
// TODO: add some user documentation

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
        let mut file = std::fs::File::open(file_name).unwrap();

        let len = file.seek(SeekFrom::End(0)).unwrap();

        Input::File {
            path: PathBuf::from(&file_name),
            file,
            len,
        }
    } else {
        let mut buf = Vec::new();
        std::io::stdin().read_to_end(&mut buf).unwrap();
        Input::Stdin(buf.into())
    };

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_maximized(true),
        ..Default::default()
    };
    eframe::run_native(
        "hexbait",
        options,
        Box::new(|_| {
            Ok(Box::new(MyApp {
                frame_time: std::time::Duration::ZERO,
                state: State::new(&input),
                statistics_handler: StatisticsHandler::new(input.try_clone().unwrap()),
                input,
                hexdump_context: HexdumpView::new(),
                endianness: Endianness::native(),
                signature_display: SignatureDisplay::new(),
                xor_value: 0,
                parse_type: "none",
                parse_offset: String::from("0"),
                sync_parse_offset_to_selection_start: true,
                marked_locations: MarkedLocations::new(),
                built_in_format_descriptions: built_in_format_descriptions(),
                custom_parser: config.parser_definition,
            }))
        }),
    )
}

struct MyApp {
    frame_time: std::time::Duration,
    state: State,
    statistics_handler: StatisticsHandler,
    input: Input,
    hexdump_context: HexdumpView,
    endianness: Endianness,
    signature_display: SignatureDisplay,
    xor_value: u8,
    parse_type: &'static str,
    parse_offset: String,
    sync_parse_offset_to_selection_start: bool,
    marked_locations: MarkedLocations,
    built_in_format_descriptions: BTreeMap<&'static str, hexbait_lang::ir::File>,
    custom_parser: Option<PathBuf>,
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
                egui::ComboBox::from_label("")
                    .selected_text(self.parse_type)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.parse_type, "none", "none");
                        if self.custom_parser.is_some() {
                            ui.selectable_value(
                                &mut self.parse_type,
                                "custom parser",
                                "custom parser",
                            );
                        }
                        for description in self.built_in_format_descriptions.keys() {
                            ui.selectable_value(&mut self.parse_type, description, *description);
                        }
                    });

                ui.label("Parse offset:");
                ui.text_edit_singleline(&mut self.parse_offset);
                if ui
                    .add_enabled(
                        self.parse_offset.parse::<u64>().is_ok(),
                        egui::Button::new("Jump to offset"),
                    )
                    .clicked()
                {
                    jump_to_offset = true;
                }
                ui.checkbox(
                    &mut self.sync_parse_offset_to_selection_start,
                    "Sync parse offset to selection start",
                );

                ui.checkbox(
                    self.state.settings.linear_byte_colors_mut(),
                    "Use linear byte colors",
                );
            });

            if jump_to_offset
                && let Ok(offset) = self.parse_offset.parse().map(AbsoluteOffset::from)
            {
                self.state.scroll_state.rearrange_bars_for_point(0, offset);
            }

            let mut parse_offset = self.parse_offset.parse().ok().map(AbsoluteOffset::from);

            ui.scope_builder(
                UiBuilder::new()
                    .max_rect(ui.max_rect().intersect(ui.cursor()))
                    .layout(Layout::left_to_right(Align::Min)),
                |ui| {
                    hexbait::gui::scrollbars::render(
                        ui,
                        &mut self.state.scroll_state,
                        &self.state.settings,
                        &mut self.marked_locations,
                        &self.statistics_handler,
                    );

                    match self.state.scroll_state.display_suggestion {
                        hexbait::state::DisplaySuggestion::Overview => {
                            let window = self.state.scroll_state.selected_window();
                            let (statistics, quality) = self
                                .statistics_handler
                                .get_bigram(window)
                                .into_result_with_quality()
                                .unwrap()
                                .unwrap_or_else(|| (Statistics::empty_for_window(window), 0.0));
                            let signature = statistics.to_signature();
                            let rect = ui.max_rect().intersect(ui.cursor());

                            ui.vertical(|ui| {
                                self.signature_display.render(
                                    ui,
                                    rect,
                                    window,
                                    &signature,
                                    self.xor_value,
                                    quality,
                                    &self.state.settings,
                                );

                                let old = ui.spacing_mut().slider_width;
                                ui.spacing_mut().slider_width =
                                    self.state.settings.font_size() * 20.0;
                                ui.add(
                                    egui::Slider::new(&mut self.xor_value, 0..=255)
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
                        hexbait::state::DisplaySuggestion::Hexview => {
                            let ir;

                            let parse_type = if self.parse_type == "custom parser" {
                                'parse_type: {
                                    let Ok(content) = std::fs::read_to_string(
                                        self.custom_parser.as_ref().expect(
                                            "if a custom parser is selected it should also exist",
                                        ),
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
                                self.built_in_format_descriptions.get(self.parse_type)
                            };

                            self.hexdump_context.render(
                                ui,
                                &self.state.settings,
                                &mut self.state.scroll_state,
                                &mut self.input,
                                &mut self.endianness,
                                parse_type,
                                &mut parse_offset,
                                &mut self.marked_locations,
                            );
                        }
                    }

                    self.statistics_handler
                        .end_of_frame(self.state.scroll_state.changed());
                },
            );

            self.marked_locations
                .remove_where(|loc| loc.kind() == MarkingKind::SearchResult);
            for result in self.state.search.searcher.results().iter() {
                self.marked_locations
                    .add(MarkedLocation::new(*result, MarkingKind::SearchResult));
            }
            self.marked_locations.end_of_frame();

            if self.sync_parse_offset_to_selection_start
                && let Some(parse_offset) = parse_offset
            {
                self.parse_offset = parse_offset.as_u64().to_string();
            }
        });
        self.frame_time = start.elapsed();
    }
}
