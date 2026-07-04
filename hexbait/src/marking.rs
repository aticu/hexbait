//! Implements marked locations within hexbait.

use std::{collections::BTreeMap, ops::ControlFlow};

use egui::Color32;
use hexbait_common::{AbsoluteOffset, Len};

use crate::{marking::store::SingleTypeStore, window::Window};

mod store;

/// A reference to a marked location.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MarkRef<'store> {
    /// The window covered by the mark.
    pub window: Window,
    /// The type of the mark.
    pub ty: &'store MarkType,
}

impl MarkRef<'_> {
    /// Creates an owned mark from this mark reference.
    pub fn to_owned(&self) -> Mark {
        Mark {
            window: self.window,
            ty: self.ty.clone(),
        }
    }
}

/// A marked location.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Mark {
    /// The window covered by the mark.
    pub window: Window,
    /// The type of the mark.
    pub ty: MarkType,
}

impl Mark {
    /// Returns this mark as a reference.
    pub fn as_ref(&self) -> MarkRef<'_> {
        MarkRef {
            window: self.window,
            ty: &self.ty,
        }
    }
}

impl PartialEq<Mark> for MarkRef<'_> {
    fn eq(&self, other: &Mark) -> bool {
        self.window == other.window && self.ty == &other.ty
    }
}

impl PartialEq<MarkRef<'_>> for Mark {
    fn eq(&self, other: &MarkRef<'_>) -> bool {
        other == self
    }
}

/// The type of a single mark.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MarkType {
    /// The result of a search.
    SearchResult,
    /// A location marked by a user.
    UserMark {
        /// The name of the marked location.
        name: String,
    },
    /// A user selection.
    Selection,
    /// Provenance of a hovered parsed value.
    HoveredParsed,
    /// Provenance of a hovered parsing error.
    HoveredParseErr,
}

impl MarkType {
    /// The inner color of this marked location.
    pub fn inner_color(&self) -> Color32 {
        match self {
            MarkType::SearchResult => Color32::BLUE,
            MarkType::UserMark { .. } => Color32::WHITE,
            MarkType::Selection => Color32::WHITE,
            MarkType::HoveredParsed => Color32::DARK_RED,
            MarkType::HoveredParseErr => Color32::WHITE,
        }
    }

    /// The border color of this marked location.
    pub fn border_color(&self) -> Color32 {
        match self {
            MarkType::SearchResult => Color32::from_rgb(252, 15, 192),
            MarkType::UserMark { .. } => Color32::DARK_RED,
            MarkType::Selection => Color32::WHITE,
            MarkType::HoveredParsed => Color32::GOLD,
            MarkType::HoveredParseErr => Color32::LIGHT_RED,
        }
    }
}

/// A store for marked locations.
pub struct MarkStore {
    /// The actual stores separated by mark type.
    per_type: BTreeMap<MarkType, SingleTypeStore>,
    /// The currently hovered location.
    hovered_location: Option<Mark>,
    /// The new location that was hovered this frame.
    new_hovered_location: Option<Mark>,
    /// The name of the current mark.
    pub current_mark_name: String,
}

impl MarkStore {
    /// Creates a new store for marked locations.
    pub fn new() -> MarkStore {
        MarkStore {
            per_type: BTreeMap::new(),
            hovered_location: None,
            new_hovered_location: None,
            current_mark_name: String::new(),
        }
    }

    /// Adds a new marked location.
    pub fn add(&mut self, window: Window, ty: MarkType) {
        let store = self.per_type.entry(ty).or_default();
        store.insert(window);
        store.consolidate();
    }

    /// Adds new marked locations in a batch.
    pub fn batch_add(&mut self, windows: impl Iterator<Item = Window>, ty: MarkType) {
        let store = self.per_type.entry(ty).or_default();
        store.extend(windows);
        store.consolidate();
    }

    /// Clears all marks of the given type.
    pub fn clear_marks_of_type(&mut self, ty: MarkType) {
        self.per_type.remove(&ty);
    }

    /// Removes all marks that match the filter and (if it is `Some(_)`) `ty`.
    pub fn remove_where(&mut self, ty: Option<MarkType>, mut filter: impl FnMut(MarkRef) -> bool) {
        match ty {
            Some(ty) => {
                let Some(store) = self.per_type.get_mut(&ty) else {
                    return;
                };
                store.remove_where(|window| filter(MarkRef { window, ty: &ty }));
                store.consolidate();
            }
            None => {
                for (ty, store) in &mut self.per_type {
                    store.remove_where(|window| filter(MarkRef { window, ty }));
                    store.consolidate();
                }
            }
        }
    }

    /// Iterates over all marks in the given window.
    pub fn iter_marks_in_window<'store>(
        &'store self,
        window: Window,
        mut out: impl FnMut(MarkRef<'store>),
    ) {
        for (ty, store) in &self.per_type {
            let _ = store.query_window(window, |window| {
                out(MarkRef { window, ty });
                ControlFlow::Continue(())
            });
        }
    }

    /// Returns the "best" mark at the position.
    ///
    /// The exact algorithm used is unspecified and may change in the future.
    pub fn mark_at_pos<'store>(&'store self, offset: AbsoluteOffset) -> Option<MarkRef<'store>> {
        let mut out = None;

        let key =
            |mark: MarkRef<'store>| (std::cmp::Reverse(mark.window.size()), mark.ty, mark.window);

        for (ty, store) in &self.per_type {
            let _ = store.query_window(Window::from_start_len(offset, Len::from(1)), |window| {
                let mark = MarkRef { window, ty };
                match out {
                    Some(current_out) if key(mark) > key(current_out) => out = Some(mark),
                    None => out = Some(mark),
                    _ => (),
                }
                ControlFlow::Continue(())
            });
        }

        out
    }

    /// Returns the mark at the given position.
    pub fn user_mark_at_pos(&self, offset: AbsoluteOffset) -> Option<MarkRef<'_>> {
        let mut out = None;

        for (ty, store) in &self.per_type {
            if !matches!(ty, MarkType::UserMark { .. }) {
                continue;
            }

            let _ = store.query_window(Window::from_start_len(offset, Len::from(1)), |window| {
                out = Some(MarkRef { window, ty });
                ControlFlow::Break(())
            });
            if out.is_some() {
                return out;
            }
        }

        out
    }

    /// Returns the number of marks with the given type.
    pub fn count_of_type(&self, ty: MarkType) -> usize {
        self.per_type.get(&ty).map(|store| store.len()).unwrap_or(0)
    }

    /// Returns the hovered mark, if any.
    pub fn hovered(&self) -> Option<&Mark> {
        self.hovered_location.as_ref()
    }

    /// Marks the given mark as hovered.
    pub fn mark_hovered(&mut self, mark: Mark) {
        self.new_hovered_location = Some(mark);
    }

    /// Marks the end of the frame, updating the marked location.
    pub fn end_of_frame(&mut self) {
        self.hovered_location = self.new_hovered_location.take();
    }
}

impl Default for MarkStore {
    fn default() -> Self {
        MarkStore::new()
    }
}
