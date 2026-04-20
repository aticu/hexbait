//! Implements classification of byte statistics.
//!
//! Classification works by computing the [Hellinger distance](https://en.wikipedia.org/wiki/Hellinger_distance)
//! between the probability distributions that statistics represent.
//! To do this the statistics are first smoothed (depending on the size of window the statistics
//! covers the amount differs).
//! Then the resulting data is converted into a probability distribution with `256 * 256` events
//! each corresponding to one of the possible bigrams.
//! Then the square root of the probabilities is taken as required by Hellinger and these values
//! are quantized to a `u16` where `0` means `0.0` and `u16::MAX` means `1.0`.
//! The calculation is done on these quantized values and the true final value can be extracted by
//! dividing the result by `(u16::MAX as u32 * u16::MAX as u32) as f32`.
//! Performing the computation on integers makes the whole computation a lot faster.
//!
//! Once the final score is computed, it is compared to corresponding thresholds that depend on
//! the size of the window the statistics cover.
//! This ensures that both smaller windows and larger windows can be accurately classified.
//! The thresholds cover the 90th percentile of scores of sub-statistics of the given size in the
//! sample file.

use std::io;

use hexbait_common::{AbsoluteOffset, Input, Len};

use crate::{state::State, statistics::BigramStatistics, window::Window};

mod builtin;

/// The size of a KiB in bytes.
const KIB: u64 = 1024;

/// The size of a MiB in bytes.
const MIB: u64 = KIB * KIB;

/// Represents a class of bytes determined by classification.
#[derive(Debug)]
pub struct Class {
    /// The name of the class.
    pub name: &'static str,
    /// The score that the class had.
    pub score: f32,
    /// The minimum score needed to fit to this class.
    pub min_score: f32,
}

/// The data required for the classification of a single class.
pub struct ClassificationData {
    /// The quantized Hellinger distance statistics.
    ///
    /// This corresponds to the square root of the smoothed statistics.
    /// A value of `0` corresponds to the float `0.0` and a value of `u16::MAX` corresponds to the
    /// float `1.0` with values in between spaced linearly.
    pub quantized_hellinger_statistics: [[u16; 256]; 256],
    /// The minimum score to match this class with a 32KiB sample.
    pub min_score_32kib: f32,
    /// The minimum score to match this class with a 128KiB sample.
    pub min_score_128kib: f32,
    /// The minimum score to match this class with a 1MiB sample.
    pub min_score_1mib: f32,
    /// The minimum score to match this class with a 8MiB sample.
    pub min_score_8mib: f32,
}

impl ClassificationData {
    /// Displays this value as a Rust source code literal.
    pub fn as_rust_src_str(&self) -> String {
        let mut out = String::new();
        out.push_str("ClassificationData {\n");

        out.push_str("    quantized_hellinger_statistics: [\n");
        for row in self.quantized_hellinger_statistics {
            out.push_str("        [");
            for (i, value) in row.iter().enumerate() {
                if i != 0 {
                    out.push(',');
                }
                out.push_str(&format!("{value}"));
            }
            out.push_str("],\n");
        }
        out.push_str("    ],\n");

        out.push_str(&format!(
            "    min_score_32kib: {:.09},\n",
            self.min_score_32kib
        ));
        out.push_str(&format!(
            "    min_score_128kib: {:.09},\n",
            self.min_score_128kib
        ));
        out.push_str(&format!(
            "    min_score_1mib: {:.09},\n",
            self.min_score_1mib
        ));
        out.push_str(&format!(
            "    min_score_8mib: {:.09},\n",
            self.min_score_8mib
        ));

        out.push('}');

        out
    }
}

/// Classifies the currently selected window.
pub fn classify_selected_window(state: &mut State) {
    let (statistics, quality) = state
        .statistics_handler
        .get_bigram_statistics(state.scroll_state.selected_window());
    if quality == 0.0 {
        return;
    }

    let mut quantized_statistics = Box::new([[0; 256]; 256]);
    quantized_hellinger_statistics(&statistics, &mut quantized_statistics);

    let total_count = statistics.num_covered_bytes();
    if total_count == 0 {
        state.classification_state.classification_results = None;
        return;
    }

    let mut result = Vec::new();

    for (name, classification_data) in builtin::CLASSIFICATION_DATA.iter() {
        let score = compute_score(
            &quantized_statistics,
            &classification_data.quantized_hellinger_statistics,
        );

        let (lower_bound, upper_bound, lower_score, upper_score) = if total_count < 32 * KIB {
            (0, 32 * KIB, 0.0, classification_data.min_score_32kib)
        } else if total_count < 128 * KIB {
            (
                32 * KIB,
                128 * KIB,
                classification_data.min_score_32kib,
                classification_data.min_score_128kib,
            )
        } else if total_count < MIB {
            (
                128 * KIB,
                MIB,
                classification_data.min_score_128kib,
                classification_data.min_score_1mib,
            )
        } else if total_count < 8 * MIB {
            (
                MIB,
                8 * MIB,
                classification_data.min_score_1mib,
                classification_data.min_score_8mib,
            )
        } else {
            (
                8 * MIB,
                u64::MAX,
                classification_data.min_score_8mib,
                classification_data.min_score_8mib,
            )
        };

        let score_diff = upper_score - lower_score;
        let min_score = lower_score
            + score_diff * (total_count - lower_bound) as f32 / (upper_bound - lower_bound) as f32;

        result.push(Class {
            name,
            score,
            min_score,
        });
    }

    result.sort_by(|c1, c2| {
        c1.score
            .partial_cmp(&c2.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .reverse()
    });

    state.classification_state.classification_results = Some(result);
}

/// Computes the quantized statistics ready for a Hellinger comparison.
fn quantized_hellinger_statistics(
    statistics: &BigramStatistics,
    out: &mut [[u16; 256]; 256],
) -> u64 {
    let total_count = statistics.num_covered_bytes();

    let beta = if total_count <= 32 * KIB {
        512.0
    } else if total_count < 128 * KIB {
        1024.0
    } else if total_count < MIB {
        2048.0
    } else {
        4096.0
    };

    let lambda = beta / (total_count as f32 + beta);

    for first in 0..=255 {
        for second in 0..=255 {
            let val = (statistics.follow(first, second) as f32 / total_count as f32)
                * (1.0 - lambda)
                + lambda / (256.0 * 256.0);

            out[first as usize][second as usize] = (val.sqrt() * u16::MAX as f32).round() as u16;
        }
    }

    total_count
}

/// Compute the classification data from a given input.
pub fn compute_classification_data(input: Input) -> io::Result<ClassificationData> {
    let full_window = Window::from_start_len(AbsoluteOffset::ZERO, input.len());
    let statistics = BigramStatistics::compute(&input, full_window)?;

    let mut out = ClassificationData {
        quantized_hellinger_statistics: [[0; 256]; 256],
        min_score_32kib: 1.0,
        min_score_128kib: 1.0,
        min_score_1mib: 1.0,
        min_score_8mib: 1.0,
    };

    quantized_hellinger_statistics(&statistics, &mut out.quantized_hellinger_statistics);

    let mut substats = Box::new([[0; 256]; 256]);

    let mut compute_min_score_for_size = |size| -> io::Result<f32> {
        let mut scores = Vec::new();

        for window in Window::from_start_len(AbsoluteOffset::ZERO, input.len().align_down(size))
            .subwindows_of_size(Len::from(size))
        {
            let substatistics = BigramStatistics::compute(&input, window)?;
            quantized_hellinger_statistics(&substatistics, &mut substats);

            let score = compute_score(&out.quantized_hellinger_statistics, &substats);
            scores.push(score);
        }

        scores.sort_by(f32::total_cmp);

        // select the 5th percentile score, in case the sample data is messy
        Ok(scores[((scores.len() - 1) as f32 * 0.95).round() as usize])
    };

    out.min_score_32kib = compute_min_score_for_size(32 * KIB)?;
    out.min_score_128kib = compute_min_score_for_size(128 * KIB)?;
    out.min_score_1mib = compute_min_score_for_size(MIB)?;
    out.min_score_8mib = compute_min_score_for_size(8 * MIB)?;

    Ok(out)
}

/// Computes the score for the given statistics using the given classification data.
fn compute_score(
    quantized_statistics: &[[u16; 256]; 256],
    classification_data: &[[u16; 256]; 256],
) -> f32 {
    let mut score = 0u64;

    for first in 0..=255 {
        for second in 0..=255 {
            let q = classification_data[first][second];
            let p = quantized_statistics[first][second];

            score += p as u64 * q as u64;
        }
    }

    score as f32 / (65535.0 * 65535.0)
}
