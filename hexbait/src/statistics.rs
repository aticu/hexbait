//! Compute and represent statistics about windows of data.

use std::{fmt, io, ops::AddAssign};

use hexbait_common::{AbsoluteOffset, Input};
use range_set_blaze::RangeSetBlaze;
use raw_bigrams::RawBigrams;
use size_format::SizeFormatterBinary;

use crate::window::Window;

pub mod classification;
mod handler;
mod raw_bigrams;

pub use handler::{EntropyQuality, StatisticsHandler};

/// An entropy estimate quantized to a u8.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash)]
pub struct Entropy(pub u8);

impl Entropy {
    /// Returns the entropy as value between `0.0` and `1.0`.
    pub fn as_f32(self) -> f32 {
        self.0 as f32 / 255.0
    }
}

impl fmt::Display for Entropy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.2}", self.as_f32())
    }
}

/// Computed statistics about a window of data.
#[derive(Eq, PartialEq, Clone)]
pub struct Statistics {
    /// The actual statistics.
    statistics: RawBigrams,
    /// The regions in the input that the statistics cover.
    contained_regions: RangeSetBlaze<u64>,
}

impl Statistics {
    /// Creates new empty statistics.
    pub fn empty() -> Statistics {
        Statistics {
            statistics: RawBigrams::empty(),
            contained_regions: RangeSetBlaze::new(),
        }
    }

    /// Computes statistics about a given window of data.
    pub fn compute(input: &Input, window: Window) -> Result<Statistics, io::Error> {
        let mut statistics = RawBigrams::empty();

        let window = statistics.compute(input, window)?;

        let mut contained_regions = RangeSetBlaze::new();
        contained_regions.ranges_insert(window.start().as_u64()..=window.end().as_u64() - 1);

        Ok(Statistics {
            statistics,
            contained_regions,
        })
    }

    /// Computes the marginal entropy from the bigram distribution.
    pub fn entropy(&self) -> Entropy {
        #[multiversion::multiversion(targets(
            "x86_64+avx512f+avx512bw",
            "x86_64+avx2",
            "x86_64+avx",
            "x86_64+sse4.1",
        ))]
        fn flat_counts(statistics: &[[u64; 256]; 256]) -> [u64; 256] {
            let mut counts = [0u64; 256];

            for (count, row) in counts.iter_mut().zip(statistics.iter()) {
                *count = row.iter().sum();
            }

            counts
        }

        let counts = flat_counts(self.statistics.raw_counts());

        let total: u64 = counts.iter().sum();
        if total == 0 {
            return Entropy(0);
        }

        let total = total as f32;
        let sum: f32 = counts
            .iter()
            .filter(|&&count| count != 0)
            .map(|&count| {
                let p = count as f32 / total;
                p * p.log2()
            })
            .sum();

        let entropy_01 = -sum / 8.0;

        Entropy((entropy_01.clamp(0.0, 1.0) * 255.0).round() as u8)
    }

    /// Returns an approximation of the number of bytes needed to store this statistics value.
    fn approximate_memory_usage(&self) -> u64 {
        std::mem::size_of::<[[u64; 256]; 256]>() as u64
    }

    /// Returns the number of times that `first` is followed by `second` in the statistics.
    pub fn follow(&self, first: u8, second: u8) -> u64 {
        self.statistics.follow(first, second)
    }

    /// Iterates over the statistics.
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = (u8, u8, u64)> {
        self.statistics
            .raw_counts()
            .iter()
            .enumerate()
            .flat_map(|(second, row)| {
                row.iter()
                    .enumerate()
                    .map(move |(first, count)| (first as u8, second as u8, *count))
            })
    }

    /// Returns the number of bytes that the statistics cover.
    fn num_covered_bytes(&self) -> u64 {
        self.contained_regions.len() as u64
    }

    /// Returns `true` if the given window is fully covered by the statistics.
    fn fully_contains(&self, window: Window) -> bool {
        if window.is_empty() {
            return true;
        }

        let mut window_region = RangeSetBlaze::new();
        window_region.ranges_insert(window.start().as_u64()..=window.end().as_u64() - 1);

        self.contained_regions.is_superset(&window_region)
    }

    /// Returns the first section in the given window that is not covered by the statistics.
    fn first_uncovered_section_in_window(&self, window: Window) -> Option<Window> {
        if window.is_empty() {
            return None;
        }

        let mut cursor = window.start();

        for range in self.contained_regions.ranges() {
            let range_start = AbsoluteOffset::from(*range.start());
            let range_end = AbsoluteOffset::from(*range.end() + 1); // exclusive end

            if range_end <= cursor {
                continue;
            }

            if range_start > cursor {
                return Some(Window::new(cursor, range_start.min(window.end())));
            }

            cursor = range_end;

            if cursor >= window.end() {
                return None;
            }
        }

        if cursor < window.end() {
            Some(Window::new(cursor, window.end()))
        } else {
            None
        }
    }
}

impl fmt::Debug for Statistics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Statistics")
            .field(
                "size",
                &format!("{}B", SizeFormatterBinary::new(self.num_covered_bytes())),
            )
            .field("contained_regions", &self.contained_regions)
            .finish()
    }
}

/// Adds two raw statistics together.
#[multiversion::multiversion(targets(
    "x86_64+avx512f+avx512bw",
    "x86_64+avx2",
    "x86_64+avx",
    "x86_64+sse4.1",
))]
fn add_statistics_raw(left: &mut [[u64; 256]; 256], right: &[[u64; 256]; 256]) {
    for (lrow, rrow) in left.iter_mut().zip(right.iter()) {
        for (lval, rval) in lrow.iter_mut().zip(rrow.iter()) {
            *lval += *rval;
        }
    }
}

impl AddAssign<&Statistics> for Statistics {
    #[track_caller]
    fn add_assign(&mut self, rhs: &Statistics) {
        if !self.contained_regions.is_disjoint(&rhs.contained_regions) {
            panic!("statistics must not overlap to be added:\nlhs: {self:?}\nrhs: {rhs:?}",);
        }

        add_statistics_raw(
            self.statistics.raw_counts_mut(),
            rhs.statistics.raw_counts(),
        );

        for range in rhs.contained_regions.ranges() {
            self.contained_regions.ranges_insert(range);
        }
    }
}
