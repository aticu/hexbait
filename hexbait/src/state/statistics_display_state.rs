//! Implements the state required for the statistics display.

use crate::{
    gui::{color::ColorMap, image_processing::CachedImage},
    window::Window,
};

/// The state used by the statistics display.
pub struct StatisticsDisplayState {
    /// The cached statistics display image.
    pub cached_image: CachedImage<(Window, f32, ColorMap, f64)>,
}

impl StatisticsDisplayState {
    /// Creates a new statistics display state.
    pub fn new() -> StatisticsDisplayState {
        StatisticsDisplayState {
            cached_image: CachedImage::new(false),
        }
    }
}

impl Default for StatisticsDisplayState {
    fn default() -> Self {
        StatisticsDisplayState::new()
    }
}
