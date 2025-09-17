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
    pub fn new(source: &Input) -> Searcher {
        let background = BackgroundSearcher::start(source);

        Searcher {
            progress: background.progress,
            results: background.results,
            requests: background.requests,
        }
    }

    /// Starts a new search.
    pub fn start_new_search(&self, content: &[u8], ascii_case_insensitive: bool) {
        self.requests
            .send(SearchRequest {
                content: content.to_vec(),
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
