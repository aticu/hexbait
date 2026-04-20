//! Compute and represent statistics about windows of data.

use std::ops::AddAssign;

mod bigrams;
pub mod classification;
mod downsampled_bigrams;
mod handler;

pub use bigrams::BigramStatistics;
pub use handler::{MetricsQuality, StatisticsHandler};

/// A shared trait between different statistics measures.
trait Statistics: for<'a> AddAssign<&'a Self> {
    /// Creates empty statistics.
    fn empty() -> Self;

    /// Returns an approximation of the number of bytes needed to store this statistics value.
    fn approximate_memory_usage(&self) -> u64;
}

/// Metrics computed on downsampled bigram statistics.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub struct StatisticsMetrics {
    /// The entropy of the data.
    ///
    /// This measures how unpredictable the data is.
    pub entropy: u8,
    /// The fraction of printable ASCII bigrams in the data.
    ///
    /// This measures how many byte pairs consist of printable ASCII bytes.
    pub printable_ascii: u8,
    /// The delta of consecutive bytes.
    ///
    /// This is a measure of how much consecutive bytes differ.
    pub byte_delta: u8,
}

impl StatisticsMetrics {
    /// Returns empty metrics that can be used a placeholders where a value is needed.
    fn empty() -> StatisticsMetrics {
        StatisticsMetrics {
            printable_ascii: 0,
            entropy: 0,
            byte_delta: 0,
        }
    }

    /// Returns the average metrics from the given metrics.
    ///
    /// Returns `None` when `metrics` is empty.
    fn from_average(metrics: &[StatisticsMetrics]) -> Option<StatisticsMetrics> {
        if metrics.is_empty() {
            return None;
        }

        let mut total_printable_ascii = 0;
        let mut total_entropy = 0;
        let mut total_byte_delta = 0;

        for metric in metrics {
            total_entropy += metric.entropy as u64;
            total_printable_ascii += metric.printable_ascii as u64;
            total_byte_delta += metric.byte_delta as u64;
        }

        let n = metrics.len() as u64;
        Some(StatisticsMetrics {
            entropy: (total_entropy / n) as u8,
            printable_ascii: (total_printable_ascii / n) as u8,
            byte_delta: (total_byte_delta / n) as u8,
        })
    }
}
