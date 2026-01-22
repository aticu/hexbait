//! Does the actual searching on a background thread.

use std::{
    collections::BTreeSet,
    sync::{
        Arc, RwLock,
        mpsc::{self, RecvError, TryRecvError},
    },
};

use aho_corasick::AhoCorasick;
use hexbait_common::{AbsoluteOffset, Input, Len};

use crate::window::Window;

/// Contains shared state between background and foreground searcher.
pub(crate) struct BackgroundSearcherStartResult {
    /// The progress of the current search.
    pub(crate) progress: Arc<RwLock<f32>>,
    /// The search results.
    pub(crate) results: Arc<RwLock<BTreeSet<Window>>>,
    /// The requests for new searches to run.
    pub(crate) requests: mpsc::Sender<SearchRequest>,
}

/// The search request that the background thread receives.
pub(crate) struct SearchRequest {
    /// The content to search for.
    pub(crate) content: Vec<Vec<u8>>,
    /// Whether to search case insensitively.
    pub(crate) ascii_case_insensitive: bool,
}

/// The search state of the background searcher.
pub(crate) struct BackgroundSearcher {
    /// The progress of the current search.
    progress: Arc<RwLock<f32>>,
    /// The search results.
    results: Arc<RwLock<BTreeSet<Window>>>,
    /// The current offset at which the search happens.
    current_offset: AbsoluteOffset,
    /// The searcher performing the search itself.
    searcher: Option<AhoCorasick>,
    /// The size of the portion of the buffer that needs to overlap between searches.
    overlap_size: Len,
    /// The size of the search window.
    search_window_size: Len,
    /// The buffer where file contents are loaded.
    buf: Vec<u8>,
    /// The requests for new searches to run.
    requests: mpsc::Receiver<SearchRequest>,
    /// The input to read from.
    input: Input,
}

/// The minimum size of the search window for a single iteration.
const MIN_SEARCH_WINDOW_SIZE: Len = Len::mib(1);

impl BackgroundSearcher {
    /// Starts a background searcher thread.
    pub(crate) fn start(input: &Input) -> BackgroundSearcherStartResult {
        let progress = Arc::new(RwLock::new(1.0));
        let results = Arc::new(RwLock::new(BTreeSet::new()));
        let (sender, receiver) = mpsc::channel();

        let source = input.clone();

        let searcher = BackgroundSearcher {
            progress: Arc::clone(&progress),
            results: Arc::clone(&results),
            current_offset: AbsoluteOffset::ZERO,
            searcher: None,
            overlap_size: Len::ZERO,
            search_window_size: Len::ZERO,
            buf: Vec::new(),
            requests: receiver,
            input: source,
        };

        std::thread::spawn(move || {
            searcher.run();
        });

        BackgroundSearcherStartResult {
            progress,
            results,
            requests: sender,
        }
    }

    /// Processes new search requests.
    ///
    /// A new request will always cancel previous requests.
    fn process_new_requests(&mut self, has_more_work: bool) -> bool {
        let request = if has_more_work {
            match self.requests.try_recv() {
                Ok(request) => request,
                Err(TryRecvError::Empty) => return true,
                Err(TryRecvError::Disconnected) => return false,
            }
        } else {
            match self.requests.recv() {
                Ok(request) => request,
                Err(RecvError) => return false,
            }
        };

        let largest_content_size = Len::from(
            request
                .content
                .iter()
                .map(|content| content.len())
                .max()
                .unwrap_or(0) as u64,
        );

        if largest_content_size.is_zero() {
            return true;
        }

        *self.progress.write().unwrap() = 0.0;
        self.results.write().unwrap().clear();

        self.current_offset = AbsoluteOffset::ZERO;
        self.searcher = Some(
            AhoCorasick::builder()
                .ascii_case_insensitive(request.ascii_case_insensitive)
                .build(&request.content)
                .unwrap(),
        );

        self.overlap_size = largest_content_size - Len::from(1);
        self.search_window_size = std::cmp::max(largest_content_size * 2, MIN_SEARCH_WINDOW_SIZE);

        true
    }

    /// Returns whether a search is currently running.
    fn search_is_running(&self) -> bool {
        self.searcher.is_some()
    }

    /// Runs one iteration of the search.
    fn run_search(&mut self) {
        let current_overlap = if self.current_offset.is_start_of_file() {
            Len::ZERO
        } else {
            self.overlap_size
        };
        let start = self.current_offset - current_overlap;

        // This is a bit wasteful because it reads overlapping bytes multiple times.
        //
        // In practice I expect many searches to be for small patterns, so this is less of an
        // issue. Unfortunately while the new API for reading from `Input` is much nicer for
        // everything else, here it falls short.
        // But even then, when using memory mapped reads, this makes it actually more efficient.
        let buf = self
            .input
            .read_at(start, self.search_window_size, Some(&mut self.buf))
            .expect("TODO: improve error handling here");
        if buf.is_empty() {
            // we finished the search
            self.searcher = None;
            *self.progress.write().unwrap() = 1.0;
            return;
        }
        let buf_len = Len::from(u64::try_from(buf.len()).expect("buffer length must fit u64"));

        for result in self.searcher.as_ref().unwrap().find_overlapping_iter(&*buf) {
            let offset =
                start + Len::from(u64::try_from(result.start()).expect("read buffer must fit u64"));
            let len = Len::from(u64::try_from(result.len()).expect("search string must fit u64"));
            let window = Window::from_start_len(offset, len);
            self.results.write().unwrap().insert(window);
        }

        if Len::from((start + buf_len).as_u64()) == self.input.len() {
            // we finished the search
            self.searcher = None;
            *self.progress.write().unwrap() = 1.0;
            return;
        }

        self.current_offset += buf_len - current_overlap;

        let fraction_completed =
            (self.current_offset.as_u64() as f32) / (self.input.len().as_u64() as f32);

        *self.progress.write().unwrap() = fraction_completed;
    }

    /// Runs the background thread.
    fn run(mut self) {
        loop {
            if !self.process_new_requests(self.search_is_running()) {
                break;
            }

            self.run_search();
        }
    }
}
