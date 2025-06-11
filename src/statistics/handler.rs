//! Implements a handler that manages statistics for an input.

use std::sync::Arc;

use quick_cache::sync::Cache;

use crate::{data::DataSource, window::Window};

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
        &self,
        source: &mut Source,
        window: Window,
    ) -> Result<(Arc<Statistics>, bool), Source::Error> {
        let size = CacheSize::try_from(window.size()).expect("not a valid cache size");
        assert_eq!(window.start() % size.size(), 0, "unaligned cache request");

        self.caches[size.index()]
            .get(&window.start())
            .map(|stats| Ok((stats, true)))
            .unwrap_or_else(|| {
                Statistics::compute(source, window).map(|stats| (Arc::new(stats), false))
            })
    }

    /// Returns the cached statistics for the given window.
    ///
    /// Recomputes if necessary.
    fn get_or_cache<Source: DataSource>(
        &self,
        source: &mut Source,
        window: Window,
    ) -> Result<Arc<Statistics>, Source::Error> {
        let (stats, cached) = self.get_or_compute(source, window)?;
        if cached {
            return Ok(stats);
        }
        let size = CacheSize::try_from(window.size()).expect("not a valid cache size");

        self.caches[size.index()].insert(window.start(), stats.clone());

        Ok(stats)
    }

    /// Adds a section that is aligned to a cache size to the statistics.
    fn add_aligned_section<Source: DataSource>(
        &self,
        stats: &mut Statistics,
        source: &mut Source,
        window: Window,
        size: CacheSize,
    ) -> Result<(), Source::Error> {
        let size_u64 = size.size();

        let add_window =
            |this: &Self, stats: &mut Statistics, source: &mut Source, window: Window| {
                for i in 0..window.size() / size_u64 {
                    let window_stats = this.get_or_cache(
                        source,
                        Window::from_start_len(window.start() + i * size_u64, size_u64),
                    )?;

                    *stats += &window_stats;
                }

                Ok(())
            };

        if let Some(next_size) = size.next()
            && let Some((before, aligned, after)) = window.align(next_size.size())
        {
            add_window(self, stats, source, before)?;
            self.add_aligned_section(stats, source, aligned, next_size)?;
            add_window(self, stats, source, after)?;
        } else {
            add_window(self, stats, source, window)?;
        }

        Ok(())
    }

    /// Returns the statistics associated with the given window.
    pub fn get<Source: DataSource>(
        &self,
        source: &mut Source,
        window: Window,
    ) -> Result<Statistics, Source::Error> {
        let mut output = Statistics::empty_for_window(window);

        if let Some((before, aligned, after)) = window.align(CacheSize::SMALLEST.size()) {
            if !before.is_empty() {
                output += &Statistics::compute(source, before)?;
            }
            self.add_aligned_section(&mut output, source, aligned, CacheSize::SMALLEST)?;
            if !after.is_empty() {
                output += &Statistics::compute(source, after)?;
            }
        } else {
            output += &Statistics::compute(source, window)?;
        }

        assert_eq!(output.window, window);

        Ok(output)
    }
}

impl Default for StatisticsHandler {
    fn default() -> Self {
        StatisticsHandler::new()
    }
}
