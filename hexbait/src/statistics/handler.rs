//! Implements a handler that manages statistics for an input.

use std::sync::{Arc, atomic::AtomicU32, mpsc};

use arc_swap::ArcSwap;
use hexbait_common::{Input, Len};

use crate::{
    state::{ScrollState, Settings},
    statistics::StatisticsMetrics,
    window::Window,
};

use super::BigramStatistics;

mod background;

/// A request from the frontend to the backend.
struct Request {
    /// Pre-aligned windows for the backend to fill.
    ///
    /// These are guaranteed to be monotonically decreasing in size and are contained in each other.
    /// Smaller windows always implicitly have higher priority.
    windows: Vec<Window>,
    /// How many bins are visible in each view.
    bins_per_window: u64,
    /// How many bins are visible in the innermost window.
    bins_in_innermost_window: u64,
}

/// The size of the minimum sample window for derived metrics.
const MIN_SAMPLE_SIZE: Len = Len::from(1024);

/// The result for a single scrollbar.
struct BarResultBuffer {
    /// The window covered by the scrollbar.
    ///
    /// This is used to check for staleness.
    window: Window,
    /// The bin size used by the buffer.
    ///
    /// This is used to check for staleness.
    bin_size: Len,
    /// The shared buffer between frontend and backend that contains the results for each bin.
    ///
    /// The length of this is the ceiling of `window.size() / bin_size`.
    buf: Box<[AtomicU32]>,
}

impl BarResultBuffer {
    /// Returns the metrics for the given window.
    fn get(&self, index: usize) -> (Option<StatisticsMetrics>, MetricsQuality) {
        let raw = self.buf[index].load(std::sync::atomic::Ordering::Relaxed);

        let status = raw & 0xff;
        if status == 0 {
            return (None, MetricsQuality::Estimated);
        }

        let quality = if status == 1 {
            MetricsQuality::Estimated
        } else {
            MetricsQuality::Accurate
        };

        let entropy = ((raw >> 8) & 0xff) as u8;
        let printable_ascii = ((raw >> 16) & 0xff) as u8;
        let byte_delta = ((raw >> 24) & 0xff) as u8;

        (
            Some(StatisticsMetrics {
                entropy,
                printable_ascii,
                byte_delta,
            }),
            quality,
        )
    }

    /// Sets the slot in the buffer corresponding to the window to the given metrics and quality.
    fn set(&self, index: usize, metrics: StatisticsMetrics, quality: MetricsQuality) {
        let status = match quality {
            MetricsQuality::Estimated => 1,
            MetricsQuality::Accurate => 2,
        };
        let val = ((metrics.byte_delta as u32) << 24)
            | ((metrics.printable_ascii as u32) << 16)
            | ((metrics.entropy as u32) << 8)
            | status;

        self.buf[index].store(val, std::sync::atomic::Ordering::Relaxed);
    }
}

/// Provides access to the computed statistics metrics.
///
/// Access to this type is only given once it was checked that the window is the correct one.
pub struct StatisticsBufAccess {
    /// The index into the slice of buffers.
    idx: usize,
    /// The slice of buffers to access.
    buf: Arc<[BarResultBuffer]>,
}

impl StatisticsBufAccess {
    /// Returns the metrics for the map and the given window.
    pub fn get_metrics(&self, index: usize) -> (Option<StatisticsMetrics>, MetricsQuality) {
        self.buf[self.idx].get(index)
    }
}

/// Contains the result of backend computations.
struct CalculationResult {
    /// The computed statistics for the selected window.
    statistics: BigramStatistics,
    /// The selected window for which the statistics are calculated.
    selected_window: Window,
    /// The buffers where the computation results for the bars are stored.
    bar_buffers: Arc<[BarResultBuffer]>,
    /// The buffer where the computation results for the Gilbert map are stored.
    ///
    /// This will always be of length `1`, but allows for more uniform code in the buffer access.
    map_buffer: Arc<[BarResultBuffer]>,
}

/// The quality of a returned metrics.
#[derive(Debug, Clone, Copy)]
pub enum MetricsQuality {
    /// The metrics are estimated.
    Estimated,
    /// The metrics are accurately computed.
    Accurate,
}

impl MetricsQuality {
    /// Whether or not the metrics where estimated or accurate.
    pub fn is_estimated(&self) -> bool {
        match self {
            MetricsQuality::Estimated => true,
            MetricsQuality::Accurate => false,
        }
    }
}

/// Manages statistics for an input.
pub struct StatisticsHandler {
    /// The channel over which to send requests to the backend.
    request_channel: mpsc::Sender<Request>,
    /// The result view shared by the backend.
    result: Arc<ArcSwap<CalculationResult>>,
    /// Stores the number of bins in each window.
    bins_per_window: u64,
}

impl StatisticsHandler {
    /// Creates a new statistics handler.
    pub fn new(input: Input) -> StatisticsHandler {
        let background = background::BackgroundStatisticsEngine::start(input);

        StatisticsHandler {
            request_channel: background.request_channel,
            result: background.result,
            bins_per_window: 1,
        }
    }

    /// Returns the bigram statistics associated with the given window along with an estimation quality.
    ///
    /// If the value is not full computed yet, the value is estimated instead.
    /// The quality of the estimation is returned and ranges from `0.0` (worst) to `1.0` (best).
    pub fn get_bigram_statistics(&self, window: Window) -> (BigramStatistics, f32) {
        let result = self.result.load();
        if !result.selected_window.contains_window(window) {
            return (BigramStatistics::empty(), 1.0);
        }

        let coverage = result.statistics.num_covered_bytes() as f32 / window.size().as_u64() as f32;

        (result.statistics.clone(), coverage.clamp(0.0, 1.0))
    }

    /// Returns access to the map metrics if the window still matches.
    pub fn get_map_metrics_access(
        &self,
        window: Window,
        pixel_budget: usize,
    ) -> Option<StatisticsBufAccess> {
        let buf = Arc::clone(&self.result.load().map_buffer);

        if buf[0].window != window.expand_to_align(buf[0].bin_size.as_u64())
            || buf[0].buf.len() != pixel_budget
        {
            return None;
        }

        Some(StatisticsBufAccess { idx: 0, buf })
    }

    /// Returns access to the bar metrics for the give bar index if the window still matches.
    pub fn get_bar_metrics_access(
        &self,
        bar_idx: usize,
        window: Window,
        bin_count: usize,
    ) -> Option<StatisticsBufAccess> {
        let result = self.result.load();

        if result.bar_buffers.len() <= bar_idx
            || result.bar_buffers[bar_idx].window
                != window.expand_to_align(result.bar_buffers[bar_idx].bin_size.as_u64())
            || result.bar_buffers[bar_idx].buf.len() != bin_count
        {
            return None;
        }

        Some(StatisticsBufAccess {
            idx: bar_idx,
            buf: Arc::clone(&result.bar_buffers),
        })
    }

    /// Signals to the statistics handler that a frame has ended.
    ///
    /// The `changed` parameter corresponds to the change state of the scrollbars.
    pub fn end_of_frame(&mut self, settings: &Settings, scroll_state: &ScrollState) {
        if scroll_state.changed().is_changed() {
            self.bins_per_window = (scroll_state.effective_height()
                * if settings.fine_grained_scrollbars() {
                    16
                } else {
                    1
                }) as u64;

            self.request_channel
                .send(Request {
                    windows: scroll_state.windows().collect(),
                    bins_per_window: self.bins_per_window,
                    bins_in_innermost_window: scroll_state.gilbert_pixel_budget,
                })
                .unwrap();
        }
    }
}

/// Computes the bin size for the window and returns the aligned window.
fn compute_bin_size_and_align_window(window: Window, bins_per_window: u64) -> (Len, Window) {
    let bin_size = determine_bin_size(window, bins_per_window);
    let aligned_window = window.expand_to_align(bin_size.as_u64());

    (bin_size, aligned_window)
}

/// Determines the bin size for the given window.
fn determine_bin_size(window: Window, bins_per_window: u64) -> Len {
    let raw_bin_size = window.size() / bins_per_window;
    raw_bin_size_to_bin_size(raw_bin_size)
}

/// Determines the bin size for the given raw bin size.
fn raw_bin_size_to_bin_size(raw_bin_size: Len) -> Len {
    let rounded_bin_size = if raw_bin_size.as_u64().is_power_of_two() {
        raw_bin_size.as_u64()
    } else {
        raw_bin_size.as_u64().next_power_of_two() >> 1
    };

    Len::from(rounded_bin_size).max(MIN_SAMPLE_SIZE)
}
