#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use std::{io::Read, path::PathBuf};

use hexbait::{
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

fn main() -> eframe::Result {
    let input = if let Some(arg) = std::env::args().nth(1) {
        Input::File {
            path: PathBuf::from(&arg),
            file: std::fs::File::open(arg).unwrap(),
        }
    } else {
        let mut buf = Vec::new();
        std::io::stdin().read_to_end(&mut buf).unwrap();
        Input::Stdin(buf.into())
    };

    let parse = hexbait_lang::parse(include_str!("../../format_descriptions/pe.hbl"));
    dbg!(&parse.ast);
    let ir = hexbait_lang::ir::lower_file(parse.ast);
    let mut input = input;
    let view = match &mut input {
        Input::File { file, .. } => hexbait_lang::View::File(file),
        Input::Stdin(bytes) => hexbait_lang::View::Bytes(&*bytes),
    };
    let out = hexbait_lang::eval_ir(&ir, view);
    dbg!(&ir);
    dbg!(out);
    dbg!(hexbait::parsing::tmp_pe_file());

    let statistics_cache_input = input.clone().unwrap();
    let searcher = Searcher::new(&input);

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
                input,
                hexdump_context: HexdumpView::new(),
                endianness: Endianness::native(),
                zoombars: Zoombars::new(),
                signature_display: SignatureDisplay::new(),
                xor_value: 0,
                statistics_handler: StatisticsHandler::new(statistics_cache_input),
                searcher,
                search_text: String::new(),
                parse_type: "none",
                parse_offset: String::from("0"),
                sync_parse_offset_to_selection_start: true,
                marked_locations: MarkedLocations::new(),
            }))
        }),
    )
}

struct MyApp {
    frame_time: std::time::Duration,
    settings: Settings,
    input: Input,
    hexdump_context: HexdumpView,
    endianness: Endianness,
    zoombars: Zoombars,
    signature_display: SignatureDisplay,
    xor_value: u8,
    statistics_handler: StatisticsHandler,
    searcher: Searcher,
    search_text: String,
    parse_type: &'static str,
    parse_offset: String,
    sync_parse_offset_to_selection_start: bool,
    marked_locations: MarkedLocations,
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
                        ui.selectable_value(&mut self.parse_type, "pe_file", "pe_file");
                        ui.selectable_value(&mut self.parse_type, "ntfs_header", "ntfs_header");
                        ui.selectable_value(&mut self.parse_type, "mft_entry", "mft_entry");
                        ui.selectable_value(
                            &mut self.parse_type,
                            "mft_index_entry",
                            "mft_index_entry",
                        );
                        ui.selectable_value(
                            &mut self.parse_type,
                            "bitlocker_header",
                            "bitlocker_header",
                        );
                        ui.selectable_value(
                            &mut self.parse_type,
                            "bitlocker_information",
                            "bitlocker_information",
                        );
                        ui.selectable_value(
                            &mut self.parse_type,
                            "vhdx_region_table",
                            "vhdx_region_table",
                        );
                        ui.selectable_value(
                            &mut self.parse_type,
                            "vhdx_metadata_table",
                            "vhdx_metadata_table",
                        );
                        ui.selectable_value(&mut self.parse_type, "vmdk_header", "vmdk_header");
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
            // TODO: implement to-disk caching for some sizes to increase re-load times
            // TODO: fix up main file
            // TODO: use "inner" and "outer" color for displaying marked locations

            let mut parse_offset = self.parse_offset.parse().ok();

            self.zoombars.render(
                ui,
                file_size,
                &mut self.input,
                &self.settings,
                &mut self.marked_locations,
                &self.statistics_handler,
                |ui, source, start, marked_locations| {
                    self.hexdump_context.render(
                        ui,
                        &self.settings,
                        source,
                        &mut self.endianness,
                        start,
                        (self.parse_type, &mut parse_offset),
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
                        ui.text_edit_singleline(&mut self.search_text);
                        if ui.button("start search").clicked() {
                            self.searcher
                                .start_new_search(self.search_text.as_bytes(), false);
                        }
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
