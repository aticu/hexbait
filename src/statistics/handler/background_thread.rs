//! Defines a thread to compute statistics in the background.

use std::{
    cmp,
    sync::{
        Arc,
        mpsc::{self, TryRecvError},
    },
};

use quick_cache::sync::Cache;

use crate::{
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
    /// Request bigram statistics for a given window.
    Bigrams = 0,
    /// Request flat statistics for a given window.
    Flat = 1,
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

        let key = |request: &Request| (cmp::Reverse(request.window.start()), request.window.size());

        key(self).cmp(&key(other))
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
    pub(crate) entropy_results: Arc<Cache<u64, u8>>,
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
        loop {
            match self.requests.try_recv() {
                Ok(message) => match message {
                    Message::Compute(request) => {
                        self.request_buffer.push(request);
                    }
                    Message::ClearRequests => {
                        self.request_buffer.clear();
                    }
                },
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return false,
            }
        }

        self.request_buffer.sort();

        true
    }

    /// Serves the given request.
    fn serve_request(&mut self, request: Request) {
        match request.kind {
            RequestKind::Bigrams => self.serve_bigram_request(request.window),
            RequestKind::Flat => todo!(),
            RequestKind::EntropyEstimate => self.serve_entropy_request(request.window),
        }
    }

    /// Serves the given bigram request.
    fn serve_bigram_request(&mut self, window: Window) {
        let mut compute = || {
            //eprintln!("computing {window:?}");
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

                self.aligned_windows[cache_size.index()].insert(window.start(), compute());
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
        let window = Window::from_start_len(window.start(), MIN_ENTROPY_WINDOW_SIZE);

        if self.entropy_results.contains_key(&window.start()) {
            return;
        }

        let stats = FlatStatistics::compute(&mut self.source, window)
            .expect("TODO: improve error handling here in the future");
        let entropy = stats.entropy();

        self.entropy_results
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
                // 50ms is probably responsive enough but does not buy loop unnecessarily
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        }
    }
}
