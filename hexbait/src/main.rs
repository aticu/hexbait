//! Implements the hexbait application.
//!
//! This is a hexadecimal viewer and analysis tool.

#![forbid(unsafe_code)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use std::path::PathBuf;

use clap::Parser;
use egui::{CentralPanel, Frame, Key, KeyboardShortcut, MenuBar, Modifiers, Panel, TextStyle, Ui};
use egui_dock::{DockArea, DockState, SurfaceIndex};
use hexbait::{
    gui::{
        diagnostic_emitter::emit_diagnostics,
        modules::{Context, TabType, hex_dock_state},
    },
    state::{JumpToError, State},
};
use hexbait_common::{AbsoluteOffset, Input};
use hexbait_lang::{
    compile::{CompileResult, compile_repl_statement},
    eval::{ValueKind, View, eval_repl_statement},
};

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
    parser_definitions: Vec<PathBuf>,
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
                    state: State::new(&input, config.parser_definitions),
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
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        // force dark theme for now, because hexbait is not optimized for light themes
        ui.ctx().set_theme(egui::Theme::Dark);

        let start = std::time::Instant::now();

        Panel::top("menubar").show(ui, |ui| {
            self.context.state.settings.apply_settings_to_ui(ui);
            MenuBar::new().ui(ui, |ui| {
                ui.menu_button("Tabs", |ui| {
                    // allow certain tabs to be toggled
                    for tab in &[TabType::Settings, TabType::Search, TabType::Marking] {
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

                if !self
                    .context
                    .state
                    .format_discovery
                    .is_in_format_discovery_mode()
                {
                    ui.label("Jump to:");

                    let id = ui.make_persistent_id("jump_to");

                    let shortcut = KeyboardShortcut::new(Modifiers::CTRL, Key::G);
                    let shortcut_pressed = ui.input_mut(|i| i.consume_shortcut(&shortcut));
                    if shortcut_pressed {
                        ui.memory_mut(|m| m.request_focus(id));
                    }

                    let mut jump_to_text = egui::TextEdit::singleline(
                        &mut self.context.state.general_gui_state.jump_to_expr,
                    )
                    .id(id)
                    .return_key(None)
                    .hint_text("Press Ctrl+G to focus")
                    .show(ui);

                    if jump_to_text.response.gained_focus() || shortcut_pressed {
                        jump_to_text.state.cursor.set_char_range(Some(
                            egui::text::CCursorRange::select_all(&jump_to_text.galley),
                        ));
                        jump_to_text.state.store(ui.ctx(), id);
                    }

                    if jump_to_text.response.changed() || jump_to_text.response.lost_focus() {
                        self.context.state.general_gui_state.jump_to_error = None;
                    }

                    if jump_to_text.response.has_focus() && ui.input(|i| i.key_pressed(Key::Enter))
                    {
                        let (ir, diagnostics) = match compile_repl_statement(
                            "jump_to_input",
                            &self.context.state.general_gui_state.jump_to_expr,
                        ) {
                            CompileResult::NoDiagnostics { ir } => (Some(ir), None),
                            CompileResult::WithWarnings { ir, diagnostics } => {
                                (Some(ir), Some(diagnostics))
                            }
                            CompileResult::Failure { diagnostics } => (None, Some(diagnostics)),
                        };

                        if let Some(diagnostics) = diagnostics {
                            self.context.state.general_gui_state.jump_to_error =
                                Some(JumpToError::Compile(diagnostics));
                        }

                        if let Some(ir) = &ir {
                            let view = View::from_input(self.context.input.clone());
                            let eval_result =
                                eval_repl_statement(ir, self.context.state.endianness, view);

                            if let ValueKind::Integer(int) = &eval_result.value.kind {
                                if let Ok(offset) = u64::try_from(int)
                                    && let offset = AbsoluteOffset::from(offset)
                                    && offset.as_len() < self.context.input.len()
                                {
                                    self.context
                                        .state
                                        .scroll_state
                                        .rearrange_bars_for_point(0, offset);
                                } else {
                                    self.context.state.general_gui_state.jump_to_error =
                                        Some(JumpToError::Message(format!(
                                            "`{:?}` is out of range",
                                            eval_result.value.kind
                                        )));
                                }
                            } else {
                                self.context.state.general_gui_state.jump_to_error =
                                    Some(JumpToError::Message(format!(
                                        "`{:?}` is not an integer",
                                        eval_result.value.kind
                                    )));
                            }

                            if !eval_result.diagnostics.is_empty() {
                                self.context.state.general_gui_state.jump_to_error =
                                    Some(JumpToError::Eval(eval_result));
                            }
                        }
                    }

                    egui::Popup::from_response(&jump_to_text.response)
                        .open(self.context.state.general_gui_state.jump_to_error.is_some())
                        .show(
                            |ui| match &self.context.state.general_gui_state.jump_to_error {
                                Some(JumpToError::Compile(diagnostics)) => {
                                    emit_diagnostics(ui, diagnostics)
                                }
                                Some(JumpToError::Eval(result)) => {
                                    for diagnostic in &result.diagnostics {
                                        ui.label(&diagnostic.message);
                                    }
                                }
                                Some(JumpToError::Message(msg)) => {
                                    ui.label(msg);
                                }
                                None => {}
                            },
                        );
                }

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
            .frame(Frame::central_panel(ui.style()).inner_margin(0.0))
            .show(ui, |ui| {
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
