//! Implements a handler that manages statistics for an input.

use std::{ops::Range, sync::Arc};

use quick_cache::sync::Cache;

use crate::data::DataSource;

use super::Statistics;

#[macro_use]
mod cache_size;

cache_sizes! {
    CacheSize {
        8 KiB with 256 entries,
        512 KiB with 256 entries,
        32 MiB with 256 entries,
        2 GiB with 256 entries,
        128 GiB with 256 entries,
        8 TiB with 256 entries,
        512 TiB with 256 entries,
        32 PiB with 256 entries,
        2 PiB with 256 entries,
        128 PiB with 256 entries,
        8 EiB with 1 entries,
    }
}

/// Manages statistics for an input.
pub struct StatisticsHandler {
    /// Statistics for a 64KiB block in the input.
    caches: [Cache<u64, Arc<Statistics>>; CacheSize::NUM_SIZES],
}

impl StatisticsHandler {
    /// Creates a new statistics handler.
    pub fn new() -> StatisticsHandler {
        StatisticsHandler {
            caches: std::array::from_fn(|i| {
                Cache::new(CacheSize::try_from_index(i).unwrap().num_entries())
            }),
        }
    }

    /// Returns the cached statistics for the given window or computes them.
    fn get_or_compute<Source: DataSource>(
        &mut self,
        source: &mut Source,
        window: Range<u64>,
    ) -> Result<(Arc<Statistics>, bool), Source::Error> {
        let size = CacheSize::try_from(window.end - window.start).expect("not a valid cache size");
        assert_eq!(window.start % size.size(), 0, "unaligned cache request");
        let start = window.start;

        self.caches[size.index()]
            .get(&start)
            .map(|stats| Ok((stats, true)))
            .unwrap_or_else(|| {
                Statistics::compute(source, window).map(|stats| (Arc::new(stats), false))
            })
    }

    /// Returns the cached statistics for the given window.
    ///
    /// Recomputes if necessary.
    fn get_or_cache<Source: DataSource>(
        &mut self,
        source: &mut Source,
        window: Range<u64>,
    ) -> Result<Arc<Statistics>, Source::Error> {
        let (stats, cached) = self.get_or_compute(source, window.clone())?;
        if cached {
            return Ok(stats);
        }
        let size = CacheSize::try_from(window.end - window.start).expect("not a valid cache size");

        self.caches[size.index()].insert(window.start, stats.clone());

        Ok(stats)
    }

    /// Adds a section that is aligned to a cache size to the statistics.
    fn add_aligned_section<Source: DataSource>(
        &mut self,
        stats: &mut Statistics,
        source: &mut Source,
        window: Range<u64>,
        size: CacheSize,
    ) -> Result<(), Source::Error> {
        let size_u64 = size.size();

        let add_window =
            |this: &mut Self, stats: &mut Statistics, source: &mut Source, start, end| {
                for i in 0..(end - start) / size_u64 {
                    let window_stats = this.get_or_cache(
                        source,
                        start + i * size_u64..start + i * size_u64 + size_u64,
                    )?;

                    *stats += &window_stats;
                }

                Ok(())
            };

        if let Some(next_size) = size.next()
            && let next_size_start = next_size.next_start(window.start)
            && let next_size_end = next_size.prev_end(window.end)
            && next_size_start < next_size_end
        {
            add_window(self, stats, source, window.start, next_size_start)?;
            self.add_aligned_section(stats, source, next_size_start..next_size_end, next_size)?;
            add_window(self, stats, source, next_size_end, window.end)?;
        } else {
            add_window(self, stats, source, window.start, window.end)?;
        }

        Ok(())
    }

    /// Returns the statistics
    pub fn get<Source: DataSource>(
        &mut self,
        source: &mut Source,
        window: Range<u64>,
    ) -> Result<Statistics, Source::Error> {
        let len = window.end - window.start;
        let mut output = Statistics::empty_at_with_capacity(window.start, len);

        let first_cached_start = CacheSize::SMALLEST.next_start(window.start);
        let last_cached_end = CacheSize::SMALLEST.prev_end(window.end);

        if first_cached_start > last_cached_end {
            output += &Statistics::compute(source, window.clone())?;
        } else {
            if window.start < first_cached_start {
                output += &Statistics::compute(source, window.start..first_cached_start)?;
            }

            self.add_aligned_section(
                &mut output,
                source,
                first_cached_start..last_cached_end,
                CacheSize::SMALLEST,
            )?;

            if last_cached_end < window.end {
                output += &Statistics::compute(source, last_cached_end..window.end)?;
            }
        }

        assert_eq!(output.window, window);

        Ok(output)
    }
}
