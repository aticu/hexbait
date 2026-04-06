//! Implements counts of bigrams.

use std::io;

use hexbait_common::{Input, Len};

use crate::window::Window;

/// Computed statistics about bigrams in a window of data.
#[derive(Eq, PartialEq, Clone)]
pub(super) struct RawBigrams {
    /// `follow[b1][b2]` counts how many `b1`s follow a `b2` in the window.
    follow: Box<[[u64; 256]; 256]>,
}

impl RawBigrams {
    /// Creates an empty raw bigram count.
    pub(super) fn empty() -> RawBigrams {
        RawBigrams {
            follow: Box::new([[0; 256]; 256]),
        }
    }

    /// Fills the bigram counts with information about the given window.
    pub(super) fn compute(&mut self, input: &Input, window: Window) -> Result<Window, io::Error> {
        const WINDOW_SIZE: usize = 4 * 1024 * 1024;

        let byte_before_window = if window.start().is_start_of_file() {
            None
        } else {
            input
                .read_at(window.start() - Len::from(1), Len::from(1), None)?
                .first()
                .copied()
        };

        const DEFAULT_PREV_BYTE: u8 = 0;

        let mut buf = Vec::new();

        let mut prev_byte = byte_before_window.unwrap_or(DEFAULT_PREV_BYTE);
        let mut start = window.start();
        while start < window.end() {
            let max_size = std::cmp::min((window.end() - start).as_u64() as usize, WINDOW_SIZE);

            let subwindow = input.read_at(start, Len::from(max_size as u64), Some(&mut buf))?;

            if let Some(&first) = subwindow.first() {
                self.follow[first as usize][prev_byte as usize] += 1;
            }
            for pair in subwindow.windows(2) {
                self.follow[pair[1] as usize][pair[0] as usize] += 1;
            }
            prev_byte = subwindow.last().copied().unwrap_or(prev_byte);

            start += Len::from(subwindow.len() as u64);

            if subwindow.is_empty() {
                break;
            }
        }
        // in case the originally given range was larger than the window
        let window_size = start - window.start();

        Ok(Window::from_start_len(window.start(), window_size))
    }

    /// Returns the count of values where `second` follows `first` in the window.
    pub(super) fn follow(&self, first: u8, second: u8) -> u64 {
        self.follow[second as usize][first as usize]
    }

    /// Returns access to the raw counts.
    pub(super) fn raw_counts(&self) -> &[[u64; 256]; 256] {
        &self.follow
    }

    /// Returns mutabl access to the raw counts.
    pub(super) fn raw_counts_mut(&mut self) -> &mut [[u64; 256]; 256] {
        &mut self.follow
    }
}
