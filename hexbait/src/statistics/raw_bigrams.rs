//! Implements counts of bigrams.

use std::{
    collections::BTreeMap,
    io,
    iter::Sum,
    ops::{AddAssign, SubAssign},
};

use hexbait_common::{Input, Len};

use crate::window::Window;

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
    pub(super) fn compute(
        &mut self,
        input: &mut Input,
        window: Window,
    ) -> Result<(Window, Option<u8>), io::Error> {
        raw_compute(
            self,
            |this, first, second| this.add_count(first, second, Count::from(1u8)),
            |this, first, second| {
                this.follow[second as usize][first as usize] -= Count::from(1u8);
            },
            input,
            window,
        )
    }

    /// Returns the count of values where `second` follows `first` in the window.
    pub(super) fn follow(&self, first: u8, second: u8) -> Count {
        self.follow[second as usize][first as usize]
    }

    /// Iterates over all non-zero counts.
    pub(super) fn iter_non_zero(&self) -> RawBigramNonZeroIter<'_, Count> {
        RawBigramNonZeroIter {
            raw: &self.follow,
            second: 0,
            first: 0,
        }
    }
}

/// An iterator over all non-zero counts.
pub(super) struct RawBigramNonZeroIter<'raw, Count> {
    /// The raw counts used by the iterator.
    raw: &'raw [[Count; 256]; 256],
    /// The current `second` value.
    second: usize,
    /// The current `first` value.
    first: usize,
}

impl<'raw, Count> Iterator for RawBigramNonZeroIter<'raw, Count>
where
    Count: Copy,
    u64: From<Count>,
{
    type Item = (u8, u8, u64);

    fn next(&mut self) -> Option<Self::Item> {
        if self.second == 256 {
            return None;
        }

        if self.first < 256 {
            let old_first = self.first;
            self.first += 1;

            Some((
                old_first as u8,
                self.second as u8,
                u64::from(self.raw[self.second][old_first]),
            ))
        } else {
            let old_second = self.second;
            self.second += 1;
            self.first = 0;

            Some((0, self.second as u8, u64::from(self.raw[old_second][0])))
        }
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
    pub(super) fn compute(
        &mut self,
        input: &mut Input,
        window: Window,
    ) -> Result<(Window, Option<u8>), io::Error> {
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
            input,
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
    pub(super) fn iter_non_zero(&self) -> RawSmallBigramNonZeroIter<'_> {
        RawSmallBigramNonZeroIter {
            iter: self.follow.iter(),
        }
    }
}

/// An iterator over all non-zero counts.
pub(super) struct RawSmallBigramNonZeroIter<'raw> {
    /// The underlying iterator.
    iter: std::collections::btree_map::Iter<'raw, (u8, u8), u16>,
}

impl<'raw> Iterator for RawSmallBigramNonZeroIter<'raw> {
    type Item = (u8, u8, u64);

    fn next(&mut self) -> Option<Self::Item> {
        self.iter
            .next()
            .map(|(&(second, first), &count)| (first, second, u64::from(count)))
    }
}

/// Computes the statistics.
fn raw_compute<T>(
    this: &mut T,
    mut increase_count: impl FnMut(&mut T, u8, u8),
    decrease_count: impl FnOnce(&mut T, u8, u8),
    input: &mut Input,
    window: Window,
) -> Result<(Window, Option<u8>), io::Error> {
    const WINDOW_SIZE: usize = 4096;

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

    // TODO: this can probably be optimized using SIMD, since this is completely independent of
    // any data but the previous byte (which is only required between subwindows)
    let mut prev_byte = byte_before_window.unwrap_or(DEFAULT_PREV_BYTE);
    let mut start = window.start();
    while start < window.end() {
        let max_size = std::cmp::min((window.end() - start).as_u64() as usize, WINDOW_SIZE);

        let subwindow = input.read_at(start, Len::from(max_size as u64), Some(&mut buf))?;

        for &byte in &*subwindow {
            increase_count(this, prev_byte, byte);
            prev_byte = byte;
        }

        start += Len::from(subwindow.len() as u64);

        if subwindow.is_empty() {
            break;
        }
    }
    // in case the originally given range was larger than the window
    let window_size = start - window.start();

    let first_byte = 'first_byte: {
        if byte_before_window.is_none() {
            // if there is no byte before this window, we initialize `prev_byte`
            if let Some(&first_byte) = input.read_at(window.start(), Len::from(1), None)?.first() {
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
