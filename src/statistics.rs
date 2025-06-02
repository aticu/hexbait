//! Compute and represent statistics about windows of data.

use std::{
    iter::Sum,
    ops::{Add, AddAssign, Range, SubAssign},
};

use crate::data::DataSource;

/// Computed statistics about a window of data.
struct RawStatistics<Count> {
    /// `follow[b1][b2]` counts how many `b1`s follow a `b2` in the window.
    follow: Box<[[Count; 256]; 256]>,
}

impl<Count> RawStatistics<Count>
where
    Count: Copy + AddAssign<Count> + SubAssign<Count> + From<u8> + Ord,
    u64: Sum<Count> + From<Count>,
{
    /// Computes statistics about a given window of data.
    fn compute<Source: DataSource>(
        source: &mut Source,
        window: Range<u64>,
    ) -> Result<(RawStatistics<Count>, Range<u64>, Option<u8>), Source::Error> {
        let mut follow = Box::new([[Count::from(0u8); 256]; 256]);

        const WINDOW_SIZE: usize = 4096;

        let byte_before_window = if window.start > 0 {
            source
                .window_at(window.start - 1, &mut [0])?
                .first()
                .copied()
        } else {
            None
        };

        const DEFAULT_PREV_BYTE: usize = 0;

        // TODO: this can probably be optimized using SIMD, since this is completely independent of
        // any data but the previous byte (which is only required between subwindows)
        let mut prev_byte = byte_before_window
            .map(|byte| byte as usize)
            .unwrap_or(DEFAULT_PREV_BYTE);
        let mut start = window.start;
        while start < window.end {
            let mut buf = [0; WINDOW_SIZE];
            let max_size = std::cmp::min((window.end - start) as usize, WINDOW_SIZE);

            let subwindow = source.window_at(start, &mut buf[..max_size])?;

            for &byte in subwindow {
                let byte = byte as usize;
                follow[byte][prev_byte] += Count::from(1u8);
                prev_byte = byte;
            }

            start += subwindow.len() as u64;

            if subwindow.is_empty() {
                break;
            }
        }
        // in case the originally given range was larger than the window
        let window_size = start - window.start;

        let first_byte = 'first_byte: {
            if byte_before_window.is_none() {
                // if there is no byte before this window, we initialize `prev_byte`
                if let Some(&first_byte) = source.window_at(window.start, &mut [0])?.first() {
                    follow[first_byte as usize][DEFAULT_PREV_BYTE] -= Count::from(1u8);

                    break 'first_byte Some(first_byte);
                }
            }

            // no need to store the first byte for windows that start later in the file, as they
            // are already accounted for
            None
        };

        Ok((
            RawStatistics { follow },
            window.start..window.start + window_size,
            first_byte,
        ))
    }

    /// Computes the entropy from the collected statistics.
    fn entropy(&self, window: Range<u64>, first_byte: Option<u8>) -> f32 {
        let window_size = (window.end - window.start) as f32;
        -(0..256)
            .map(|i| self.follow[i].into_iter().sum::<u64>() + (first_byte == Some(i as u8)) as u64)
            .filter(|&count| count > 0)
            .map(|count| count as f32 / window_size)
            .map(|p| p * p.log2())
            .sum::<f32>()
            / 8.0
    }

    /// Returns the count of values where `second` follows `first` in the window.
    fn follow(&self, first: u8, second: u8) -> Count {
        self.follow[second as usize][first as usize]
    }

    /// Iterates over all non-zero counts.
    fn iter_non_zero(&self) -> impl Iterator<Item = (usize, usize, Count)> {
        self.follow.iter().enumerate().flat_map(|(second, row)| {
            row.iter()
                .enumerate()
                .map(move |(first, &count)| (first, second, count))
        })
    }
}

/// Computed statistics about a window of data.
enum StatisticsKind {
    /// Statistics over a large window of data (>4GiB).
    Large(RawStatistics<u64>),
}

/// Computed statistics about a window of data.
pub struct Statistics {
    /// The actual statistics.
    statistics: StatisticsKind,
    /// The window over which the statistics were calculated.
    window: Range<u64>,
    /// The first byte in the window, if it is the first window.
    first_byte: Option<u8>,
}

impl Statistics {
    /// Computes statistics about a given window of data.
    pub fn compute<Source: DataSource>(
        source: &mut Source,
        window: Range<u64>,
    ) -> Result<Statistics, Source::Error> {
        RawStatistics::<u64>::compute(source, window).map(|(raw, window, first_byte)| Statistics {
            statistics: StatisticsKind::Large(raw),
            window,
            first_byte,
        })
    }

    /// Computes the entropy from the collected statistics.
    pub fn entropy(&self) -> f32 {
        match &self.statistics {
            StatisticsKind::Large(raw_statistics) => {
                raw_statistics.entropy(self.window.clone(), self.first_byte)
            }
        }
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
        }

        // the mean scaled as a value between 0 and 1
        let mean = sum as f64 / nonzero_count as f64 / max as f64;

        // compute gamma such that the mean will get a middle color
        let gamma = 0.5f64.log2() / mean.log2();

        for first in 0..256usize {
            for second in 0..256usize {
                // scale the number as a value between 0 and 1
                let num = match &self.statistics {
                    StatisticsKind::Large(raw_statistics) => {
                        raw_statistics.follow(first as u8, second as u8)
                    }
                } as f64
                    / max as f64;

                // apply gamma correction
                let scaled_num = num.powf(gamma);

                // save the output
                output[first][second] = (scaled_num * 255.0).round() as u8;
            }
        }

        Signature { values: output }
    }
}

impl Add<&Statistics> for &Statistics {
    type Output = Statistics;

    fn add(self, rhs: &Statistics) -> Self::Output {
        let window = if self.window.end == rhs.window.start {
            self.window.start..rhs.window.end
        } else if rhs.window.end == self.window.start {
            rhs.window.start..self.window.end
        } else {
            panic!("statistics must be adjacent to be added");
        };

        let statistics = match (&self.statistics, &rhs.statistics) {
            (StatisticsKind::Large(this), StatisticsKind::Large(other)) => {
                let mut follow = Box::new([[0; 256]; 256]);

                for (first, second, val) in this.iter_non_zero() {
                    follow[second][first] = val;
                }
                for (first, second, val) in other.iter_non_zero() {
                    follow[second][first] += val;
                }

                StatisticsKind::Large(RawStatistics { follow })
            }
        };

        Statistics {
            statistics,
            window,
            first_byte: self.first_byte.or(rhs.first_byte),
        }
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
