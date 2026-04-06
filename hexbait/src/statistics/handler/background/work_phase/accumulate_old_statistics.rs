//! Implements the statistics computation phase.

use hexbait_common::{AbsoluteOffset, Len};

use crate::{
    statistics::{
        Statistics,
        handler::background::{
            ComputationState,
            statistics_tree::Tier,
            work_phase::{FinishedWork, MAX_AGGREGATION_WORK_STEPS},
        },
    },
    window::Window,
};

/// Performs accumulation of previously computed statistics.
#[derive(Debug)]
pub struct AccumulateOldStatistics {
    /// The size of a single bin.
    bin_size: Len,
    /// The current offset at which statistics are accumulated.
    offset: AbsoluteOffset,
    /// The end offset until which statistics need to be accumulated.
    end_offset: AbsoluteOffset,
    /// Tracks statistics of a partially aggregated bin.
    bin_statistics: Option<Statistics>,
}

impl AccumulateOldStatistics {
    /// Returns the initial state for the statistics computation phase.
    pub fn new(computation_state: &mut ComputationState) -> AccumulateOldStatistics {
        let window_index = computation_state.last_window_index();
        let (bin_size, aligned_window) =
            computation_state.bin_size_and_aligned_window(window_index);

        AccumulateOldStatistics {
            bin_size,
            offset: aligned_window.start(),
            end_offset: aligned_window.end(),
            bin_statistics: None,
        }
    }

    /// Continues the current work.
    pub fn advance(&mut self, computation_state: &mut ComputationState) -> Option<FinishedWork> {
        while self.offset < self.end_offset {
            computation_state.maybe_yield()?;

            let bin = Window::from_start_len(self.offset, self.bin_size);

            let bin_statistics = match &mut self.bin_statistics {
                Some(partial_bin_statistics) => partial_bin_statistics,
                None => {
                    if computation_state.statistics_tree.covers_window_exactly(bin) {
                        self.bin_statistics.insert(Statistics::empty())
                    } else {
                        self.offset += self.bin_size;
                        continue;
                    }
                }
            };

            if let Some(uncovered_section) = bin_statistics.first_uncovered_section_in_window(bin) {
                computation_state.statistics_tree.aggregate_for_window(
                    bin_statistics,
                    uncovered_section,
                    MAX_AGGREGATION_WORK_STEPS,
                    Tier::LEAF_TIER,
                );
            } else {
                let statistics = self.bin_statistics.take().unwrap();
                computation_state
                    .derived_values
                    .insert(bin, statistics.entropy());
                computation_state.current_window_statistics += &statistics;

                self.offset += self.bin_size;
            }
        }

        Some(FinishedWork)
    }
}
