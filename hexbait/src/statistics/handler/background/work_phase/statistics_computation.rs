//! Implements the statistics computation phase.

use hexbait_common::{AbsoluteOffset, Len};

use crate::{
    statistics::handler::background::{
        ComputationState,
        work_phase::{FinishedWork, compute_bin::ComputeBin},
    },
    window::Window,
};

/// Performs statistics computation.
#[derive(Debug)]
pub struct StatisticsComputation {
    /// The index of the window that is currently being worked on.
    window_index: usize,
    /// The size of bins in this window.
    bin_size: Len,
    /// The offset in the current window that work is currently being performed.
    window_offset: AbsoluteOffset,
    /// The end offset of the computable window.
    end_offset: AbsoluteOffset,
    /// The computation of the current bin.
    compute_bin: Option<ComputeBin>,
}

impl StatisticsComputation {
    /// Returns the initial state for the statistics computation phase.
    pub fn new(computation_state: &mut ComputationState) -> StatisticsComputation {
        let window_index = computation_state.last_window_index();
        let (bin_size, aligned_window) =
            computation_state.bin_size_and_aligned_window(window_index);

        StatisticsComputation {
            window_index,
            bin_size,
            window_offset: aligned_window.start(),
            end_offset: aligned_window.end(),
            compute_bin: None,
        }
    }

    /// Prepares the state for the next window.
    fn next_window(&mut self, computation_state: &mut ComputationState) {
        self.window_index -= 1;

        let (bin_size, aligned_window) =
            computation_state.bin_size_and_aligned_window(self.window_index);
        self.bin_size = bin_size;
        self.window_offset = aligned_window.start();
        self.end_offset = aligned_window.end();
    }

    /// Determines if the given bin still needs to be estimated.
    fn still_needs_bin(
        &self,
        computation_state: &ComputationState,
        offset: AbsoluteOffset,
    ) -> bool {
        let bin = Window::from_start_len(offset, self.bin_size);

        !computation_state.derived_values.contains_key(&bin)
            || (self.window_index == computation_state.last_window_index()
                && !computation_state
                    .current_window_statistics
                    .fully_contains(bin))
    }

    /// Continues the current work.
    pub fn advance(&mut self, computation_state: &mut ComputationState) -> Option<FinishedWork> {
        loop {
            while self.window_offset < self.end_offset || self.compute_bin.is_some() {
                computation_state.maybe_yield()?;

                if let Some(compute_bin) = self.compute_bin.as_mut() {
                    compute_bin.advance(computation_state)?;
                    let compute_bin = self.compute_bin.take().unwrap();
                    let (statistics, bin) = compute_bin.statistics_and_bin();

                    computation_state
                        .derived_values
                        .insert(bin, statistics.downsampled().metrics());

                    if self.window_index == computation_state.last_window_index() {
                        computation_state.current_window_statistics += &statistics;
                    }
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
                self.compute_bin = Some(ComputeBin::new(bin));
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
