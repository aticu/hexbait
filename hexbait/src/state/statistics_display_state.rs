//! Implements the state required for the statistics display.

use crate::{
    gui::{cached_image::CachedImage, color::ColorMap},
    window::Window,
};

/// The state used by the statistics display.
pub struct StatisticsDisplayState {
    /// The cached statistics display image.
    pub cached_image: CachedImage<(Window, f32, ColorMap)>,
}

impl StatisticsDisplayState {
    /// Creates a new statistics display state.
    pub fn new() -> StatisticsDisplayState {
        StatisticsDisplayState {
            cached_image: CachedImage::new(),
        }
    }
}

impl Default for StatisticsDisplayState {
    fn default() -> Self {
        StatisticsDisplayState::new()
    }
}
