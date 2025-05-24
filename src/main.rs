#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use hexamine::{
    data::DataSource as _,
    gui::{hex::HexdumpView, zoombars::Zoombars},
};

fn main() -> eframe::Result {
    let arg = std::env::args().nth(1).unwrap();
    let mut file = std::fs::File::open(arg).unwrap();
    let len = file.len().unwrap();

    let statistics = hexamine::statistics::Statistics::compute(&mut file, 0..len).unwrap();
    let signature = statistics.to_signature();

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
                signature,
                file,
                hexdump_context: HexdumpView::new(),
                big_endian: false,
                selected: None,
                selecting: false,
                zoombars: Zoombars::new(),
            }))
        }),
    )
}

struct MyApp {
    frame_time: std::time::Duration,
    signature: hexamine::statistics::Signature,
    file: std::fs::File,
    hexdump_context: HexdumpView,
    big_endian: bool,
    selected: Option<std::ops::RangeInclusive<u64>>,
    selecting: bool,
    zoombars: Zoombars,
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

            let file_size = self.file.len().unwrap();

            self.zoombars.render(
                ui,
                file_size,
                &mut self.file,
                |ui, source, start| {
                    self.hexdump_context
                        .render(ui, source, &mut self.big_endian, start);
                },
                |ui, source, range| {
                    let statistics = hexamine::statistics::Statistics::compute(
                        source,
                        *range.start()..*range.end(),
                    )
                    .unwrap();
                    let signature = statistics.to_signature();
                    hexamine::gui::signature_display::render_signature_display(ui, &signature);
                },
            );

            return;
            // TODO: fix up size = 3.0 (two places)
            // TODO: factor out continuous color schemes into a function
            // TODO: factor out scale
        });
        self.frame_time = start.elapsed();
    }
}
