//! Implements the state for format discovery mode.

use std::collections::BTreeMap;

/// The default length for format discovery mode.
const DEFAULT_LEN: u64 = 64;

/// The state for format discovery mode.
pub struct FormatDiscoveryState {
    /// The type of mark that is used for the format discovery mode.
    ///
    /// If this is `None`, the mode is inactive.
    mark_name: Option<String>,
    /// The length of the structure for each mark type.
    length: BTreeMap<String, u64>,
}

impl FormatDiscoveryState {
    /// Creates new state for format discovery mode.
    pub fn new() -> FormatDiscoveryState {
        FormatDiscoveryState {
            mark_name: None,
            length: BTreeMap::new(),
        }
    }

    /// Enters format discovery mode for the given mark name.
    pub fn enter(&mut self, mark_name: String) {
        self.mark_name = Some(mark_name);
    }

    /// Leaves format discovery mode.
    pub fn exit(&mut self) {
        self.mark_name = None;
    }

    /// Whether format discovery mode is currently active.
    pub fn is_in_format_discovery_mode(&self) -> bool {
        self.mark_name.is_some()
    }

    /// The mark type that is being investigated.
    ///
    /// # Panics
    /// This function will panic if not in format discovery mode.
    pub fn mark_name(&self) -> &str {
        self.mark_name.as_deref().unwrap()
    }

    /// Returns mutable access to the length for the current mark type.
    ///
    /// # Panics
    /// This function will panic if not in format discovery mode.
    pub fn len_mut(&mut self) -> &mut u64 {
        self.length
            .entry(self.mark_name.as_ref().unwrap().clone())
            .or_insert(DEFAULT_LEN)
    }

    /// Returns the length for the current mark type.
    ///
    /// # Panics
    /// This function will panic if not in format discovery mode.
    pub fn len(&self) -> u64 {
        self.length
            .get(self.mark_name.as_ref().unwrap())
            .copied()
            .unwrap_or(DEFAULT_LEN)
    }
}
