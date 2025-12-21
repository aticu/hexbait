//! Implements rendering of the inspector window for bytes.

use egui::{Color32, Rect, RichText, Sense, Ui, vec2};
use hexbait_common::Endianness;

use crate::state::Settings;

/// Renders a data inspector, showing different views on the selected data.
pub(crate) fn render_inspector(
    ui: &mut Ui,
    selected: Option<&[u8]>,
    endianness: &mut Endianness,
    settings: &Settings,
) {
    let row_height = settings.font_size() * 1.1;

    ui.horizontal(|ui| {
        ui.selectable_value(endianness, Endianness::Little, "Little Endian");
        ui.selectable_value(endianness, Endianness::Big, "Big Endian");
    });

    let buf = selected.unwrap_or(&[]);
    let endianness = *endianness;

    macro_rules! read_int {
        ($type:ident) => {
            read_int!($type, offset = 0)
        };
        ($type:ident, offset = $offset:expr) => {{
            let offset: usize = $offset;
            buf.get(offset..offset + ::std::mem::size_of::<$type>())
                .map(|buf| {
                    let from_bytes = match endianness {
                        Endianness::Little => $type::from_le_bytes,
                        Endianness::Big => $type::from_be_bytes,
                    };

                    from_bytes(buf.try_into().unwrap())
                })
        }};
    }

    let show_char = |c: char| {
        let name = unicode_names2::name(c);
        if let Some(name) = name {
            format!("U+{:X}: {name} ({c:?})", c as u32)
        } else {
            format!("U+{:X}: ({c:?})", c as u32)
        }
    };

    let values = [
        (
            "8-bit binary",
            read_int!(u8).map(|byte| format!("0b{byte:08b}")),
        ),
        (
            "8-bit octal",
            read_int!(u8).map(|byte| format!("0o{byte:03o}")),
        ),
        ("8-bit unsigned", read_int!(u8).map(|int| int.to_string())),
        ("8-bit signed", read_int!(i8).map(|int| int.to_string())),
        ("16-bit unsigned", read_int!(u16).map(|int| int.to_string())),
        ("16-bit signed", read_int!(i16).map(|int| int.to_string())),
        ("32-bit unsigned", read_int!(u32).map(|int| int.to_string())),
        ("32-bit signed", read_int!(i32).map(|int| int.to_string())),
        ("64-bit unsigned", read_int!(u64).map(|int| int.to_string())),
        ("64-bit signed", read_int!(i64).map(|int| int.to_string())),
        (
            "128-bit unsigned",
            read_int!(u128).map(|int| int.to_string()),
        ),
        ("128-bit signed", read_int!(i128).map(|int| int.to_string())),
        (
            "32-bit float",
            read_int!(u32).map(|int| format!("{:?}", f32::from_bits(int))),
        ),
        (
            "64-bit float",
            read_int!(u64).map(|int| format!("{:?}", f64::from_bits(int))),
        ),
        (
            "GUID",
            (buf.len() >= 16).then(|| {
                format!(
                    "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
                    buf[3],
                    buf[2],
                    buf[1],
                    buf[0],
                    buf[5],
                    buf[4],
                    buf[7],
                    buf[6],
                    buf[8],
                    buf[9],
                    buf[10],
                    buf[11],
                    buf[12],
                    buf[13],
                    buf[14],
                    buf[15],
                )
            })
        ),
        ("UTF-8 character", {
            let c = match std::str::from_utf8(buf) {
                Ok(s) => s.chars().next(),
                Err(err) => {
                    let valid = err.valid_up_to();
                    if valid != 0 {
                        std::str::from_utf8(&buf[..valid]).unwrap().chars().next()
                    } else {
                        None
                    }
                }
            };

            c.map(show_char)
        }),
        ("UTF-16 character", {
            let c = {
                let mut u16_buf = [0; 2];
                let u16_buf = match (read_int!(u16), read_int!(u16, offset = 2)) {
                    (Some(val1), Some(val2)) => {
                        u16_buf[0] = val1;
                        u16_buf[1] = val2;
                        &u16_buf[..]
                    }
                    (Some(val), None) => {
                        u16_buf[0] = val;
                        &u16_buf[..1]
                    }
                    (None, _) => &[],
                };

                String::from_utf16(u16_buf)
                    .ok()
                    .and_then(|s| s.chars().next())
            };

            c.map(show_char)
        }),
        ("UTF-32 character", {
            let c = read_int!(u32).and_then(|val| char::try_from(val).ok());

            c.map(show_char)
        }),
        ("UTF-8 string", {
            std::str::from_utf8(buf)
                .ok()
                .filter(|s| !s.is_empty())
                .map(|s| format!("{s:?}"))
        }),
        ("UTF-16 string", {
            let u16buf = (0..buf.len())
                .step_by(size_of::<u16>())
                .filter_map(|i| read_int!(u16, offset = i))
                .collect::<Vec<_>>();

            String::from_utf16(&u16buf)
                .ok()
                .filter(|s| !s.is_empty())
                .map(|s| format!("{s:?}"))
        }),
        ("UTF-32 string", {
            let as_str = (0..buf.len())
                .step_by(size_of::<u32>())
                .filter_map(|i| read_int!(u32, offset = i))
                .map(char::from_u32)
                .collect::<Option<String>>();

            as_str.filter(|s| !s.is_empty()).map(|s| format!("{s:?}"))
        }),
        ("32-bit UNIX timestamp", {
            let int = read_int!(u32);

            int.and_then(|val| {
                let datetime = chrono::DateTime::from_timestamp(val.into(), 0);

                datetime.map(|datetime| format!("{datetime}"))
            })
        }),
        ("Windows FILETIME", {
            let int = read_int!(u64);

            int.and_then(|val| {
                const UNIX_DIFF_SECS: i64 = 11_644_473_600; // seconds between 1601-01-01 and 1970-01-01
                let secs = (val / 10_000_000) as i64 - UNIX_DIFF_SECS;
                let nsecs = ((val % 10_000_000) * 100) as u32;
                let datetime = chrono::DateTime::from_timestamp(secs, nsecs);

                datetime.map(|datetime| format!("{datetime}"))
            })
        }),
    ];

    use egui_extras::{Column, TableBuilder};
    TableBuilder::new(ui)
        .striped(true)
        .id_salt("inspector")
        .column(Column::exact(settings.font_size() * 11.0))
        .column(Column::remainder())
        .drag_to_scroll(false)
        .header(row_height * 1.5, |mut header| {
            header.col(|ui| {
                ui.heading(RichText::new("Type").heading());
            });
            header.col(|ui| {
                ui.heading(RichText::new("Value").heading());
            });
        })
        .body(|mut body| {
            for (name, value) in &values {
                if let Some(value) = value {
                    body.row(row_height, |mut row| {
                        row.col(|ui| {
                            ui.label(*name);
                        });
                        row.col(|ui| {
                            ui.label(value);
                        });
                    });
                }
            }

            if buf.len() >= 3 {
                body.row(row_height, |mut row| {
                    row.col(|ui| {
                        ui.label("RGB8 color");
                    });
                    row.col(|ui| {
                        let r = buf[0];
                        let g = buf[1];
                        let b = buf[2];
                        let color = Color32::from_rgba_premultiplied(r, g, b, 255);
                        let rect = Rect::from_min_size(
                            ui.cursor().min,
                            vec2(settings.font_size() * 8.0, settings.font_size()),
                        );
                        ui.painter().rect_filled(rect, 0.0, color);
                        ui.allocate_rect(rect, Sense::hover())
                            .on_hover_ui_at_pointer(|ui| {
                                ui.label(format!(
                                    "R: {} ({:0.03}), G: {} ({:0.03}), B: {} ({:0.03})",
                                    r,
                                    r as f32 / 255.0,
                                    g,
                                    g as f32 / 255.0,
                                    b,
                                    b as f32 / 255.0
                                ));
                                ui.label(format!("#{r:02x}{g:02x}{b:02x}"));
                            });
                    });
                });
            }

            if buf.len() >= 2 {
                body.row(row_height, |mut row| {
                    row.col(|ui| {
                        ui.label("RGB565 color");
                    });
                    row.col(|ui| {
                        let val = read_int!(u16).unwrap();
                        let raw_r = val >> 11;
                        let raw_g = (val >> 5) & 0b111111;
                        let raw_b = val & 0b11111;
                        let r = ((raw_r as f32 / 31.0) * 255.0).round() as u8;
                        let g = ((raw_g as f32 / 63.0) * 255.0).round() as u8;
                        let b = ((raw_b as f32 / 31.0) * 255.0).round() as u8;
                        let color = Color32::from_rgba_premultiplied(r, g, b, 255);
                        let rect = Rect::from_min_size(
                            ui.cursor().min,
                            vec2(settings.font_size() * 8.0, settings.font_size()),
                        );
                        ui.painter().rect_filled(rect, 0.0, color);
                        ui.allocate_rect(rect, Sense::hover())
                            .on_hover_ui_at_pointer(|ui| {
                                ui.label(format!(
                                    "R: {} ({:0.03}), G: {} ({:0.03}), B: {} ({:0.03})",
                                    r,
                                    r as f32 / 255.0,
                                    g,
                                    g as f32 / 255.0,
                                    b,
                                    b as f32 / 255.0
                                ));
                                ui.label(format!("#{r:02x}{g:02x}{b:02x}"));
                            });
                    });
                });
            }

            if let Some(ty) = infer::get(buf) {
                body.row(row_height, |mut row| {
                    row.col(|ui| {
                        ui.label("mime type");
                    });
                    row.col(|ui| {
                        ui.label(ty.mime_type());
                    });
                });
            }
        });
}
