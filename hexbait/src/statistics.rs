//! Compute and represent statistics about windows of data.

use std::{fmt, io, ops::AddAssign};

use hexbait_common::Input;
use raw_bigrams::{RawBigrams, SmallRawBigrams};
use size_format::SizeFormatterBinary;

use crate::window::Window;

mod flat;
mod handler;
mod raw_bigrams;

pub use handler::{StatisticsHandler, StatisticsResult};

pub use flat::FlatStatistics;

/// Computed statistics about a window of data.
#[derive(Eq, PartialEq)]
enum StatisticsKind {
    /// Statistics over a large window of data (>4GiB).
    Large(RawBigrams<u64>),
    /// Statistics over a medium window of data (64KiB to 4GiB).
    Medium(RawBigrams<u32>),
    /// Statistics over a small window of data (<64KiB).
    Small(SmallRawBigrams),
}

impl StatisticsKind {
    /// Allocates an appropriate statistics kind for the given window size.
    fn with_capacity(capacity: u64) -> StatisticsKind {
        if capacity > u64::from(u32::MAX) {
            StatisticsKind::Large(RawBigrams::empty())
        } else if capacity > u64::from(u16::MAX) {
            StatisticsKind::Medium(RawBigrams::empty())
        } else {
            StatisticsKind::Small(SmallRawBigrams::empty())
        }
    }
}

/// Computed statistics about a window of data.
#[derive(Eq, PartialEq)]
pub struct Statistics {
    /// The actual statistics.
    statistics: StatisticsKind,
    /// The window over which the statistics were calculated.
    window: Window,
    /// The first byte in the window, if it is the first window.
    first_byte: Option<u8>,
}

impl Statistics {
    /// Creates new empty statistics starting at the beginning of the given window.
    ///
    /// The capacity of the window is chosen such that it will fit the whole given window.
    pub fn empty_for_window(window: Window) -> Statistics {
        Statistics {
            statistics: StatisticsKind::with_capacity(window.size().as_u64()),
            window: Window::empty_from_start(window.start()),
            first_byte: None,
        }
    }

    /// Computes statistics about a given window of data.
    pub fn compute(input: &mut Input, window: Window) -> Result<Statistics, io::Error> {
        let capacity = window.size();
        let mut statistics = StatisticsKind::with_capacity(capacity.as_u64());

        let (window, first_byte) = match &mut statistics {
            StatisticsKind::Large(raw_bigrams) => raw_bigrams.compute(input, window)?,
            StatisticsKind::Medium(raw_bigrams) => raw_bigrams.compute(input, window)?,
            StatisticsKind::Small(raw_bigrams) => raw_bigrams.compute(input, window)?,
        };

        Ok(Statistics {
            statistics,
            window,
            first_byte,
        })
    }

    /// Adds an empty window to the statistics.
    ///
    /// This makes the statistics not fully representative of the input.
    #[track_caller]
    pub fn add_empty_window(&mut self, window: Window) {
        let Some(window) = self.window.joined(window) else {
            panic!(
                "statistics must be adjacent to be added:\nstatistics: {self:?}\nnew window: {window:?}",
            );
        };

        self.window = window;
    }

    /// Converts the given statistics to flat statistics.
    pub fn to_flat(&self) -> FlatStatistics {
        FlatStatistics::from_bigrams(self)
    }

    /// Returns the number of times that `first` is followed by `second` in the statistics.
    pub fn follow(&self, first: u8, second: u8) -> u64 {
        match &self.statistics {
            StatisticsKind::Large(raw_statistics) => raw_statistics.follow(first, second),
            StatisticsKind::Medium(raw_statistics) => {
                u64::from(raw_statistics.follow(first, second))
            }
            StatisticsKind::Small(raw_statistics) => {
                u64::from(raw_statistics.follow(first, second))
            }
        }
    }

    /// Iterates over all non-zero values in this statistic.
    pub fn iter_non_zero(&self) -> impl Iterator<Item = (u8, u8, u64)> {
        enum IterKind<'this> {
            Small(raw_bigrams::RawSmallBigramNonZeroIter<'this>),
            Medium(raw_bigrams::RawBigramNonZeroIter<'this, u32>),
            Large(raw_bigrams::RawBigramNonZeroIter<'this, u64>),
        }

        impl<'this> Iterator for IterKind<'this> {
            type Item = (u8, u8, u64);

            fn next(&mut self) -> Option<Self::Item> {
                match self {
                    IterKind::Small(iter) => iter.next(),
                    IterKind::Medium(iter) => iter.next(),
                    IterKind::Large(iter) => iter.next(),
                }
            }
        }

        match &self.statistics {
            StatisticsKind::Large(raw_bigrams) => IterKind::Large(raw_bigrams.iter_non_zero()),
            StatisticsKind::Medium(raw_bigrams) => IterKind::Medium(raw_bigrams.iter_non_zero()),
            StatisticsKind::Small(raw_bigrams) => IterKind::Small(raw_bigrams.iter_non_zero()),
        }
    }

    /// Adds the given count to the given tuple.
    ///
    /// This function only produces correct results under the assumption that this will not
    /// overflow the underlying storage.
    fn add_fitting_count(&mut self, first: u8, second: u8, count: u64) {
        match &mut self.statistics {
            StatisticsKind::Large(raw_bigrams) => raw_bigrams.add_count(first, second, count),
            StatisticsKind::Medium(raw_bigrams) => {
                raw_bigrams.add_count(first, second, count as u32)
            }
            StatisticsKind::Small(raw_bigrams) => {
                raw_bigrams.add_count(first, second, count as u16)
            }
        }
    }
}

impl fmt::Debug for Statistics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let size = self.window.size();
        f.debug_struct("Statistics")
            .field(
                "kind",
                &format!(
                    "{} ({}B)",
                    match &self.statistics {
                        StatisticsKind::Large(_) => "large",
                        StatisticsKind::Medium(_) => "medium",
                        StatisticsKind::Small(_) => "small",
                    },
                    SizeFormatterBinary::new(size.as_u64())
                ),
            )
            .field("window", &self.window)
            .field("first_byte", &self.first_byte)
            .finish()
    }
}

impl AddAssign<&Statistics> for Statistics {
    #[track_caller]
    fn add_assign(&mut self, rhs: &Statistics) {
        let Some(window) = self.window.joined(rhs.window) else {
            panic!("statistics must be adjacent to be added:\nlhs: {self:?}\nrhs: {rhs:?}",);
        };

        // TODO: This only works under the assumption that the left statistics instance is large
        // enough to fit both. It should be upgraded to a larger variant if that is not the case.

        for (first, second, count) in rhs.iter_non_zero() {
            self.add_fitting_count(first, second, count);
        }

        self.window = window;
    }
}
