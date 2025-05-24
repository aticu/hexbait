//! Compute and represent statistics about windows of data.

use crate::data::DataSource;

/// Computed statistics about a window of data.
pub struct Statistics {
    /// `follows[b1][b2]` counts how many `b2`s followed a `b1` in the `window`.
    follows: Box<[[u64; 256]; 256]>,
}

impl Statistics {
    /// Computes statistics about a given window of data.
    pub fn compute<Source: DataSource>(
        source: &mut Source,
        window: std::ops::Range<u64>,
    ) -> Result<Statistics, Source::Error> {
        let mut follows = Box::new([[0u64; 256]; 256]);

        // TODO: this can probably be optimized using SIMD, since this is completely independent of
        // any data but the previous byte (which is only required between subwindows)
        let mut prev_byte = 0;
        let mut start = window.start;
        while start < window.end {
            let mut buf = [0; 4096];

            let subwindow = source.window_at(start, &mut buf)?;

            for &byte in subwindow {
                let byte = byte as usize;
                follows[prev_byte][byte] += 1;
                prev_byte = byte;
            }

            start += subwindow.len() as u64;

            if subwindow.is_empty() {
                break;
            }
        }

        if let Some(&first_byte) = source.window_at(window.start, &mut [0; 1])?.first() {
            let first_byte = first_byte as usize;
            follows[0][first_byte] = follows[0][first_byte].saturating_sub(1);
        }

        Ok(Statistics { follows })
    }

    /// Converts the statistics to a signature which looses information, but is more efficient.
    pub fn to_signature(&self) -> Signature {
        let mut output = Box::new([[0; 256]; 256]);

        // first calculate some statistics
        let mut nonzero_count = 0;
        let mut sum = 0;
        let mut max = 0;
        for row in self.follows.iter() {
            for &val in row {
                if val > max {
                    max = val;
                }
                if val != 0 {
                    nonzero_count += 1;
                }
                sum += val;
            }
        }

        // the mean scaled as a value between 0 and 1
        let mean = sum as f64 / nonzero_count as f64 / max as f64;

        // compute gamma such that the mean will get a middle color
        let gamma = 0.5f64.log2() / mean.log2();

        for first in 0..256 {
            for second in 0..256 {
                // scale the number as a value between 0 and 1
                let num = self.follows[first][second] as f64 / max as f64;

                // apply gamma correction
                let scaled_num = num.powf(gamma);

                // save the output
                output[first][second] = (scaled_num * 255.0).round() as u8;
            }
        }

        Signature { values: output }
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
