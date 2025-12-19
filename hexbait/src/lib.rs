//! Implements the backbone functionality of the hexbait application.

#![forbid(unsafe_code)]

use std::time::Duration;

/// The idling time in case no user input is present.
pub(crate) const IDLE_TIME: Duration = Duration::from_millis(100);

pub mod data;
pub mod gui;
pub mod search;
pub mod state;
pub mod statistics;
pub mod window;
