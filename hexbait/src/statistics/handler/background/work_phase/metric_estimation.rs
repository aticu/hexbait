//! Implements the metric estimation phase.

use hexbait_common::{AbsoluteOffset, Len};

use crate::{
    statistics::{
        downsampled_bigrams::DownsampledBigramStatistics,
        handler::{
            MIN_SAMPLE_SIZE,
            background::{ComputationState, work_phase::FinishedWork},
        },
    },
    window::Window,
};

/// Performs estimation for the computed metrics.
#[derive(Debug)]
pub struct MetricEstimation {
    /// The index of the window that is currently being worked on.
    window_index: usize,
    /// The size of bins in this window.
    bin_size: Len,
    /// The offset in the current window that work is currently being performed.
    window_offset: AbsoluteOffset,
    /// The end offset of the computable window.
    end_offset: AbsoluteOffset,
    /// The offset until which prefetching was already signaled.
    ///
    /// For simplicity prefetching is only done on a per-window basis.
    prefetch_offset: AbsoluteOffset,
}

/// The number of bins to prefetch.
const PREFETCH_COUNT: usize = 64;

impl MetricEstimation {
    /// Returns the initial state for the metric estimation phase.
    pub fn new(computation_state: &mut ComputationState) -> MetricEstimation {
        let window_index = computation_state.last_window_index();
        let (bin_size, aligned_window) =
            computation_state.bin_size_and_aligned_window(window_index);

        let mut out = MetricEstimation {
            window_index,
            bin_size,
            window_offset: aligned_window.start(),
            end_offset: aligned_window.end(),
            prefetch_offset: aligned_window.start(),
        };

        for _ in 0..PREFETCH_COUNT {
            out.prefetch_once(computation_state);
        }

        out
    }

    /// Prepares the state for the next window.
    fn next_window(&mut self, computation_state: &mut ComputationState) {
        self.window_index -= 1;

        let (bin_size, aligned_window) =
            computation_state.bin_size_and_aligned_window(self.window_index);
        self.bin_size = bin_size;
        self.window_offset = aligned_window.start();
        self.end_offset = aligned_window.end();
        self.prefetch_offset = aligned_window.start();

        for _ in 0..PREFETCH_COUNT {
            self.prefetch_once(computation_state);
        }
    }

    /// Determines if the given bin still needs to be estimated.
    fn still_needs_bin(
        &self,
        offset: AbsoluteOffset,
        computation_state: &ComputationState,
    ) -> bool {
        let bin_window = Window::from_start_len(offset, self.bin_size);

        !computation_state
            .derived_values
            .range(
                Window::empty_from_start(bin_window.start())
                    ..Window::empty_from_start(bin_window.end()),
            )
            .any(|(window, _)| bin_window.contains_window(*window))
    }

    /// Performs prefetching within the current window.
    fn prefetch_once(&mut self, computation_state: &mut ComputationState) {
        while self.prefetch_offset < self.end_offset {
            let old_offset = self.prefetch_offset;
            self.prefetch_offset += self.bin_size;

            if self.still_needs_bin(old_offset, computation_state) {
                computation_state
                    .input
                    .signal_planned_read(old_offset, MIN_SAMPLE_SIZE);
                break;
            }
        }
    }

    /// Continues the current work.
    pub fn advance(&mut self, computation_state: &mut ComputationState) -> Option<FinishedWork> {
        loop {
            while self.window_offset < self.end_offset {
                computation_state.maybe_yield()?;

                let old_offset = self.window_offset;
                self.window_offset += self.bin_size;

                if !self.still_needs_bin(old_offset, computation_state) {
                    continue;
                }
                self.prefetch_once(computation_state);

                let sample_window = Window::from_start_len(old_offset, MIN_SAMPLE_SIZE);
                let Ok(statistics) =
                    DownsampledBigramStatistics::compute(&computation_state.input, sample_window)
                else {
                    continue;
                };
                computation_state
                    .derived_values
                    .insert(sample_window, statistics.metrics());
            }

            if self.window_index == 0 {
                break;
            } else {
                self.next_window(computation_state);
            }
        }

        Some(FinishedWork)
    }
}
