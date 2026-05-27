//! Implements bigram statistics computed on high nibbles instead of full bytes.

use std::{fmt, io, ops::AddAssign};

use hexbait_common::{Input, Len};
use range_set_blaze::RangeSetBlaze;
use size_format::SizeFormatterBinary;

use crate::{
    statistics::{Statistics, StatisticsMetrics},
    window::Window,
};

/// Computed bigram statistics about a window of data, downsampled to high nibbles instead of full bytes.
#[derive(Eq, PartialEq, Clone)]
pub struct DownsampledBigramStatistics {
    /// `follow[b1][b2]` counts how many `b1`s follow a `b2` in the window.
    pub(crate) follow: Box<[[u64; 16]; 16]>,
    /// The regions in the input that the statistics cover.
    pub(crate) contained_regions: RangeSetBlaze<u64>,
}

impl DownsampledBigramStatistics {
    /// Creates empty statistics.
    pub fn empty() -> Self {
        DownsampledBigramStatistics {
            follow: Box::new([[0; 16]; 16]),
            contained_regions: RangeSetBlaze::new(),
        }
    }

    /// Returns the number of bytes that the statistics cover.
    pub fn num_covered_bytes(&self) -> u64 {
        self.contained_regions.len() as u64
    }

    /// The entropy metric.
    ///
    /// This measures how unpredictable the byte distribution is.
    fn entropy(&self) -> u8 {
        let total = self.num_covered_bytes() as f32;

        let raw_entropy = -self
            .follow
            .iter()
            .flat_map(|row| row.iter())
            .filter(|&&count| count != 0)
            .map(|&count| count as f32 / total)
            .map(|p| p * p.log2())
            .sum::<f32>()
            / 8.0;

        (raw_entropy.clamp(0.0, 1.0) * 255.0).round() as u8
    }

    /// The printable ASCII metric.
    ///
    /// This measures how many byte pairs consist of printable ASCII bytes.
    fn printable_ascii(&self) -> u8 {
        let total = self.num_covered_bytes() as f32;

        let mut text_mass = 0.0f32;
        for i in 2..8 {
            for j in 2..8 {
                text_mass += self.follow[i][j] as f32 / total;
            }
        }
        // uniform distribution gives 36/256 ≈ 0.14
        // pure ASCII text gives close to 1.0
        (text_mass.clamp(0.0, 1.0) * 255.0).round() as u8
    }

    /// The byte delta metric.
    ///
    /// This is a measure of how much consecutive bytes differ.
    ///
    /// It is also the opposite of diagonal concentration of bigram distribution.
    fn byte_delta(&self) -> u8 {
        /// The expected diagonal concentration of a uniform distribution.
        const UNIFORM_EXPECTED: f32 = {
            let mut weighted = 0.0f32;
            let mut i = 0;
            while i < 16 {
                let mut j = 0;
                while j < 16 {
                    let p = 1.0 / 256.0;
                    let dist = (i as f32 - j as f32).abs() / 15.0;
                    weighted += (1.0 - dist) * p;
                    j += 1;
                }
                i += 1;
            }
            weighted
        };

        let total = self.num_covered_bytes() as f32;

        let mut weighted = 0.0f32;
        for i in 0..16 {
            for j in 0..16 {
                let p = self.follow[i][j] as f32 / total;
                let dist = (i as f32 - j as f32).abs() / 15.0;
                weighted += (1.0 - dist) * p;
            }
        }

        // rescale: uniform -> 0, maximum (all on diagonal) -> 255
        let diagonal_concentration = (weighted - UNIFORM_EXPECTED) / (1.0 - UNIFORM_EXPECTED);
        let inverted = 1.0 - diagonal_concentration;

        (inverted.clamp(0.0, 1.0) * 255.0).round() as u8
    }

    /// The derived metrics from these statistics.
    pub fn metrics(&self) -> StatisticsMetrics {
        StatisticsMetrics {
            entropy: self.entropy(),
            printable_ascii: self.printable_ascii(),
            byte_delta: self.byte_delta(),
        }
    }
}

impl Statistics for DownsampledBigramStatistics {
    fn empty() -> Self {
        DownsampledBigramStatistics::empty()
    }

    fn approximate_memory_usage(&self) -> u64 {
        std::mem::size_of::<[[u64; 256]; 256]>() as u64
    }

    fn compute(input: &Input, window: Window) -> Result<DownsampledBigramStatistics, io::Error> {
        let mut follow = Box::new([[0; 16]; 16]);

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
                follow[(first >> 4) as usize][(prev_byte >> 4) as usize] += 1;
            }
            for pair in subwindow.windows(2) {
                follow[(pair[1] >> 4) as usize][(pair[0] >> 4) as usize] += 1;
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

        Ok(DownsampledBigramStatistics {
            follow,
            contained_regions,
        })
    }

    fn contained_regions(&self) -> impl IntoIterator<Item = std::ops::RangeInclusive<u64>> {
        self.contained_regions.ranges()
    }
}

impl fmt::Debug for DownsampledBigramStatistics {
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
fn add_statistics_raw(left: &mut [[u64; 16]; 16], right: &[[u64; 16]; 16]) {
    for (lrow, rrow) in left.iter_mut().zip(right.iter()) {
        for (lval, rval) in lrow.iter_mut().zip(rrow.iter()) {
            *lval += *rval;
        }
    }
}

impl AddAssign<&DownsampledBigramStatistics> for DownsampledBigramStatistics {
    #[track_caller]
    fn add_assign(&mut self, rhs: &DownsampledBigramStatistics) {
        if !self.contained_regions.is_disjoint(&rhs.contained_regions) {
            panic!("statistics must not overlap to be added:\nlhs: {self:?}\nrhs: {rhs:?}",);
        }

        add_statistics_raw(&mut self.follow, &rhs.follow);

        for range in rhs.contained_regions.ranges() {
            self.contained_regions.ranges_insert(range);
        }
    }
}
