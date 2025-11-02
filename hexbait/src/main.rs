#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use std::{collections::BTreeMap, io::Read, path::PathBuf};

use clap::Parser;
use hexbait::{
    built_in_format_descriptions::built_in_format_descriptions,
    data::{DataSource as _, Input},
    gui::{
        hex::HexdumpView,
        marking::{MarkedLocation, MarkedLocations, MarkingKind},
        settings::Settings,
        signature_display::SignatureDisplay,
        zoombars::Zoombars,
    },
    model::Endianness,
    search::Searcher,
    statistics::{Statistics, StatisticsHandler},
};

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

    let input = if let Some(file) = &config.file {
        Input::File {
            path: PathBuf::from(&file),
            file: std::fs::File::open(file).unwrap(),
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
                settings: Settings::new(),
                searcher: Searcher::new(&input),
                statistics_handler: StatisticsHandler::new(input.clone().unwrap()),
                input,
                hexdump_context: HexdumpView::new(),
                endianness: Endianness::native(),
                zoombars: Zoombars::new(),
                signature_display: SignatureDisplay::new(),
                xor_value: 0,
                search_text: String::new(),
                search_ascii_case_insensitive: false,
                search_utf16: false,
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
    settings: Settings,
    searcher: Searcher,
    statistics_handler: StatisticsHandler,
    input: Input,
    hexdump_context: HexdumpView,
    endianness: Endianness,
    zoombars: Zoombars,
    signature_display: SignatureDisplay,
    xor_value: u8,
    search_text: String,
    search_ascii_case_insensitive: bool,
    search_utf16: bool,
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
            if let Some(fps) = 1_000_000_000u128.checked_div(self.frame_time.as_nanos()) {
                ui.painter().text(
                    ui.max_rect().right_top(),
                    egui::Align2::RIGHT_TOP,
                    format!("{fps} FPS"),
                    ui.style()
                        .text_styles
                        .get(&egui::TextStyle::Body)
                        .unwrap()
                        .clone(),
                    ui.visuals().text_color(),
                );
            }

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
                ui.checkbox(
                    &mut self.sync_parse_offset_to_selection_start,
                    "Sync parse offset to selection start",
                );

                ui.checkbox(
                    self.settings.linear_byte_colors_mut(),
                    "Use linear byte colors",
                );
            });

            let file_size = self.input.len().unwrap();
            // TODO: change font to render more characters
            // TODO: change default font-size and refactor around that
            // TODO: implement to-disk caching for some sizes to decrease re-load times
            // TODO: fix up main file
            // TODO: use outer color for displays in zoombars
            // TODO: join polygons of adjoining marked locations
            // TODO: remove data source and use concrete types instead

            let mut parse_offset = self.parse_offset.parse().ok();

            self.zoombars.render(
                ui,
                file_size,
                &mut self.input,
                &self.settings,
                &mut self.marked_locations,
                &self.statistics_handler,
                |ui, source, start, marked_locations| {
                    let ir;

                    let parse_type = if self.parse_type == "custom parser" {
                        'parse_type: {
                            let Ok(content) = std::fs::read_to_string(
                                self.custom_parser
                                    .as_ref()
                                    .expect("if a custom parser is selected it should also exist"),
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
                        &self.settings,
                        source,
                        &mut self.endianness,
                        start,
                        (parse_type, &mut parse_offset),
                        marked_locations,
                    );
                },
                |ui, window| {
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
                            &self.settings,
                        );

                        let old = ui.spacing_mut().slider_width;
                        ui.spacing_mut().slider_width = self.settings.font_size() * 20.0;
                        ui.add(egui::Slider::new(&mut self.xor_value, 0..=255).text("xor value"));
                        ui.spacing_mut().slider_width = old;

                        ui.label(format!(
                            "search {:.02}% complete ({} results)",
                            self.searcher.progress() * 100.0,
                            self.searcher.results().len()
                        ));

                        ui.horizontal(|ui| {
                            ui.text_edit_singleline(&mut self.search_text);
                            let mut search_bytes = Vec::new();
                            // TODO: implement more convenient escaping here
                            let valid = match hexbait_lang::ir::str_lit_content_to_bytes(
                                &self.search_text,
                                &mut search_bytes,
                            ) {
                                Ok(()) => true,
                                Err((msg, _)) => {
                                    ui.label(
                                        egui::RichText::new("⚠").color(ui.visuals().warn_fg_color),
                                    )
                                    .on_hover_ui(|ui| {
                                        ui.label(format!("invalid string literal: {msg}"));
                                    });
                                    false
                                }
                            };
                            let valid_utf8 = std::str::from_utf8(&search_bytes).is_ok();

                            ui.checkbox(
                                &mut self.search_ascii_case_insensitive,
                                "ASCII case insensitive",
                            );
                            ui.add_enabled(
                                valid_utf8,
                                egui::Checkbox::new(&mut self.search_utf16, "include UTF-16"),
                            );
                            if ui
                                .add_enabled(
                                    valid && !search_bytes.is_empty(),
                                    egui::Button::new("start search"),
                                )
                                .clicked()
                            {
                                self.searcher.start_new_search(
                                    &search_bytes,
                                    self.search_ascii_case_insensitive,
                                    self.search_utf16 && valid_utf8,
                                );
                            }
                        });
                    });
                },
            );

            self.marked_locations
                .remove_where(|loc| loc.kind() == MarkingKind::SearchResult);
            for result in self.searcher.results().iter() {
                self.marked_locations
                    .add(MarkedLocation::new(*result, MarkingKind::SearchResult));
            }
            // TODO: jump to location
            // TODO: figure out why entropy calculations are sometimes so slow
            // TODO: fix dragging across end during initial zoombar selection

            self.statistics_handler
                .end_of_frame(self.zoombars.changed());

            if self.sync_parse_offset_to_selection_start {
                if let Some(parse_offset) = parse_offset {
                    self.parse_offset = parse_offset.to_string();
                }
            }
        });
        self.frame_time = start.elapsed();
    }
}
