//! Implements a handler that manages statistics for an input.

use std::{
    cell::Cell,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
};

use background_thread::{BackgroundThread, Request, RequestKind};
use hexbait_common::{AbsoluteOffset, ChangeState, Len};
use quick_cache::sync::Cache;

use crate::{data::Input, window::Window};

use super::Statistics;

pub use statistics_result::StatisticsResult;

#[macro_use]
mod cache_size;
mod background_thread;
mod statistics_result;

/// The minimum window size for entropy requests.
const MIN_ENTROPY_WINDOW_SIZE: Len = Len::from(1024);

cache_sizes! {
    CacheSize {
        8 KiB with 256 entries,
        512 KiB with 256 entries,
        32 MiB with 256 entries,
        2 GiB with 256 entries,
        128 GiB with 256 entries,
        8 TiB with 256 entries,
        512 TiB with 256 entries,
        32 PiB with 256 entries,
        2 PiB with 256 entries,
        128 PiB with 256 entries,
        8 EiB with 1 entries,
    }
}

/// Manages statistics for an input.
pub struct StatisticsHandler {
    /// Statistics for different block-sizes in the input, aligned to the block size.
    aligned_windows: Arc<[Cache<AbsoluteOffset, Arc<Statistics>>; CacheSize::NUM_SIZES]>,
    /// A cache for unaligned windows.
    unaligned_windows: Arc<Cache<Window, Arc<Statistics>>>,
    /// A cache for entropy values of the smallest possible window size.
    entropy_smallest: Arc<Cache<AbsoluteOffset, u8>>,
    /// A cache for entropy values.
    entropy_results: Arc<Cache<Window, u8>>,
    /// Whether the queue in the background thread is empty.
    empty_queue: Arc<AtomicBool>,
    /// The requests for new windows to compute.
    requests: mpsc::Sender<background_thread::Message>,
    /// Whether to send requests for new data to the backend.
    send_requests: bool,
    /// Whether or not an incomplete section was found during computing of the statistics.
    saw_uncompleteness: Cell<bool>,
}

impl StatisticsHandler {
    /// Creates a new statistics handler.
    pub fn new(input: Input) -> StatisticsHandler {
        let (sender, receiver) = mpsc::channel();
        let aligned_windows = Arc::new(std::array::from_fn(|i| {
            Cache::new(CacheSize::try_from_index(i).unwrap().num_entries())
        }));
        let unaligned_windows = Arc::new(Cache::new(4));
        let entropy_smallest = Arc::new(Cache::new(65_536));
        let entropy_results = Arc::new(Cache::new(65_536));
        let empty_queue = Arc::new(AtomicBool::new(false));

        let background_thread = BackgroundThread {
            aligned_windows: Arc::clone(&aligned_windows),
            unaligned_windows: Arc::clone(&unaligned_windows),
            entropy_smallest: Arc::clone(&entropy_smallest),
            entropy_results: Arc::clone(&entropy_results),
            empty_queue: Arc::clone(&empty_queue),
            requests: receiver,
            request_buffer: Vec::new(),
            input,
        };

        std::thread::spawn(|| background_thread.run());

        StatisticsHandler {
            aligned_windows,
            unaligned_windows,
            entropy_smallest,
            entropy_results,
            empty_queue,
            requests: sender,
            send_requests: true,
            saw_uncompleteness: Cell::new(false),
        }
    }

    /// Requests the given window.
    fn request_window(&self, window: Window, request_kind: RequestKind) {
        if self.send_requests {
            self.requests
                .send(background_thread::Message::Compute(Request {
                    kind: request_kind,
                    window,
                }))
                .unwrap();
        }
    }

    /// Returns the cached statistics for the given window or requests it.
    fn get_or_request_aligned(&self, window: Window) -> Option<Arc<Statistics>> {
        let size = CacheSize::try_from(window.size()).expect("not a valid cache size");
        assert!(
            window.start().is_aligned(size.size().as_u64()),
            "unaligned cache request"
        );

        let cached_stats = self.aligned_windows[size.index()].get(&window.start());
        if cached_stats.is_some() {
            return cached_stats;
        }

        self.request_window(window, RequestKind::Bigrams);
        None
    }

    /// Adds an unaligned section to the given statistics.
    fn add_unaligned_section(&self, stats: &mut Statistics, window: Window) -> Len {
        if let Some(window_stats) = self.unaligned_windows.get(&window) {
            *stats += &window_stats;
            return window.size();
        }

        stats.add_empty_window(window);
        self.request_window(window, RequestKind::Bigrams);
        Len::ZERO
    }

    /// Adds a section that is aligned to a cache size.
    fn add_aligned_section(&self, stats: &mut Statistics, window: Window, size: CacheSize) -> Len {
        let mut total_valid_bytes = Len::ZERO;

        let add_window = |this: &Self, stats: &mut Statistics, window: Window| -> Len {
            let mut total_valid_bytes = Len::ZERO;

            for subwindow in window.subwindows_of_size(size.size()) {
                if let Some(window_stats) = this.get_or_request_aligned(subwindow) {
                    *stats += &window_stats;
                    total_valid_bytes += subwindow.size();
                } else {
                    // In case we have no windows of the correct size, we try smaller sizes.
                    // Because of the way the backend fills the caches, it makes sense here to keep
                    // going until the smaller siźe either fully fills the subwindow (in case of
                    // race conditions with the check earlier) or there is none for the specific
                    // sub-subwindow.
                    let mut start = subwindow.start();
                    let mut smaller_size = size.next_down();
                    while let Some(smaller_size_candidate) = smaller_size {
                        if this.aligned_windows[smaller_size_candidate.index()].contains_key(&start)
                        {
                            break;
                        }
                        smaller_size = smaller_size_candidate.next_down();
                    }

                    if let Some(smaller_size) = smaller_size {
                        for subsubwindow in subwindow.subwindows_of_size(smaller_size.size()) {
                            if let Some(subsubwindow_stats) = this.aligned_windows
                                [smaller_size.index()]
                            .get(&subsubwindow.start())
                            {
                                *stats += &subsubwindow_stats;
                                total_valid_bytes += subsubwindow.size();
                                start = subsubwindow.end();
                            } else {
                                break;
                            }
                        }
                    }

                    stats.add_empty_window(Window::new(start, subwindow.end()));
                }
            }

            total_valid_bytes
        };

        if let Some(next_size) = size.next_up()
            && let Some((before, aligned, after)) = window.align(next_size.size().as_u64())
        {
            total_valid_bytes += add_window(self, stats, before);
            total_valid_bytes += self.add_aligned_section(stats, aligned, next_size);
            total_valid_bytes += add_window(self, stats, after);
        } else {
            total_valid_bytes += add_window(self, stats, window);
        }

        total_valid_bytes
    }

    /// Returns the bigram statistics associated with the given window.
    pub fn get_bigram(&self, window: Window) -> StatisticsResult<Statistics> {
        let mut output = Statistics::empty_for_window(window);
        let mut total_valid_bytes = Len::ZERO;

        if let Some((before, aligned, after)) = window.align(CacheSize::SMALLEST.size().as_u64()) {
            if !before.is_empty() {
                total_valid_bytes += self.add_unaligned_section(&mut output, before);
            }
            total_valid_bytes +=
                self.add_aligned_section(&mut output, aligned, CacheSize::SMALLEST);
            if !after.is_empty() {
                total_valid_bytes += self.add_unaligned_section(&mut output, after);
            }
        } else {
            total_valid_bytes += self.add_unaligned_section(&mut output, window);
        }

        assert_eq!(output.window, window);

        if total_valid_bytes != window.size() {
            self.saw_uncompleteness.set(true);
            StatisticsResult::Estimate {
                value: output,
                quality: total_valid_bytes.as_u64() as f32 / window.size().as_u64() as f32,
            }
        } else {
            StatisticsResult::Exact(output)
        }
    }

    /// Returns the entropy of the given window.
    pub fn get_entropy(&self, window: Window) -> StatisticsResult<f32> {
        let window = window.expand_to_align(MIN_ENTROPY_WINDOW_SIZE.as_u64());

        match self.entropy_results.get(&window) {
            Some(result) => StatisticsResult::Exact(result as f32 / 255.0),
            None => {
                self.request_window(window, RequestKind::Entropy);
                match self.entropy_smallest.get(&window.start()) {
                    Some(result) => StatisticsResult::Estimate {
                        value: result as f32 / 255.0,
                        quality: (MIN_ENTROPY_WINDOW_SIZE.as_u64() as f64
                            / window.size().as_u64() as f64)
                            as f32,
                    },
                    None => {
                        self.request_window(window, RequestKind::EntropyEstimate);
                        StatisticsResult::Unknown
                    }
                }
            }
        }
    }

    /// Signals to the statistics handler that a frame has ended.
    ///
    /// The `changed` parameter corresponds to the change state of the scrollbars.
    pub fn end_of_frame(&mut self, changed: ChangeState) {
        match changed {
            ChangeState::Changed => {
                self.requests
                    .send(background_thread::Message::ClearRequests)
                    .unwrap();
                self.send_requests = true;
            }
            ChangeState::Unchanged => {
                self.send_requests =
                    self.saw_uncompleteness.get() && self.empty_queue.load(Ordering::Relaxed);
                self.saw_uncompleteness.set(false);
            }
        }
    }
}
