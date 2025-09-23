//! Implements statistics that count the frequencies of different byte values.

use std::{
    fmt,
    iter::Sum,
    ops::{AddAssign, SubAssign},
};

use crate::{
    data::DataSource,
    statistics::{Statistics, StatisticsKind},
    window::Window,
};

/// Computed flat statistics about a window of data.
#[derive(Eq, PartialEq)]
pub struct FlatStatistics {
    /// The actual statistics.
    statistics: FlatStatisticsKind,
    /// The window over which the statistics were calculated.
    window: Window,
}

impl FlatStatistics {
    /// Creates new empty statistics starting at the beginning of the given window.
    ///
    /// The capacity of the window is chosen such that it will fit the whole given window.
    pub fn empty_for_window(window: Window) -> FlatStatistics {
        FlatStatistics {
            statistics: FlatStatisticsKind::with_capacity(window.size()),
            window: Window::from_start_len(window.start(), 0),
        }
    }

    /// Computes statistics about a given window of data.
    pub fn compute<Source: DataSource>(
        source: &mut Source,
        window: Window,
    ) -> Result<FlatStatistics, Source::Error> {
        let capacity = window.size();
        let mut statistics = FlatStatisticsKind::with_capacity(capacity);

        let window = match &mut statistics {
            FlatStatisticsKind::Large(raw_stats) => raw_stats.compute(source, window)?,
            FlatStatisticsKind::Medium(raw_stats) => raw_stats.compute(source, window)?,
            FlatStatisticsKind::Small(raw_stats) => raw_stats.compute(source, window)?,
        };

        Ok(FlatStatistics { statistics, window })
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

    /// Computes the entropy over the given counts.
    pub fn entropy(&self) -> f32 {
        let total_count = self.statistics.iter_counts_u64().sum::<u64>() as f32;

        -self
            .statistics
            .iter_counts_u64()
            .filter(|&count| count != 0)
            .map(|count| {
                let p = count as f32 / total_count;
                p * p.log2()
            })
            .sum::<f32>()
            / 8.0
    }

    /// Converts the given bigram statistics to flat statistics.
    pub fn from_bigrams(bigrams: &Statistics) -> FlatStatistics {
        let mut flat = FlatStatistics::empty_for_window(bigrams.window);

        flat.window = bigrams.window;

        for byte_value in 0..=255 {
            let mut count = 0;

            for prev_byte_value in 0..=255 {
                count += match &bigrams.statistics {
                    StatisticsKind::Large(raw_statistics) => {
                        raw_statistics.follow(prev_byte_value, byte_value)
                    }
                    StatisticsKind::Medium(raw_statistics) => {
                        u64::from(raw_statistics.follow(prev_byte_value, byte_value))
                    }
                    StatisticsKind::Small(raw_statistics) => {
                        u64::from(raw_statistics.follow(prev_byte_value, byte_value))
                    }
                };
            }

            let byte_value = byte_value as usize;
            match &mut flat.statistics {
                FlatStatisticsKind::Large(stats) => stats.counts[byte_value] += count,
                FlatStatisticsKind::Medium(stats) => {
                    stats.counts[byte_value] +=
                        u32::try_from(count).expect("window must be appropriately sized")
                }
                FlatStatisticsKind::Small(stats) => {
                    stats.counts[byte_value] +=
                        u16::try_from(count).expect("window must be appropriately sized")
                }
            }
        }

        if let Some(byte_value) = bigrams.first_byte {
            let byte_value = byte_value as usize;
            match &mut flat.statistics {
                FlatStatisticsKind::Large(stats) => stats.counts[byte_value] += 1,
                FlatStatisticsKind::Medium(stats) => stats.counts[byte_value] += 1,
                FlatStatisticsKind::Small(stats) => stats.counts[byte_value] += 1,
            }
        }

        flat
    }
}

impl fmt::Debug for FlatStatistics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let size = self.window.size();
        f.debug_struct("Statistics")
            .field(
                "kind",
                &format!(
                    "{} ({}B)",
                    match &self.statistics {
                        FlatStatisticsKind::Large(_) => "large",
                        FlatStatisticsKind::Medium(_) => "medium",
                        FlatStatisticsKind::Small(_) => "small",
                    },
                    size_format::SizeFormatterBinary::new(size)
                ),
            )
            .field("window", &self.window)
            .finish()
    }
}

impl AddAssign<&FlatStatistics> for FlatStatistics {
    #[track_caller]
    fn add_assign(&mut self, rhs: &FlatStatistics) {
        let Some(window) = self.window.joined(rhs.window) else {
            panic!("statistics must be adjacent to be added:\nlhs: {self:?}\nrhs: {rhs:?}",);
        };

        // TODO: This only works under the assumption that the left statistics instance is large
        // enough to fit both. It should be upgraded to a larger variant if that is not the case.

        match (&mut self.statistics, &rhs.statistics) {
            (FlatStatisticsKind::Large(this), FlatStatisticsKind::Large(other)) => {
                for (this_count, other_count) in this.counts.iter_mut().zip(other.counts.iter()) {
                    *this_count += other_count;
                }
            }
            (FlatStatisticsKind::Large(this), FlatStatisticsKind::Medium(other)) => {
                for (this_count, &other_count) in this.counts.iter_mut().zip(other.counts.iter()) {
                    *this_count += u64::from(other_count);
                }
            }
            (FlatStatisticsKind::Medium(this), FlatStatisticsKind::Medium(other)) => {
                for (this_count, other_count) in this.counts.iter_mut().zip(other.counts.iter()) {
                    *this_count += other_count;
                }
            }
            (FlatStatisticsKind::Large(this), FlatStatisticsKind::Small(other)) => {
                for (this_count, &other_count) in this.counts.iter_mut().zip(other.counts.iter()) {
                    *this_count += u64::from(other_count);
                }
            }
            (FlatStatisticsKind::Medium(this), FlatStatisticsKind::Small(other)) => {
                for (this_count, &other_count) in this.counts.iter_mut().zip(other.counts.iter()) {
                    *this_count += u32::from(other_count);
                }
            }
            (FlatStatisticsKind::Small(this), FlatStatisticsKind::Small(other)) => {
                for (this_count, other_count) in this.counts.iter_mut().zip(other.counts.iter()) {
                    *this_count += other_count;
                }
            }
            (FlatStatisticsKind::Medium(_), FlatStatisticsKind::Large(_))
            | (FlatStatisticsKind::Small(_), FlatStatisticsKind::Large(_))
            | (FlatStatisticsKind::Small(_), FlatStatisticsKind::Medium(_)) => {
                unreachable!("trying to add a non-fitting statistic")
            }
        };

        self.window = window;
    }
}

/// Computed flat statistics about a window of data.
#[derive(Eq, PartialEq)]
enum FlatStatisticsKind {
    /// Statistics over a small window of data (<64KiB).
    Small(RawFlatStatistics<u16>),
    /// Statistics over a medium window of data (64KiB to 4GiB).
    Medium(RawFlatStatistics<u32>),
    /// Statistics over a large window of data (>4GiB).
    Large(RawFlatStatistics<u64>),
}

impl FlatStatisticsKind {
    /// Allocates an appropriate statistics kind for the given window size.
    fn with_capacity(capacity: u64) -> FlatStatisticsKind {
        if capacity > u64::from(u32::MAX) {
            FlatStatisticsKind::Large(RawFlatStatistics::empty())
        } else if capacity > u64::from(u16::MAX) {
            FlatStatisticsKind::Medium(RawFlatStatistics::empty())
        } else {
            FlatStatisticsKind::Small(RawFlatStatistics::empty())
        }
    }

    /// Iterates over all counts as `u64`s.
    fn iter_counts_u64(&self) -> impl Iterator<Item = u64> {
        let fake_u16 = &[0u16; 0][..];
        let fake_u32 = &[0u32; 0][..];
        let fake_u64 = &[0u64; 0][..];

        let (arr_u16, arr_u32, arr_u64) = match self {
            FlatStatisticsKind::Small(raw_flat_statistics) => {
                (&raw_flat_statistics.counts[..], fake_u32, fake_u64)
            }
            FlatStatisticsKind::Medium(raw_flat_statistics) => {
                (fake_u16, &raw_flat_statistics.counts[..], fake_u64)
            }
            FlatStatisticsKind::Large(raw_flat_statistics) => {
                (fake_u16, fake_u32, &raw_flat_statistics.counts[..])
            }
        };

        let iter_u16 = arr_u16.iter().copied().map(u64::from);
        let iter_u32 = arr_u32.iter().copied().map(u64::from);
        let iter_u64 = arr_u64.iter().copied();

        iter_u16.chain(iter_u32).chain(iter_u64)
    }
}

/// Contains the raw counts over a window of data.
#[derive(Eq, PartialEq)]
struct RawFlatStatistics<Count> {
    /// The counts for each byte frequency.
    counts: Box<[Count; 256]>,
}

impl<Count> RawFlatStatistics<Count>
where
    Count: Copy + AddAssign<Count> + SubAssign<Count> + From<u8> + Ord + Sum<Count>,
    u64: From<Count>,
{
    /// Creates new empty statistics.
    fn empty() -> RawFlatStatistics<Count> {
        RawFlatStatistics {
            counts: Box::new([Count::from(0u8); 256]),
        }
    }

    /// Computes raw statistics for the given window.
    fn compute<Source: DataSource>(
        &mut self,
        source: &mut Source,
        window: Window,
    ) -> Result<Window, Source::Error> {
        const WINDOW_SIZE: usize = 4096;

        // TODO: this can probably be optimized using SIMD, since this is completely independent of
        // any data but the previous byte (which is only required between subwindows)
        let mut start = window.start();
        while start < window.end() {
            let mut buf = [0; WINDOW_SIZE];
            let max_size = std::cmp::min((window.end() - start) as usize, WINDOW_SIZE);

            let subwindow = source.window_at(start, &mut buf[..max_size])?;

            for &byte in subwindow {
                self.counts[byte as usize] += Count::from(1u8);
            }

            start += subwindow.len() as u64;

            if subwindow.is_empty() {
                break;
            }
        }
        // in case the originally given range was larger than the window
        let window_size = start - window.start();

        Ok(Window::from_start_len(window.start(), window_size))
    }
}
