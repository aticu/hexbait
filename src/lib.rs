//! Implements the backbone functionality of the hexbait application.

use std::time::Duration;

/// The idling time in case no user input is present.
pub(crate) const IDLE_TIME: Duration = Duration::from_millis(100);

pub mod data;
pub mod gui;
pub mod model;
pub mod parsing;
pub mod search;
pub mod statistics;
pub mod window;
