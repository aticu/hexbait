//! Implements search through files.

use std::{
    collections::BTreeSet,
    sync::{Arc, RwLock, RwLockReadGuard, mpsc},
};

use crate::{
    data::Input,
    search::background::{BackgroundSearcher, SearchRequest},
    window::Window,
};

mod background;

/// The searcher types manages searches and reports search results.
pub struct Searcher {
    /// The progress of the current search.
    progress: Arc<RwLock<f32>>,
    /// The search results.
    results: Arc<RwLock<BTreeSet<Window>>>,
    /// The requests for new searches to run.
    requests: mpsc::Sender<SearchRequest>,
}

impl Searcher {
    /// Creates a new searcher.
    pub fn new(input: &Input) -> Searcher {
        let background = BackgroundSearcher::start(input);

        Searcher {
            progress: background.progress,
            results: background.results,
            requests: background.requests,
        }
    }

    /// Starts a new search.
    pub fn start_new_search(
        &self,
        content: &[u8],
        ascii_case_insensitive: bool,
        include_utf16: bool,
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

        self.requests
            .send(SearchRequest {
                content: search_sequences,
                ascii_case_insensitive,
            })
            .unwrap();
    }

    /// The progress of the current search.
    pub fn progress(&self) -> f32 {
        *self.progress.read().unwrap()
    }

    /// The current search results.
    pub fn results(&self) -> RwLockReadGuard<'_, BTreeSet<Window>> {
        self.results.read().unwrap()
    }
}
