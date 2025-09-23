//! Implements counts of bigrams.

use std::{
    collections::BTreeMap,
    iter::Sum,
    ops::{AddAssign, SubAssign},
};

use crate::{data::DataSource, window::Window};

/// Computed statistics about bigrams in a window of data.
#[derive(Eq, PartialEq)]
pub(super) struct RawBigrams<Count> {
    /// `follow[b1][b2]` counts how many `b1`s follow a `b2` in the window.
    follow: Box<[[Count; 256]; 256]>,
}

impl<Count> RawBigrams<Count>
where
    Count: Copy + AddAssign<Count> + SubAssign<Count> + From<u8> + Ord + Sum<Count>,
    u64: From<Count>,
{
    /// Creates an empty raw bigram count.
    pub(super) fn empty() -> RawBigrams<Count> {
        RawBigrams {
            follow: Box::new([[Count::from(0u8); 256]; 256]),
        }
    }

    /// Adds the given count to the given bigram.
    pub(super) fn add_count(&mut self, first: u8, second: u8, count: Count) {
        self.follow[second as usize][first as usize] += count;
    }

    /// Fills the bigram counts with information about the given window.
    pub(super) fn compute<Source: DataSource>(
        &mut self,
        source: &mut Source,
        window: Window,
    ) -> Result<(Window, Option<u8>), Source::Error> {
        raw_compute(
            self,
            |this, first, second| this.add_count(first, second, Count::from(1u8)),
            |this, first, second| {
                this.follow[second as usize][first as usize] -= Count::from(1u8);
            },
            source,
            window,
        )
    }

    /// Returns the count of values where `second` follows `first` in the window.
    pub(super) fn follow(&self, first: u8, second: u8) -> Count {
        self.follow[second as usize][first as usize]
    }

    /// Iterates over all non-zero counts.
    pub(super) fn iter_non_zero(&self) -> impl Iterator<Item = (u8, u8, Count)> {
        self.follow.iter().enumerate().flat_map(|(second, row)| {
            row.iter()
                .enumerate()
                .map(move |(first, &count)| (first as u8, second as u8, count))
        })
    }
}

/// Computed statistics about small numbers bigrams in a window of data.
#[derive(Eq, PartialEq)]
pub(super) struct SmallRawBigrams {
    /// `follow[(b1, b2)]` counts how many `b1`s follow a `b2` in the window.
    follow: BTreeMap<(u8, u8), u16>,
}

impl SmallRawBigrams {
    /// Creates an empty raw bigram count.
    pub(super) fn empty() -> SmallRawBigrams {
        SmallRawBigrams {
            follow: BTreeMap::new(),
        }
    }

    /// Adds the given count to the given bigram.
    pub(super) fn add_count(&mut self, first: u8, second: u8, count: u16) {
        *self.follow.entry((second, first)).or_default() += count;
    }

    /// Fills the bigram counts with information about the given window.
    pub(super) fn compute<Source: DataSource>(
        &mut self,
        source: &mut Source,
        window: Window,
    ) -> Result<(Window, Option<u8>), Source::Error> {
        raw_compute(
            self,
            |this, first, second| this.add_count(first, second, 1),
            |this, first, second| {
                if let Some(count) = this.follow.get_mut(&(second, first)) {
                    *count -= 1;
                    if *count == 0 {
                        // remove the 0 count to keep the invariant that if a count exists, it is
                        // nonzero
                        this.follow.remove(&(second, first));
                    }
                }
            },
            source,
            window,
        )
    }

    /// Returns the count of values where `second` follows `first` in the window.
    pub(super) fn follow(&self, first: u8, second: u8) -> u16 {
        self.follow
            .get(&(second, first))
            .copied()
            .unwrap_or_default()
    }

    /// Iterates over all non-zero counts.
    pub(super) fn iter_non_zero(&self) -> impl Iterator<Item = (u8, u8, u16)> {
        self.follow
            .iter()
            .map(|(&(second, first), &val)| (first, second, val))
    }
}

/// Computes the statistics.
fn raw_compute<Source: DataSource, T>(
    this: &mut T,
    mut increase_count: impl FnMut(&mut T, u8, u8),
    decrease_count: impl FnOnce(&mut T, u8, u8),
    source: &mut Source,
    window: Window,
) -> Result<(Window, Option<u8>), Source::Error> {
    const WINDOW_SIZE: usize = 4096;

    let byte_before_window = if window.start() > 0 {
        source
            .window_at(window.start() - 1, &mut [0])?
            .first()
            .copied()
    } else {
        None
    };

    const DEFAULT_PREV_BYTE: u8 = 0;

    // TODO: this can probably be optimized using SIMD, since this is completely independent of
    // any data but the previous byte (which is only required between subwindows)
    let mut prev_byte = byte_before_window.unwrap_or(DEFAULT_PREV_BYTE);
    let mut start = window.start();
    while start < window.end() {
        let mut buf = [0; WINDOW_SIZE];
        let max_size = std::cmp::min((window.end() - start) as usize, WINDOW_SIZE);

        let subwindow = source.window_at(start, &mut buf[..max_size])?;

        for &byte in subwindow {
            increase_count(this, prev_byte, byte);
            prev_byte = byte;
        }

        start += subwindow.len() as u64;

        if subwindow.is_empty() {
            break;
        }
    }
    // in case the originally given range was larger than the window
    let window_size = start - window.start();

    let first_byte = 'first_byte: {
        if byte_before_window.is_none() {
            // if there is no byte before this window, we initialize `prev_byte`
            if let Some(&first_byte) = source.window_at(window.start(), &mut [0])?.first() {
                decrease_count(this, DEFAULT_PREV_BYTE, first_byte);

                break 'first_byte Some(first_byte);
            }
        }

        // no need to store the first byte for windows that start later in the file, as they
        // are already accounted for
        None
    };

    Ok((
        Window::from_start_len(window.start(), window_size),
        first_byte,
    ))
}
