//! Implements the state needed to fill a bin with interruptions.

use hexbait_common::{AbsoluteOffset, Len};

use crate::{
    statistics::{
        BigramStatistics,
        downsampled_bigrams::DownsampledBigramStatistics,
        handler::background::{
            ComputationState,
            statistics_tree::{StatisticsTree, Tier},
            work_phase::{FinishedWork, MAX_AGGREGATION_WORK_STEPS},
        },
    },
    window::Window,
};

/// Contains the state to compute a single bin.
#[derive(Debug)]
pub struct ComputeBin<Statistics> {
    /// The statistics into which the bin should be computed.
    statistics: Statistics,
    /// The window that represents the bin.
    bin: Window,
    /// Until which offset previous statistics have already been aggregated.
    aggregated_until: AbsoluteOffset,
    /// A function to access the correct statistics tree.
    get_tree: fn(&mut ComputationState) -> &mut StatisticsTree<Statistics>,
}

impl ComputeBin<BigramStatistics> {
    /// Creates a new bin computation state for a full statistics bin.
    pub fn new_full(bin: Window) -> ComputeBin<BigramStatistics> {
        ComputeBin {
            statistics: BigramStatistics::empty(),
            bin,
            aggregated_until: bin.start(),
            get_tree: |computation_state| &mut computation_state.statistics_tree,
        }
    }
}

impl ComputeBin<DownsampledBigramStatistics> {
    /// Creates a new bin computation state for a downsampled statistics bin.
    pub fn new_downsampled(bin: Window) -> ComputeBin<DownsampledBigramStatistics> {
        ComputeBin {
            statistics: DownsampledBigramStatistics::empty(),
            bin,
            aggregated_until: bin.start(),
            get_tree: |computation_state| &mut computation_state.downsampled_statistics_tree,
        }
    }
}

impl<Statistics: crate::statistics::Statistics> ComputeBin<Statistics> {
    /// Fills the given bin in the statistics tree and updates the given statistics.
    pub fn advance(&mut self, computation_state: &mut ComputationState) -> Option<FinishedWork> {
        while self.aggregated_until < self.bin.end() {
            computation_state.maybe_yield()?;

            self.aggregated_until = (self.get_tree)(computation_state).aggregate_for_window(
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

            if let Ok(statistics) = Statistics::compute(&computation_state.input, new_section) {
                self.statistics += &statistics;
                (self.get_tree)(computation_state).insert(new_section.start(), tier, statistics);
            }
        }

        Some(FinishedWork)
    }

    /// Returns the contained statistics and the bin.
    pub fn statistics_and_bin(self) -> (Statistics, Window) {
        (self.statistics, self.bin)
    }
}
