//! Implements different phases of work in the background handler.

use crate::statistics::handler::background::{
    ComputationState,
    work_phase::{
        accumulate_old_statistics::AccumulateOldStatistics, metric_estimation::MetricEstimation,
        statistics_computation::StatisticsComputation,
    },
};

mod accumulate_old_statistics;
mod compute_bin;
mod metric_estimation;
mod statistics_computation;

/// Represents that some work was finished.
pub struct FinishedWork;

/// Tracks the different work phases that the background thread goes through.
#[derive(Debug)]
#[allow(private_interfaces)]
pub enum WorkPhase {
    /// There is nothing to do.
    Idle,
    /// Performs estimation for the computed metrics.
    MetricEstimation(MetricEstimation),
    /// Performs accumulation of previously computed statistics.
    AccumulateOldStatistics(AccumulateOldStatistics),
    /// Performs statistics computation.
    StatisticsComputation(StatisticsComputation),
}

impl WorkPhase {
    /// Restarts work from the beginning.
    pub fn from_beginning(computation_state: &mut ComputationState) -> WorkPhase {
        WorkPhase::MetricEstimation(MetricEstimation::new(computation_state))
    }

    /// Continues the current work.
    pub fn advance(&mut self, computation_state: &mut ComputationState) -> Option<FinishedWork> {
        loop {
            match self {
                WorkPhase::Idle => {
                    break;
                }
                WorkPhase::MetricEstimation(entropy_estimation) => {
                    entropy_estimation.advance(computation_state)?;
                    *self = WorkPhase::AccumulateOldStatistics(AccumulateOldStatistics::new(
                        computation_state,
                    ));
                    continue;
                }
                WorkPhase::AccumulateOldStatistics(accumulate_old_statistics) => {
                    accumulate_old_statistics.advance(computation_state)?;
                    *self = WorkPhase::StatisticsComputation(StatisticsComputation::new(
                        computation_state,
                    ));
                    continue;
                }
                WorkPhase::StatisticsComputation(statistics_computation) => {
                    statistics_computation.advance(computation_state)?;
                    *self = WorkPhase::Idle;
                    continue;
                }
            }
        }

        Some(FinishedWork)
    }
}

/// How many statistics to aggregate in one step.
const MAX_AGGREGATION_WORK_STEPS: usize = 16;
