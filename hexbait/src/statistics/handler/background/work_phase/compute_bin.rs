//! Implements the state needed to fill a bin with interruptions.

use hexbait_common::{AbsoluteOffset, Len};

use crate::{
    statistics::{
        BigramStatistics,
        handler::background::{
            ComputationState,
            statistics_tree::Tier,
            work_phase::{FinishedWork, MAX_AGGREGATION_WORK_STEPS},
        },
    },
    window::Window,
};

/// Contains the state to compute a single bin.
#[derive(Debug)]
pub struct ComputeBin {
    /// The statistics into which the bin should be computed.
    statistics: BigramStatistics,
    /// The window that represents the bin.
    bin: Window,
    /// Until which offset previous statistics have already been aggregated.
    aggregated_until: AbsoluteOffset,
}

impl ComputeBin {
    /// Creates a new bin computation state.
    pub fn new(bin: Window) -> ComputeBin {
        ComputeBin {
            statistics: BigramStatistics::empty(),
            bin,
            aggregated_until: bin.start(),
        }
    }

    /// Fills the given bin in the statistics tree and updates the given statistics.
    pub fn advance(&mut self, computation_state: &mut ComputationState) -> Option<FinishedWork> {
        while self.aggregated_until < self.bin.end() {
            computation_state.maybe_yield()?;

            self.aggregated_until = computation_state.statistics_tree.aggregate_for_window(
                &mut self.statistics,
                Window::new(self.aggregated_until, self.bin.end()),
                MAX_AGGREGATION_WORK_STEPS,
                Tier::LEAF_TIER,
            );
        }

        while let Some(uncovered_section) =
            self.statistics.first_uncovered_section_in_window(self.bin)
        {
            computation_state.maybe_yield()?;

            // if we're at the end of the input, we should stop here, since there is nothing we can meaningfully do still
            if uncovered_section.start().as_u64() == computation_state.input.len().as_u64() {
                break;
            }

            let section_align = 1
                << ((0..63)
                    .find(|shift| !uncovered_section.start().is_aligned(1 << shift))
                    .unwrap_or(64)
                    - 1);
            let tier_size = uncovered_section.size().min(Len::from(section_align));
            let tier = Tier::fitting_tier(tier_size).min(Tier::MAX_DIRECT_TIER);
            let new_section = Window::from_start_len(uncovered_section.start(), tier.size());

            if let Ok(statistics) = BigramStatistics::compute(&computation_state.input, new_section)
            {
                self.statistics += &statistics;
                computation_state
                    .statistics_tree
                    .insert(new_section.start(), tier, statistics);
            }
        }

        Some(FinishedWork)
    }

    /// Returns the contained statistics and the bin.
    pub fn statistics_and_bin(self) -> (BigramStatistics, Window) {
        (self.statistics, self.bin)
    }
}
