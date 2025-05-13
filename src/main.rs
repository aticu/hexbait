#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use hexamine::{data::DataSource as _, gui::hex::HexdumpView};

fn main() -> eframe::Result {
    let arg = std::env::args().nth(1).unwrap();
    let mut file = std::fs::File::open(arg).unwrap();
    let len = file.len().unwrap();

    let mut buf = [0; 256];
    file.window_at(0, &mut buf).unwrap();
    dbg!(entropy(&buf) / 8.0);

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

            self.hexdump_context
                .render(ui, &mut self.file, &mut self.big_endian);
            return;

            const SIDE_LEN: f32 = 4.0;
            let rect = egui::Rect::from_min_size(
                ui.cursor().left_top(),
                egui::vec2(SIDE_LEN * 256.0, SIDE_LEN * 256.0),
            );
            let response = ui.allocate_rect(rect, egui::Sense::hover());

            for first in 0..=255 {
                for second in 0..=255 {
                    let rect = egui::Rect::from_min_size(
                        rect.left_top()
                            + egui::vec2(SIDE_LEN * first as f32, SIDE_LEN * second as f32),
                        egui::vec2(SIDE_LEN, SIDE_LEN),
                    );
                    let painter = ui.painter().with_clip_rect(rect);

                    let intensity = self.signature.tuple(first, second);
                    let color = hexamine::gui::color::VIRIDIS[intensity as usize];

                    if let Some(pos) = response.hover_pos() {
                        if rect.contains(pos) {
                            egui::show_tooltip_at_pointer(
                                ui.ctx(),
                                ui.layer_id(),
                                "overview_hover".into(),
                                |ui| {
                                    ui.vertical(|ui| {
                                        ui.horizontal(|ui| {
                                            hexamine::gui::hex::render_hex(
                                                ui,
                                                20.0,
                                                egui::Sense::hover(),
                                                first,
                                            );
                                            hexamine::gui::hex::render_hex(
                                                ui,
                                                20.0,
                                                egui::Sense::hover(),
                                                second,
                                            );

                                            ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
                                            ui.add_space(30.0);

                                            hexamine::gui::hex::render_glyph(
                                                ui,
                                                20.0,
                                                egui::Sense::hover(),
                                                first,
                                            );
                                            hexamine::gui::hex::render_glyph(
                                                ui,
                                                20.0,
                                                egui::Sense::hover(),
                                                second,
                                            );
                                        });
                                        ui.label(
                                            egui::RichText::new(format!(
                                                "Relative Density: {:0.02}%",
                                                intensity as f64 / 2.55,
                                            ))
                                            .color(color),
                                        );
                                    });
                                },
                            );
                        }
                    }

                    painter.rect_filled(rect, 0.0, color);
                }
            }
        });
        self.frame_time = start.elapsed();
    }
}

fn entropy(bytes: &[u8]) -> f64 {
    let mut counts = [0u64; 256];

    for &byte in bytes {
        counts[byte as usize] += 1;
    }
    let total = counts.iter().sum::<u64>() as f64;

    -counts
        .into_iter()
        .filter(|&count| count != 0)
        .map(|count| count as f64 / total)
        .map(|p| p * p.log2())
        .sum::<f64>()
}
