//! Handles the user settings.

use egui::{Color32, FontId, TextStyle, Ui};

use crate::gui::color::{BYTE_COLORS, ColorMap};

/// Determine what to show in the main screen.
#[derive(Debug, Default, PartialEq, Eq, Clone, Copy)]
pub enum ViewKind {
    /// Determine based on the size of the selected window.
    #[default]
    Auto,
    /// Always show a hex view.
    ForceHexView,
    /// Always show a statistics view.
    ForceStatisticsView,
}

impl ViewKind {
    /// Returns this view kind as a displayable string.
    pub fn display_str(&self) -> &str {
        match self {
            ViewKind::Auto => "auto",
            ViewKind::ForceHexView => "hex only",
            ViewKind::ForceStatisticsView => "statistics only",
        }
    }
}

/// The settings of the GUI.
pub struct Settings {
    /// The scale of the GUI.
    ///
    /// This number is the font size of the hex text, but influences everything else.
    scale: f32,
    /// The color map to use.
    color_map: ColorMap,
    /// Whether to use linear colors for bytes.
    linear_byte_colors: bool,
    /// Whether to use fine grained displays in scroll bars.
    fine_grained_scrollbars: bool,
    /// The thing to display in the main screen.
    view_kind: ViewKind,
}

impl Settings {
    /// Creates new default settings.
    pub fn new() -> Settings {
        Settings {
            scale: 20.0,
            color_map: ColorMap::Viridis,
            linear_byte_colors: false,
            fine_grained_scrollbars: true,
            view_kind: ViewKind::Auto,
        }
    }

    /// Applies the current settings to the [`Ui`].
    pub fn apply_settings_to_ui(&self, ui: &mut Ui) {
        let text_styles = &mut ui.style_mut().text_styles;

        text_styles.insert(TextStyle::Small, FontId::proportional(self.scale * 0.65));
        text_styles.insert(TextStyle::Body, FontId::proportional(self.scale * 0.75));
        text_styles.insert(TextStyle::Monospace, FontId::monospace(self.scale * 0.75));
        text_styles.insert(TextStyle::Button, FontId::proportional(self.scale * 0.75));
        text_styles.insert(TextStyle::Heading, FontId::proportional(self.scale * 1.15));
        text_styles.insert(TextStyle::Name("hex".into()), FontId::monospace(self.scale));
    }

    /// Mutable access to the field determining whether linear byte colors are used.
    pub fn linear_byte_colors_mut(&mut self) -> &mut bool {
        &mut self.linear_byte_colors
    }

    /// Whether linear byte colors are used.
    pub fn linear_byte_colors(&self) -> bool {
        self.linear_byte_colors
    }

    /// Mutable access to the field determining whether fine grained scrollbars are used.
    pub fn fine_grained_scrollbars_mut(&mut self) -> &mut bool {
        &mut self.fine_grained_scrollbars
    }

    /// Whether fine grained scrollbars are used.
    pub fn fine_grained_scrollbars(&self) -> bool {
        self.fine_grained_scrollbars
    }

    /// The font size of normal text.
    pub fn font_size(&self) -> f32 {
        self.scale * 0.75
    }

    /// The font size of hex text.
    pub fn hex_font(&self) -> FontId {
        FontId::monospace(self.scale)
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

    /// The thing to display on the main screen.
    pub fn view_kind(&mut self) -> &mut ViewKind {
        &mut self.view_kind
    }

    /// A representative color for the given byte value.
    pub fn byte_color(&self, byte: u8) -> Color32 {
        if self.linear_byte_colors {
            self.scale_color_u8(byte)
        } else {
            BYTE_COLORS[byte as usize]
        }
    }

    /// A color along a scale from `0u8` to `255u8`.
    pub fn scale_color_u8(&self, scalar: u8) -> Color32 {
        self.color_map.get_map()[scalar as usize]
    }

    /// A color along a scale from `0.0` to `1.0`.
    pub fn scale_color_f32(&self, scalar: f32) -> Color32 {
        self.color_map.get_map()[(scalar.clamp(0.0, 1.0) * 255.0).round() as usize]
    }

    /// A color representing an entropy.
    pub fn entropy_color(&self, entropy: f32) -> Color32 {
        self.scale_color_f32(entropy)
    }

    /// The color representing missing data.
    pub fn missing_color(&self) -> Color32 {
        Color32::BROWN
    }

    /// The width multiplier of the scrollbars.
    pub fn bar_width_multiplier(&self) -> usize {
        3
    }
}

impl Default for Settings {
    fn default() -> Self {
        Settings::new()
    }
}
