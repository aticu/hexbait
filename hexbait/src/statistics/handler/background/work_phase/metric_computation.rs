//! Implements the metric computation phase.

use hexbait_common::{AbsoluteOffset, Len};

use crate::{
    statistics::{
        MetricsQuality, Statistics as _, StatisticsMetrics,
        downsampled_bigrams::DownsampledBigramStatistics,
        handler::{
            MIN_SAMPLE_SIZE,
            background::{
                ComputationState,
                work_phase::{FinishedWork, compute_bin::ComputeBin},
            },
        },
    },
    window::Window,
};

/// The different computation modes for metrics computations.
#[derive(Debug, Clone, Copy)]
pub enum ComputationMode {
    /// Estimate metrics quickly.
    Estimation,
    /// Perform full-quality metrics computations.
    FullQuality,
}

/// Performs computation of the required metrics.
#[derive(Debug)]
pub struct MetricComputation {
    /// The way the computation is done.
    mode: ComputationMode,
    /// The index of the window that is currently being worked on.
    window_index: usize,
    /// The size of bins in this window.
    bin_size: Len,
    /// The offset in the current window that is currently being worked on.
    window_offset: AbsoluteOffset,
    /// The end offset of the computable window.
    end_offset: AbsoluteOffset,
    /// The computation for the current bin.
    compute_bin: Option<ComputeBin<DownsampledBigramStatistics>>,
    /// The total number of bins covered by the window.
    total_bins: usize,
    /// The number of bins that have already been computed in the current window.
    bin_count: usize,
    /// The index into the output buffer of the current bar where the results should be written to.
    out_index: usize,
    /// The metrics quality for the current bar.
    quality: MetricsQuality,
    /// The metrics quality for the map.
    map_quality: MetricsQuality,
    /// Whether we are currently computing the map.
    is_map: bool,
    /// The index into the output buffer of the map where the results should be written to.
    map_out_index: usize,
}

/// Computes the quality from the given bin size.
fn estimation_quality_from_bin_size(bin_size: Len) -> MetricsQuality {
    if bin_size == MIN_SAMPLE_SIZE {
        MetricsQuality::Accurate
    } else {
        MetricsQuality::Estimated
    }
}

impl MetricComputation {
    /// Returns the initial state for the metric computation phase.
    pub fn new(
        mode: ComputationMode,
        computation_state: &mut ComputationState,
    ) -> MetricComputation {
        let window_index = computation_state.last_window_index();
        let map_info = computation_state.innermost_bin_size_and_aligned_window();
        let bar_info = computation_state.bin_size_and_aligned_window(window_index);

        let (bin_size, aligned_window) = match mode {
            ComputationMode::Estimation => bar_info,
            ComputationMode::FullQuality => map_info,
        };

        MetricComputation {
            mode,
            window_index,
            bin_size,
            window_offset: aligned_window.start(),
            end_offset: aligned_window.end(),
            compute_bin: None,
            total_bins: (aligned_window.size().as_u64() / bin_size.as_u64()) as usize,
            bin_count: 0,
            out_index: 0,
            quality: match mode {
                ComputationMode::Estimation => estimation_quality_from_bin_size(bar_info.0),
                ComputationMode::FullQuality => MetricsQuality::Accurate,
            },
            map_quality: match mode {
                ComputationMode::Estimation => estimation_quality_from_bin_size(bin_size),
                ComputationMode::FullQuality => MetricsQuality::Accurate,
            },
            is_map: true,
            map_out_index: 0,
        }
    }

    /// Returns the computation mode of this metrics computation.
    pub fn mode(&self) -> &ComputationMode {
        &self.mode
    }

    /// Prepares the state for the next window.
    fn next_window(&mut self, computation_state: &mut ComputationState) {
        self.window_index -= 1;

        let (bin_size, aligned_window) =
            computation_state.bin_size_and_aligned_window(self.window_index);
        self.bin_size = bin_size;
        self.window_offset = aligned_window.start();
        self.end_offset = aligned_window.end();
        self.total_bins = (aligned_window.size().as_u64() / bin_size.as_u64()) as usize;
        self.bin_count = 0;
        self.out_index = 0;
        self.quality = match self.mode {
            ComputationMode::Estimation => estimation_quality_from_bin_size(bin_size),
            ComputationMode::FullQuality => MetricsQuality::Accurate,
        };
        self.is_map = false;
    }

    /// Returns an iterator over sub-bin metrics contained in the bin at `offset`.
    fn contained_bins(
        &self,
        offset: AbsoluteOffset,
        computation_state: &ComputationState,
    ) -> impl Iterator<Item = (Window, StatisticsMetrics)> {
        let bin_window = Window::from_start_len(offset, self.bin_size);

        computation_state
            .derived_values
            .range(
                Window::empty_from_start(bin_window.start())
                    ..Window::empty_from_start(bin_window.end()),
            )
            .filter(move |(window, _)| bin_window.contains_window(**window))
            .map(|(&window, &metrics)| (window, metrics))
    }

    /// Writes the just computed metrics to the output buffers.
    fn write_metrics(
        &mut self,
        metrics: StatisticsMetrics,
        computation_state: &mut ComputationState,
    ) {
        let bar_buf = &computation_state.bar_buffers[self.window_index];
        let bar_end = self.bin_count * bar_buf.buf.len() / self.total_bins;
        for i in self.out_index..bar_end {
            bar_buf.set(i, metrics, self.quality);
        }
        self.out_index = bar_end;

        if self.is_map {
            let map_buf = &computation_state.map_buffer[0];
            let map_end = self.bin_count * map_buf.buf.len() / self.total_bins;
            for i in self.map_out_index..map_end {
                map_buf.set(i, metrics, self.map_quality);
            }
            self.map_out_index = map_end;
        }
    }

    /// Returns a previously cached result for the current window.
    fn cached_result(&self, computation_state: &mut ComputationState) -> Option<StatisticsMetrics> {
        match self.mode {
            ComputationMode::Estimation => {
                let mut buf = [StatisticsMetrics::empty(); 5];
                let mut count = 0;
                for (_, sample) in self
                    .contained_bins(self.window_offset, computation_state)
                    .take(5)
                {
                    buf[count] = sample;
                    count += 1;
                }

                StatisticsMetrics::from_average(&buf[..count])
            }
            ComputationMode::FullQuality => {
                let window = Window::from_start_len(self.window_offset, self.bin_size);

                computation_state.derived_values.get(&window).copied()
            }
        }
    }

    /// The size to sample at.
    fn computation_size(&self) -> Len {
        match self.mode {
            ComputationMode::Estimation => MIN_SAMPLE_SIZE,
            ComputationMode::FullQuality => self.bin_size,
        }
    }

    /// Advances the bin computation if one is happening.
    fn advance_bin_computation(
        &mut self,
        computation_state: &mut ComputationState,
    ) -> Option<FinishedWork> {
        if let Some(compute_bin) = self.compute_bin.as_mut() {
            compute_bin.advance(computation_state)?;
            let compute_bin = self.compute_bin.take().unwrap();
            let (statistics, bin) = compute_bin.statistics_and_bin();
            let metrics = statistics.metrics();

            computation_state.derived_values.insert(bin, metrics);
            self.write_metrics(metrics, computation_state);
        }

        Some(FinishedWork)
    }

    /// Continues the current work.
    pub fn advance(&mut self, computation_state: &mut ComputationState) -> Option<FinishedWork> {
        loop {
            while self.window_offset < self.end_offset {
                computation_state.maybe_yield()?;
                self.advance_bin_computation(computation_state)?;

                let cached_result = self.cached_result(computation_state);

                let old_offset = self.window_offset;
                self.window_offset += self.bin_size;
                self.bin_count += 1;

                if let Some(cached_result) = cached_result {
                    self.write_metrics(cached_result, computation_state);
                    continue;
                }

                let computation_window =
                    Window::from_start_len(old_offset, self.computation_size());

                match self.mode {
                    ComputationMode::Estimation => {
                        // don't use the compute bin mechanism for estimation to avoid polluting the statistics tree with stray MIN_SAMPLE_SIZE windows that cannot be merged
                        let compute_result = DownsampledBigramStatistics::compute(
                            &computation_state.input,
                            computation_window,
                        );

                        let Ok(statistics) = compute_result else {
                            continue;
                        };
                        let metrics = statistics.metrics();
                        computation_state
                            .derived_values
                            .insert(computation_window, metrics);

                        self.write_metrics(metrics, computation_state);
                    }
                    ComputationMode::FullQuality => {
                        self.compute_bin = Some(ComputeBin::new_downsampled(computation_window));
                    }
                }
            }

            // make sure the final bin is finished
            self.advance_bin_computation(computation_state)?;

            if self.window_index == 0 {
                break;
            } else {
                self.next_window(computation_state);
            }
        }

        Some(FinishedWork)
    }
}
