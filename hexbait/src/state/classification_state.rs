//! Implements the state for classifier.

use crate::statistics::classification::Class;

/// The state for the input classifier.
pub struct ClassificationState {
    /// The classification results for the currently selected window.
    pub classification_results: Option<Vec<Class>>,
}

impl ClassificationState {
    /// Creates a new classification state.
    pub fn new() -> ClassificationState {
        ClassificationState {
            classification_results: None,
        }
    }
}

impl Default for ClassificationState {
    fn default() -> Self {
        Self::new()
    }
}
