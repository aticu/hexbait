//! Implements the hexbait application.
//!
//! This is a hexadecimal viewer and analysis tool.

#![forbid(unsafe_code)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use std::path::PathBuf;

use clap::Parser;
use egui::{CentralPanel, Frame, MenuBar, TextStyle, TopBottomPanel};
use egui_dock::{DockArea, DockState, SurfaceIndex};
use hexbait::{
    gui::modules::{Context, TabType, hex_dock_state},
    state::State,
};
use hexbait_common::Input;

// TODO: change font to render more characters
// TODO: implement to-disk caching for some statistic sizes to decrease re-load times
// TODO: re-use non-flat statistics for flat statistics
// TODO: join polygons of adjoining marked locations
// TODO: improve hover text for marked locations
// TODO: implement more convenient escaping of byte arrays for search
// TODO: rearrange UI in a more useful way
// TODO: fix dragging across end during initial scrollbar selection
// TODO: add relative search (from here backwards/forwards)
// TODO: add screenshots to README
// TODO: add some user documentation
// TODO: fix statistics bug where 00 01 has high probability on zero-only content
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

/// The main entry point for the application.
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
            Ok(Box::new(HexbaitApp {
                frame_time: std::time::Duration::ZERO,
                context: Context {
                    state: State::new(&input, config.parser_definition),
                    input,
                },
                dock_state: hex_dock_state(),
            }))
        }),
    )
}

/// The hexbait application state.
struct HexbaitApp {
    /// The time it took to render the last frame.
    frame_time: std::time::Duration,
    /// The context required to render the hexbait application.
    context: Context,
    /// The dock state of the view.
    dock_state: DockState<TabType>,
}

impl eframe::App for HexbaitApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let start = std::time::Instant::now();

        TopBottomPanel::top("menubar").show(ctx, |ui| {
            self.context.state.settings.apply_settings_to_ui(ui);
            MenuBar::new().ui(ui, |ui| {
                ui.menu_button("Tabs", |ui| {
                    // allow certain tabs to be toggled
                    for tab in &[TabType::Settings, TabType::Search] {
                        let open = self.dock_state.find_tab(tab).is_some();

                        if ui.selectable_label(open, format!("{tab:?}")).clicked() {
                            if let Some(index) = self.dock_state.find_tab(tab) {
                                self.dock_state.remove_tab(index);
                            } else {
                                self.dock_state[SurfaceIndex::main()].push_to_focused_leaf(*tab);
                            }

                            ui.close();
                        }
                    }
                });

                if let Some(fps) = 1_000_000_000u128.checked_div(self.frame_time.as_nanos()) {
                    ui.painter().text(
                        ui.max_rect().right_top(),
                        egui::Align2::RIGHT_TOP,
                        format!("{fps} FPS"),
                        TextStyle::Small.resolve(ui.style()),
                        ui.visuals().text_color(),
                    );
                }
            })
        });

        CentralPanel::default()
            .frame(Frame::central_panel(&ctx.style()).inner_margin(0.0))
            .show(ctx, |ui| {
                self.context.state.settings.apply_settings_to_ui(ui);
                DockArea::new(&mut self.dock_state)
                    .show_leaf_collapse_buttons(false)
                    .show_leaf_close_all_buttons(false)
                    .show_inside(ui, &mut self.context);
            });

        self.context.state.end_of_frame();
        self.frame_time = start.elapsed();
    }
}
