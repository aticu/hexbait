//! Handles GUI settings.

use egui::Color32;

use super::color::{BYTE_COLORS, ColorMap};

/// The settings of the GUI.
pub struct Settings {
    /// The scale of the GUI.
    ///
    /// This number is the font size of the hex text, but influences everything else.
    scale: f32,
    /// The color map to use.
    color_map: ColorMap,
}

impl Settings {
    /// Creates new default settings.
    pub fn new() -> Settings {
        Settings {
            scale: 20.0,
            color_map: ColorMap::Viridis,
        }
    }

    /// The font size of normal text.
    pub fn font_size(&self) -> f32 {
        self.scale * 0.75
    }

    /// The font size of large text.
    pub fn large_font_size(&self) -> f32 {
        self.scale * 0.75
    }

    /// The font size of hex text.
    pub fn hex_font_size(&self) -> f32 {
        self.scale
    }

    /// The height of a hex char.
    pub fn char_height(&self) -> f32 {
        self.scale * 1.1
    }

    /// The width of a hex character.
    pub fn char_width(&self) -> f32 {
        self.scale * 0.6
    }

    /// The size of a small space.
    pub fn small_space(&self) -> f32 {
        self.scale * 0.6
    }

    /// The size of a large space.
    pub fn large_space(&self) -> f32 {
        self.scale * 1.7
    }

    /// The corner radius to use.
    pub fn corner_radius(&self) -> f32 {
        self.scale * 0.15
    }

    /// The stroke width to use for lines.
    pub fn stroke_width(&self) -> f32 {
        self.scale * 0.08
    }

    /// A representative color for the given byte value.
    pub fn byte_color(&self, byte: u8) -> Color32 {
        BYTE_COLORS[byte as usize]
    }

    /// A color along a scale from `0u8` to `255u8`.
    pub fn scale_color_u8(&self, scalar: u8) -> Color32 {
        self.color_map.get_map()[scalar as usize]
    }

    /// A color along a scale from `0.0` to `1.0`.
    pub fn scale_color_f32(&self, scalar: f32) -> Color32 {
        self.color_map.get_map()[(scalar.clamp(0.0, 1.0) * 255.0).round() as usize]
    }

    /// The width multiplier of the zoom and scrollbars.
    pub fn bar_width_multiplier(&self) -> usize {
        3
    }
}
