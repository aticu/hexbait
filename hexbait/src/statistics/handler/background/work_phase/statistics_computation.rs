//! Implements the statistics computation phase.

use hexbait_common::{AbsoluteOffset, Len};

use crate::{
    statistics::{
        BigramStatistics,
        handler::background::{
            ComputationState,
            work_phase::{FinishedWork, compute_bin::ComputeBin},
        },
    },
    window::Window,
};

/// Performs full bigram statistics computation.
#[derive(Debug)]
pub struct StatisticsComputation {
    /// The size of bins in this window.
    bin_size: Len,
    /// The offset in the current window that work is currently being performed.
    window_offset: AbsoluteOffset,
    /// The end offset of the computable window.
    end_offset: AbsoluteOffset,
    /// The computation of the current bin.
    compute_bin: Option<ComputeBin<BigramStatistics>>,
}

impl StatisticsComputation {
    /// Returns the initial state for the statistics computation phase.
    pub fn new(computation_state: &mut ComputationState) -> StatisticsComputation {
        let (bin_size, aligned_window) = computation_state.innermost_bin_size_and_aligned_window();

        StatisticsComputation {
            bin_size,
            window_offset: aligned_window.start(),
            end_offset: aligned_window.end(),
            compute_bin: None,
        }
    }

    /// Determines if the given bin still needs to be estimated.
    fn still_needs_bin(
        &self,
        computation_state: &ComputationState,
        offset: AbsoluteOffset,
    ) -> bool {
        let bin = Window::from_start_len(offset, self.bin_size);

        !computation_state
            .current_window_statistics
            .fully_contains(bin)
    }

    /// Continues the current work.
    pub fn advance(&mut self, computation_state: &mut ComputationState) -> Option<FinishedWork> {
        while self.window_offset < self.end_offset || self.compute_bin.is_some() {
            computation_state.maybe_yield()?;

            if let Some(compute_bin) = self.compute_bin.as_mut() {
                compute_bin.advance(computation_state)?;
                let compute_bin = self.compute_bin.take().unwrap();
                let (statistics, bin) = compute_bin.statistics_and_bin();

                computation_state
                    .derived_values
                    .insert(bin, statistics.downsampled().metrics());

                computation_state.current_window_statistics += &statistics;
            }

            if self.window_offset >= self.end_offset {
                continue;
            }

            let old_offset = self.window_offset;
            self.window_offset += self.bin_size;

            if !self.still_needs_bin(computation_state, old_offset) {
                continue;
            }

            let bin = Window::from_start_len(old_offset, self.bin_size);
            self.compute_bin = Some(ComputeBin::new_full(bin));
        }

        Some(FinishedWork)
    }
}
