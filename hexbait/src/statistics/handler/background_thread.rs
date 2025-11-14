//! Defines a thread to compute statistics in the background.

use std::{
    cmp,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, TryRecvError},
    },
};

use quick_cache::sync::Cache;

use crate::{
    IDLE_TIME,
    data::Input,
    statistics::{FlatStatistics, Statistics},
    window::Window,
};

use super::{CacheSize, MIN_ENTROPY_WINDOW_SIZE};

/// The messages used to communicate to the background thread.
pub(crate) enum Message {
    /// Computes the given request.
    Compute(Request),
    /// Clears all pending requests.
    ClearRequests,
}

/// The different kinds of requests that can be made to the backend.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub(crate) enum RequestKind {
    /// Request exact entropy for a given window.
    Entropy = 0,
    /// Request bigram statistics for a given window.
    Bigrams = 1,
    /// Request an entropy estimate for a given window.
    EntropyEstimate = 2,
}

/// A request to the backend.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Request {
    /// The kind of request that is made.
    pub(crate) kind: RequestKind,
    /// The window the request applies to.
    pub(crate) window: Window,
}

impl Ord for Request {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        let ordering = self.kind.cmp(&other.kind);
        if ordering.is_ne() {
            return ordering;
        }

        match self.kind {
            RequestKind::Bigrams => {
                // this branch needs to ensure that subrequests created by a larger request are
                // served first
                let key = |request: &Request| {
                    (
                        cmp::Reverse(request.window.size()),
                        cmp::Reverse(request.window.start()),
                    )
                };

                key(self).cmp(&key(other))
            }
            RequestKind::Entropy => {
                // TODO: more rightmost frames should be computed first
                let key = |request: &Request| {
                    (
                        cmp::Reverse(request.window.start()),
                        cmp::Reverse(request.window.size()),
                    )
                };

                key(self).cmp(&key(other))
            }
            RequestKind::EntropyEstimate => {
                self.window.start().cmp(&other.window.start()).reverse()
            }
        }
    }
}

impl PartialOrd for Request {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// A background thread that computes statistics.
pub(crate) struct BackgroundThread {
    /// Statistics for different block-sizes in the input, aligned to the block size.
    ///
    /// Shared with the GUI thread.
    pub(crate) aligned_windows: Arc<[Cache<u64, Arc<Statistics>>; CacheSize::NUM_SIZES]>,
    /// A cache for unaligned windows.
    ///
    /// Shared with the GUI thread.
    pub(crate) unaligned_windows: Arc<Cache<Window, Arc<Statistics>>>,
    /// A cache for entropy values of the smallest possible window size.
    pub(crate) entropy_smallest: Arc<Cache<u64, u8>>,
    /// A cache for entropy values.
    pub(crate) entropy_results: Arc<Cache<Window, u8>>,
    /// Whether the queue in the background thread is empty.
    pub(crate) empty_queue: Arc<AtomicBool>,
    /// The requests for new windows to compute.
    pub(crate) requests: mpsc::Receiver<Message>,
    /// Already received read requests for windows.
    pub(crate) request_buffer: Vec<Request>,
    /// The source to read from.
    pub(crate) source: Input,
}

impl BackgroundThread {
    /// Processes new requests to serve.
    fn process_new_requests(&mut self) -> bool {
        let mut new_requests = false;
        loop {
            match self.requests.try_recv() {
                Ok(message) => {
                    new_requests = true;
                    match message {
                        Message::Compute(request) => {
                            self.request_buffer.push(request);
                        }
                        Message::ClearRequests => {
                            self.request_buffer.clear();
                        }
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return false,
            }
        }

        if new_requests {
            self.empty_queue.store(false, Ordering::Relaxed);
            self.request_buffer.sort();
        }

        true
    }

    /// Serves the given request.
    fn serve_request(&mut self, request: Request) {
        match request.kind {
            RequestKind::Bigrams => self.serve_bigram_request(request.window),
            RequestKind::Entropy => self.serve_entropy_request(request.window),
            RequestKind::EntropyEstimate => self.serve_entropy_estimate_request(request.window),
        }
    }

    /// Serves the given bigram request.
    fn serve_bigram_request(&mut self, window: Window) {
        let mut compute = || {
            let stats = Statistics::compute(&mut self.source, window)
                .expect("TODO: improve error handling here in the future");
            Arc::new(stats)
        };

        match CacheSize::from_size(window.size()) {
            Some(cache_size) => {
                let index = cache_size.index();
                if self.aligned_windows[index].contains_key(&window.start()) {
                    return;
                }

                // split up blocks larger than 64MiB into smaller requests first, so that the
                // background thread never hangs too long
                if cache_size.size() > 64 * 1024 * 1024 {
                    let Some(smaller_size) = cache_size.next_down() else {
                        panic!("trying to compute from smaller cache size when there is none")
                    };
                    let smaller_size_index = smaller_size.index();
                    let smaller_size = smaller_size.size();
                    let mut all_present = true;
                    let mut statistics = Statistics::empty_for_window(window);

                    for i in 0..window.size() / smaller_size {
                        let subwindow =
                            Window::from_start_len(window.start() + i * smaller_size, smaller_size);

                        if let Some(substatistics) =
                            self.aligned_windows[smaller_size_index].get(&subwindow.start())
                        {
                            if all_present {
                                statistics += &substatistics;
                            }
                        } else {
                            self.request_buffer.push(Request {
                                kind: RequestKind::Bigrams,
                                window: subwindow,
                            });
                            if all_present {
                                // ensure that the original request is preserved, but only once
                                all_present = false;
                                self.request_buffer.push(Request {
                                    kind: RequestKind::Bigrams,
                                    window,
                                });
                            }
                        }
                    }

                    if all_present {
                        // we've successfully computed the whole statistics from its parts, time to
                        // cache the result
                        self.aligned_windows[cache_size.index()]
                            .insert(window.start(), Arc::new(statistics));
                    } else {
                        self.request_buffer.sort();
                    }
                } else {
                    self.aligned_windows[cache_size.index()].insert(window.start(), compute());
                }
            }
            None => {
                if self.unaligned_windows.contains_key(&window) {
                    return;
                }

                self.unaligned_windows.insert(window, compute());
            }
        }
    }

    /// Serves the given entropy request.
    fn serve_entropy_request(&mut self, window: Window) {
        if self.entropy_results.contains_key(&window) {
            return;
        }

        let stats = FlatStatistics::compute(&mut self.source, window)
            .expect("TODO: improve error handling here in the future");
        let entropy = stats.entropy();

        self.entropy_results
            .insert(window, (entropy * 255.0).round() as u8);
    }

    /// Serves the given entropy estimate request.
    fn serve_entropy_estimate_request(&mut self, window: Window) {
        let window = Window::from_start_len(window.start(), MIN_ENTROPY_WINDOW_SIZE);

        if self.entropy_smallest.contains_key(&window.start()) {
            return;
        }

        let stats = FlatStatistics::compute(&mut self.source, window)
            .expect("TODO: improve error handling here in the future");
        let entropy = stats.entropy();

        self.entropy_smallest
            .insert(window.start(), (entropy * 255.0).round() as u8);
    }

    /// Runs the background thread.
    pub(crate) fn run(mut self) {
        loop {
            if !self.process_new_requests() {
                break;
            }

            if let Some(request) = self.request_buffer.pop() {
                self.serve_request(request);
            } else {
                self.empty_queue.store(true, Ordering::Relaxed);
                std::thread::sleep(IDLE_TIME);
            }
        }
    }
}
