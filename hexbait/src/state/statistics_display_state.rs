//! Implements the state required for the statistics display.

use crate::{gui::cached_image::CachedImage, window::Window};

/// The state used by the statistics display.
pub struct StatisticsDisplayState {
    /// The cached statistics display image.
    pub cached_image: CachedImage<(Window, u8, f32)>,
    /// The value to apply as an XOR mask for the statistics display.
    pub xor_value: u8,
}

impl StatisticsDisplayState {
    /// Creates a new statistics display state.
    pub fn new() -> StatisticsDisplayState {
        StatisticsDisplayState {
            cached_image: CachedImage::new(),
            xor_value: 0,
        }
    }
}

impl Default for StatisticsDisplayState {
    fn default() -> Self {
        StatisticsDisplayState::new()
    }
}
