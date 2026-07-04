//! Implements search through files.

use std::{
    collections::BTreeSet,
    sync::{Arc, Mutex, MutexGuard, RwLock, mpsc},
};

use hexbait_common::Input;

use crate::{
    search::background::{BackgroundSearcher, SearchRequest},
    window::Window,
};

mod background;

/// The searcher types manages searches and reports search results.
pub struct Searcher {
    /// The progress of the current search.
    progress: Arc<RwLock<f32>>,
    /// The search results.
    current_results: Arc<Mutex<BTreeSet<Window>>>,
    /// The requests for new searches to run.
    requests: mpsc::Sender<Option<SearchRequest>>,
}

impl Searcher {
    /// Creates a new searcher.
    pub fn new(input: &Input) -> Searcher {
        let background = BackgroundSearcher::start(input);

        Searcher {
            progress: background.progress,
            current_results: Arc::new(Mutex::new(BTreeSet::new())),
            requests: background.requests,
        }
    }

    /// Starts a new search.
    pub fn start_new_search(
        &mut self,
        content: &[u8],
        ascii_case_insensitive: bool,
        include_utf16: bool,
        window: Window,
    ) {
        let mut search_sequences = vec![content.to_vec()];
        if include_utf16 && let Ok(as_str) = std::str::from_utf8(content) {
            let mut le = Vec::new();
            let mut be = Vec::new();
            for code_unit in as_str.encode_utf16() {
                le.extend_from_slice(&code_unit.to_le_bytes());
                be.extend_from_slice(&code_unit.to_be_bytes());
            }

            search_sequences.push(le);
            search_sequences.push(be);
        }

        self.current_results = Arc::new(Mutex::new(BTreeSet::new()));

        self.requests
            .send(Some(SearchRequest {
                content: search_sequences,
                ascii_case_insensitive,
                window,
                results: Arc::clone(&self.current_results),
            }))
            .unwrap();
    }

    /// Stops a currently ongoing search.
    pub fn stop_search(&self) {
        self.requests.send(None).unwrap();
    }

    /// The progress of the current search.
    pub fn progress(&self) -> f32 {
        *self.progress.read().unwrap()
    }

    /// The current search results.
    pub fn results(&self) -> MutexGuard<'_, BTreeSet<Window>> {
        self.current_results.lock().unwrap()
    }
}
