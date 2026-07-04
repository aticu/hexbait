//! Implements a store for marked locations of a single type.

use std::ops::ControlFlow;

use hexbait_common::AbsoluteOffset;

use crate::window::Window;

/// The store for a single type of mark.
pub struct SingleTypeStore {
    /// The marks of the given type.
    marks: Vec<Window>,
    /// The maximum end value of the binary search subtree rooted at the given index.
    max_end: Vec<AbsoluteOffset>,
    /// Whether there was an insertion since the last consolidation.
    dirty: bool,
}

impl SingleTypeStore {
    /// Creates a new store for a single type of mark.
    pub fn new() -> SingleTypeStore {
        SingleTypeStore {
            marks: Vec::new(),
            max_end: Vec::new(),
            dirty: false,
        }
    }

    /// The number of marks stored in this store.
    pub fn len(&self) -> usize {
        self.marks.len()
    }

    /// Inserts a new mark into the store.
    pub fn insert(&mut self, window: Window) {
        self.dirty = true;
        self.marks.push(window);
    }

    /// Inserts a new mark into the store.
    pub fn extend(&mut self, windows: impl Iterator<Item = Window>) {
        self.dirty = true;
        self.marks.extend(windows);
    }

    /// Removes all marks where the filter is true.
    pub fn remove_where(&mut self, mut filter: impl FnMut(Window) -> bool) {
        let len_before = self.marks.len();
        self.marks.retain(|&mark| !filter(mark));

        if self.marks.len() != len_before {
            self.dirty = true;
        }
    }

    /// Queries the given window for marks that overlap with it.
    ///
    /// The behavior is unspecified for empty windows (both as query and in the marks).
    pub fn query_window(
        &self,
        window: Window,
        mut out: impl FnMut(Window) -> ControlFlow<()>,
    ) -> ControlFlow<()> {
        assert!(!self.dirty);

        fn walk_tree(
            marks: &[Window],
            max_end: &[AbsoluteOffset],
            window: Window,
            out: &mut impl FnMut(Window) -> ControlFlow<()>,
        ) -> ControlFlow<()> {
            if marks.is_empty() {
                return ControlFlow::Continue(());
            }

            let mid = marks.len() / 2;
            if max_end[mid] <= window.start() {
                // the whole subtree ends before the window starts
                return ControlFlow::Continue(());
            }

            walk_tree(&marks[..mid], &max_end[..mid], window, out)?;

            let current = marks[mid];
            if current.start() >= window.end() {
                // the whole subtree starts after the window ends
                return ControlFlow::Continue(());
            }

            if current.end() > window.start() {
                out(current)?;
            }

            walk_tree(&marks[mid + 1..], &max_end[mid + 1..], window, out)
        }

        walk_tree(&self.marks, &self.max_end, window, &mut out)
    }

    /// Runs consolidation so internal data structures are correct.
    ///
    /// This is required before any queries can be performed.
    pub fn consolidate(&mut self) {
        if !self.dirty {
            return;
        }

        fn walk_tree(marks: &[Window], max_end: &mut [AbsoluteOffset]) -> AbsoluteOffset {
            if marks.is_empty() {
                return AbsoluteOffset::ZERO;
            }

            let mid = marks.len() / 2;
            let left = walk_tree(&marks[..mid], &mut max_end[..mid]);
            let right = walk_tree(&marks[mid + 1..], &mut max_end[mid + 1..]);

            max_end[mid] = [left, right, marks[mid].end()].into_iter().max().unwrap();
            max_end[mid]
        }

        self.marks.sort();
        self.marks.dedup();
        self.max_end.resize(self.marks.len(), AbsoluteOffset::ZERO);
        walk_tree(&self.marks, &mut self.max_end);
        self.dirty = false;
    }
}

impl Default for SingleTypeStore {
    fn default() -> Self {
        SingleTypeStore::new()
    }
}
