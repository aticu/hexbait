//! Compute and represent statistics about windows of data.

use std::{fmt, ops::AddAssign};

use raw_bigrams::{RawBigrams, SmallRawBigrams};

use crate::{data::DataSource, window::Window};

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
            statistics: StatisticsKind::with_capacity(window.size()),
            window: Window::from_start_len(window.start(), 0),
            first_byte: None,
        }
    }

    /// Computes statistics about a given window of data.
    pub fn compute<Source: DataSource>(
        source: &mut Source,
        window: Window,
    ) -> Result<Statistics, Source::Error> {
        let capacity = window.size();
        let mut statistics = StatisticsKind::with_capacity(capacity);

        let (window, first_byte) = match &mut statistics {
            StatisticsKind::Large(raw_bigrams) => raw_bigrams.compute(source, window)?,
            StatisticsKind::Medium(raw_bigrams) => raw_bigrams.compute(source, window)?,
            StatisticsKind::Small(raw_bigrams) => raw_bigrams.compute(source, window)?,
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
    pub fn add_empty_window(&mut self, window: Window) {
        let Some(window) = self.window.joined(window) else {
            panic!(
                "statistics must be adjacent to be added:\nstatistics: {self:?}\nnew window: {window:?}",
            );
        };

        self.window = window;
    }

    /// Converts the statistics to a signature.
    pub fn to_signature(&self) -> Signature {
        let mut output = Box::new([[0; 256]; 256]);

        // first calculate some statistics
        let mut nonzero_count = 0;
        let mut sum = 0;
        let mut max = 0;
        match &self.statistics {
            StatisticsKind::Large(raw_statistics) => {
                for (_, _, count) in raw_statistics.iter_non_zero() {
                    if count > max {
                        max = count;
                    }
                    nonzero_count += 1;
                    sum += count;
                }
            }
            StatisticsKind::Medium(raw_statistics) => {
                for (_, _, count) in raw_statistics.iter_non_zero() {
                    let count = u64::from(count);
                    if count > max {
                        max = count;
                    }
                    nonzero_count += 1;
                    sum += count;
                }
            }
            StatisticsKind::Small(raw_statistics) => {
                for (_, _, count) in raw_statistics.iter_non_zero() {
                    let count = u64::from(count);
                    if count > max {
                        max = count;
                    }
                    nonzero_count += 1;
                    sum += count;
                }
            }
        }

        // the mean scaled as a value between 0 and 1
        let mean = sum as f64 / nonzero_count as f64 / max as f64;

        // compute gamma such that the mean will get a middle color
        let gamma = 0.5f64.log2() / mean.log2();

        for first in 0..=255 {
            for second in 0..=255 {
                // scale the number as a value between 0 and 1
                let num = match &self.statistics {
                    StatisticsKind::Large(raw_statistics) => raw_statistics.follow(first, second),
                    StatisticsKind::Medium(raw_statistics) => {
                        u64::from(raw_statistics.follow(first, second))
                    }
                    StatisticsKind::Small(raw_statistics) => {
                        u64::from(raw_statistics.follow(first, second))
                    }
                } as f64
                    / max as f64;

                // apply gamma correction
                let scaled_num = num.powf(gamma);

                // save the output
                output[first as usize][second as usize] = (scaled_num * 255.0).round() as u8;
            }
        }

        Signature { values: output }
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
                    size_format::SizeFormatterBinary::new(size)
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

        match (&mut self.statistics, &rhs.statistics) {
            (StatisticsKind::Large(this), StatisticsKind::Large(other)) => {
                for (first, second, val) in other.iter_non_zero() {
                    this.add_count(first, second, val);
                }
            }
            (StatisticsKind::Large(this), StatisticsKind::Medium(other)) => {
                for (first, second, val) in other.iter_non_zero() {
                    this.add_count(first, second, u64::from(val));
                }
            }
            (StatisticsKind::Medium(this), StatisticsKind::Medium(other)) => {
                for (first, second, val) in other.iter_non_zero() {
                    this.add_count(first, second, val);
                }
            }
            (StatisticsKind::Large(this), StatisticsKind::Small(other)) => {
                for (first, second, val) in other.iter_non_zero() {
                    this.add_count(first, second, u64::from(val));
                }
            }
            (StatisticsKind::Medium(this), StatisticsKind::Small(other)) => {
                for (first, second, val) in other.iter_non_zero() {
                    this.add_count(first, second, u32::from(val));
                }
            }
            (StatisticsKind::Small(this), StatisticsKind::Small(other)) => {
                for (first, second, val) in other.iter_non_zero() {
                    this.add_count(first, second, val);
                }
            }
            (StatisticsKind::Medium(_), StatisticsKind::Large(_))
            | (StatisticsKind::Small(_), StatisticsKind::Large(_))
            | (StatisticsKind::Small(_), StatisticsKind::Medium(_)) => {
                unreachable!("trying to add a non-fitting statistic")
            }
        };

        self.window = window;
        self.first_byte = self.first_byte.or(rhs.first_byte);
    }
}

// TODO: document
pub struct Signature {
    values: Box<[[u8; 256]; 256]>,
}

impl Signature {
    pub fn tuple(&self, first: u8, second: u8) -> u8 {
        self.values[first as usize][second as usize]
    }
}
