//! Does the actual searching on a background thread.

use std::{
    collections::BTreeSet,
    sync::{
        Arc, RwLock,
        mpsc::{self, RecvError, TryRecvError},
    },
};

use aho_corasick::AhoCorasick;
use hexbait_common::{AbsoluteOffset, Len};

use crate::{data::Input, window::Window};

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
    overlap_size: usize,
    /// The buffer where file contents are loaded.
    buf: Vec<u8>,
    /// The requests for new searches to run.
    requests: mpsc::Receiver<SearchRequest>,
    /// The input to read from.
    input: Input,
}

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
            overlap_size: 0,
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

        *self.progress.write().unwrap() = 0.0;
        self.results.write().unwrap().clear();

        self.current_offset = AbsoluteOffset::ZERO;
        self.searcher = Some(
            AhoCorasick::builder()
                .ascii_case_insensitive(request.ascii_case_insensitive)
                .build(&request.content)
                .unwrap(),
        );

        self.overlap_size = request.content.len().saturating_sub(1);

        self.buf.clear();
        let buf_len = std::cmp::max(request.content.len() * 2, 4 * 1024 * 1024);
        self.buf.resize(buf_len, 0);

        true
    }

    /// Returns whether a search is currently running.
    fn search_is_running(&self) -> bool {
        self.searcher.is_some()
    }

    /// Runs one iteration of the search.
    fn run_search(&mut self) {
        let current_overlap = if self.current_offset.is_start_of_file() {
            0
        } else {
            self.overlap_size
        };
        let buf = self
            .input
            .window_at(self.current_offset, &mut self.buf[current_overlap..])
            .expect("TODO: improve error handling here");
        if buf.is_empty() {
            self.searcher = None;
            return;
        }
        let buf_len = current_overlap + buf.len();
        let buf = &self.buf[..buf_len];

        for result in self.searcher.as_ref().unwrap().find_overlapping_iter(buf) {
            let offset = AbsoluteOffset::from(
                self.current_offset.as_u64()
                    + u64::try_from(result.start()).expect("read buffer must fit u64")
                    - u64::try_from(current_overlap).expect("overlap cannot exceed u64"),
            );
            let len = Len::from(u64::try_from(result.len()).expect("search string must fit u64"));
            let window = Window::from_start_len(offset, len);
            self.results.write().unwrap().insert(window);
        }

        self.buf
            .copy_within(buf_len - self.overlap_size..buf_len, 0);

        self.current_offset +=
            Len::from(u64::try_from(buf_len - current_overlap).expect("read buffer must fit u64"));

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
