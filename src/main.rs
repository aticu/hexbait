#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use std::io::Read;

use hexamine::{
    data::{DataSource as _, Input},
    gui::{
        hex::HexdumpView, settings::Settings, signature_display::SignatureDisplay,
        zoombars::Zoombars,
    },
};

fn main() -> eframe::Result {
    let input = if let Some(arg) = std::env::args().nth(1) {
        Input::File(std::fs::File::open(arg).unwrap())
    } else {
        let mut buf = Vec::new();
        std::io::stdin().read_to_end(&mut buf).unwrap();
        Input::Stdin(buf)
    };

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([320.0, 240.0]),
        ..Default::default()
    };
    eframe::run_native(
        "My egui App",
        options,
        Box::new(|_| {
            Ok(Box::new(MyApp {
                frame_time: std::time::Duration::ZERO,
                settings: Settings::new(),
                input,
                hexdump_context: HexdumpView::new(),
                big_endian: false,
                zoombars: Zoombars::new(),
                signature_display: SignatureDisplay::new(),
            }))
        }),
    )
}

struct MyApp {
    frame_time: std::time::Duration,
    settings: Settings,
    input: Input,
    hexdump_context: HexdumpView,
    big_endian: bool,
    zoombars: Zoombars,
    signature_display: SignatureDisplay,
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

            let file_size = self.input.len().unwrap();
            // TODO: Test with input that is one row bigger than screen (and with input of exact
            // size)

            self.zoombars.render(
                ui,
                file_size,
                &mut self.input,
                &self.settings,
                |ui, source, start| {
                    self.hexdump_context.render(
                        ui,
                        &self.settings,
                        source,
                        &mut self.big_endian,
                        start,
                    );
                },
                |ui, source, range| {
                    let statistics = hexamine::statistics::Statistics::compute(
                        source,
                        *range.start()..*range.end(),
                    )
                    .unwrap();
                    let signature = statistics.to_signature();
                    let rect = ui.max_rect().intersect(ui.cursor());

                    self.signature_display.render(
                        ui,
                        rect,
                        *range.start()..*range.end(),
                        &signature,
                        &self.settings,
                    );
                },
            );

            return;
            // TODO: factor out continuous color schemes into a function
        });
        self.frame_time = start.elapsed();
    }
}
