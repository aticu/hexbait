//! Implements bigram statistics.

use std::{fmt, io, ops::AddAssign};

use hexbait_common::{Input, Len};
use range_set_blaze::RangeSetBlaze;
use size_format::SizeFormatterBinary;

use crate::{
    statistics::{Statistics, downsampled_bigrams::DownsampledBigramStatistics},
    window::Window,
};

/// Computed bigram statistics about a window of data.
#[derive(Eq, PartialEq, Clone)]
pub struct BigramStatistics {
    /// `follow[b1][b2]` counts how many `b1`s follow a `b2` in the window.
    follow: Box<[[u64; 256]; 256]>,
    /// The regions in the input that the statistics cover.
    contained_regions: RangeSetBlaze<u64>,
}

impl BigramStatistics {
    /// Creates empty statistics.
    pub fn empty() -> Self {
        BigramStatistics {
            follow: Box::new([[0; 256]; 256]),
            contained_regions: RangeSetBlaze::new(),
        }
    }

    /// Returns the number of times that `first` is followed by `second` in the statistics.
    pub fn follow(&self, first: u8, second: u8) -> u64 {
        self.follow[second as usize][first as usize]
    }

    /// Iterates over the statistics.
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = (u8, u8, u64)> {
        self.follow.iter().enumerate().flat_map(|(second, row)| {
            row.iter()
                .enumerate()
                .map(move |(first, count)| (first as u8, second as u8, *count))
        })
    }

    /// Returns the number of bytes that the statistics cover.
    pub fn num_covered_bytes(&self) -> u64 {
        self.contained_regions.len() as u64
    }

    /// Returns `true` if the given window is fully covered by the statistics.
    pub fn fully_contains(&self, window: Window) -> bool {
        if window.is_empty() {
            return true;
        }

        let mut window_region = RangeSetBlaze::new();
        window_region.ranges_insert(window.start().as_u64()..=window.end().as_u64() - 1);

        self.contained_regions.is_superset(&window_region)
    }

    /// Returns the downsampled statistics.
    pub fn downsampled(&self) -> DownsampledBigramStatistics {
        let mut follow = Box::new([[0; 16]; 16]);

        for x in 0..256 {
            for y in 0..256 {
                follow[x >> 4][y >> 4] += self.follow[x][y];
            }
        }

        DownsampledBigramStatistics {
            follow,
            contained_regions: self.contained_regions.clone(),
        }
    }
}

impl Statistics for BigramStatistics {
    fn empty() -> Self {
        BigramStatistics::empty()
    }

    fn approximate_memory_usage(&self) -> u64 {
        std::mem::size_of::<[[u64; 256]; 256]>() as u64
    }

    fn compute(input: &Input, window: Window) -> Result<BigramStatistics, io::Error> {
        let mut follow = Box::new([[0; 256]; 256]);

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
                follow[first as usize][prev_byte as usize] += 1;
            }
            for pair in subwindow.windows(2) {
                follow[pair[1] as usize][pair[0] as usize] += 1;
            }
            prev_byte = subwindow.last().copied().unwrap_or(prev_byte);

            start += Len::from(subwindow.len() as u64);

            if subwindow.is_empty() {
                break;
            }
        }
        // in case the originally given range was larger than the window
        let window_size = start - window.start();

        let window = Window::from_start_len(window.start(), window_size);
        let mut contained_regions = RangeSetBlaze::new();
        contained_regions.ranges_insert(window.start().as_u64()..=window.end().as_u64() - 1);

        Ok(BigramStatistics {
            follow,
            contained_regions,
        })
    }

    fn contained_regions(&self) -> impl IntoIterator<Item = std::ops::RangeInclusive<u64>> {
        self.contained_regions.ranges()
    }
}

impl fmt::Debug for BigramStatistics {
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

impl AddAssign<&BigramStatistics> for BigramStatistics {
    #[track_caller]
    fn add_assign(&mut self, rhs: &BigramStatistics) {
        if !self.contained_regions.is_disjoint(&rhs.contained_regions) {
            panic!("statistics must not overlap to be added:\nlhs: {self:?}\nrhs: {rhs:?}",);
        }

        add_statistics_raw(&mut self.follow, &rhs.follow);

        for range in rhs.contained_regions.ranges() {
            self.contained_regions.ranges_insert(range);
        }
    }
}
