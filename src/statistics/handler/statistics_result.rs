//! Implement a different result states of statistics queries.

use std::io;

/// The result of a statistics query.
#[derive(Debug)]
pub enum StatisticsResult<T> {
    /// The value could not be computed because of an error.
    Err(io::Error),
    /// The value is completely unknown.
    Unknown,
    /// There exists an estimate for the value.
    Estimate {
        /// The estimated value.
        value: T,
        /// The quality of the estimate from `0.0` to `1.0`.
        quality: f32,
    },
    /// The exact query result.
    Exact(T),
}

impl<T> StatisticsResult<T> {
    /// Turns the statistics result into a [`Result`] with a quality.
    pub fn into_result_with_quality(self) -> io::Result<Option<(T, f32)>> {
        match self {
            StatisticsResult::Err(err) => Err(err),
            StatisticsResult::Unknown => Ok(None),
            StatisticsResult::Estimate { value, quality } => Ok(Some((value, quality))),
            StatisticsResult::Exact(value) => Ok(Some((value, 1.0))),
        }
    }
}
