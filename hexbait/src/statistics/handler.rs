//! Implements a handler that manages statistics for an input.

use std::sync::{Arc, mpsc};

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

/// The metrics for a window.
struct WindowMetrics {
    /// The window this entropy value was computed over.
    window: Window,
    /// The metrics of the window.
    metrics: StatisticsMetrics,
}

/// Contains the result of backend computations.
struct CalculationResult {
    /// Computed metrics.
    ///
    /// Sorted by `window`.
    metrics: Vec<WindowMetrics>,
    /// The computed statistics for the selected window.
    statistics: BigramStatistics,
    /// The selected window for which the statistics are calculated.
    selected_window: Window,
}

/// The quality of a returned metrics.
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

    /// Returns the metrics of the given window.
    pub fn get_metrics(&self, window: Window) -> (Option<StatisticsMetrics>, MetricsQuality) {
        let bin_size = raw_bin_size_to_bin_size(window.size());

        let result = self.result.load();
        let window_center = window.start() + window.size() / 2;
        let window =
            Window::from_start_len(window_center, Len::from(1)).expand_to_align(bin_size.as_u64());

        let index = match result
            .metrics
            .binary_search_by_key(&window.start(), |sample| sample.window.start())
        {
            Ok(mut index) => {
                while index > 0 && result.metrics[index - 1].window.start() == window.start() {
                    index -= 1;
                }
                index
            }
            Err(index) => index,
        };

        let mut buf = [StatisticsMetrics::empty(); 5];
        let mut count = 0;

        // TODO: maybe choose a smarter strategy here to subsample? maybe search for end and uniformly choose sub-samples
        for sample in result.metrics[index..].iter().take(5) {
            if sample.window == window {
                return (Some(sample.metrics), MetricsQuality::Accurate);
            }

            if sample.window.start() > window.end() {
                break;
            }

            buf[count] = sample.metrics;
            count += 1;
        }

        (
            StatisticsMetrics::from_average(&buf[..count]),
            if window.size() == MIN_SAMPLE_SIZE {
                MetricsQuality::Accurate
            } else {
                MetricsQuality::Estimated
            },
        )
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
                    bins_in_innermost_window: self.bins_per_window,
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
