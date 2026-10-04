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

    /// A description of the mark.
    pub fn description(&self) -> &str {
        match &self {
            MarkType::SearchResult => "Search result",
            MarkType::UserMark { .. } => "User mark",
            MarkType::Selection => "Selection",
            MarkType::HoveredParsed => "Hovered parsed value",
            MarkType::HoveredParseErr => "Hovered parsing error",
        }
    }

    /// The name of the user mark if it is one.
    ///
    /// Returns `None` for non-user-marks and `Some(None)` for unnamed user marks.
    pub fn name(&self) -> Option<Option<&str>> {
        match &self {
            MarkType::UserMark { name } => Some((!name.is_empty()).then_some(name)),
            MarkType::SearchResult
            | MarkType::Selection
            | MarkType::HoveredParsed
            | MarkType::HoveredParseErr => None,
        }
    }
}

/// Implements a storage for hovered values that can be used form different places without disturbing the state.
///
/// The reason this is needed is because the order between the consumer of the value and the producer is not guaranteed.
/// It is therefore not clear who should be responsible for clearing the value when it's not hovered anymore and when.
/// With a single producer the producer itself can clear the hover before possibly setting it, but for multiple producers this stops working.
struct HoveredValue<T> {
    /// The current value.
    current_frame: Option<T>,
    /// The value that was produced in this frame so far.
    ///
    /// This will be live during the next frame.
    next_frame: Option<T>,
}

impl<T> HoveredValue<T> {
    /// Creates a new hovered value.
    fn new() -> HoveredValue<T> {
        HoveredValue {
            current_frame: None,
            next_frame: None,
        }
    }

    /// Sets the hovered value.
    fn set(&mut self, value: T) {
        self.next_frame = Some(value);
    }

    /// Returns the currently hovered value.
    fn get(&self) -> Option<&T> {
        self.current_frame.as_ref()
    }

    /// Performs the required book keeping at the end of a frame.
    fn end_of_frame(&mut self) {
        self.current_frame = self.next_frame.take();
    }
}

/// A store for marked locations.
pub struct MarkStore {
    /// The actual stores separated by mark type.
    per_type: BTreeMap<MarkType, SingleTypeStore>,
    /// The hovered location.
    hovered_location: HoveredValue<Mark>,
    /// The currently hovered parsed values.
    hovered_parsed_value: HoveredValue<Vec<Window>>,
    /// The currently hovered parse error.
    hovered_parse_err: HoveredValue<Vec<Window>>,
    /// The name of the current mark.
    pub current_mark_name: String,
}

impl MarkStore {
    /// Creates a new store for marked locations.
    pub fn new() -> MarkStore {
        MarkStore {
            per_type: BTreeMap::new(),
            hovered_location: HoveredValue::new(),
            hovered_parsed_value: HoveredValue::new(),
            hovered_parse_err: HoveredValue::new(),
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

    /// Whether there are any marks of th given type.
    pub fn contains_marks_of_type(&self, ty: MarkType) -> bool {
        match self.per_type.get(&ty) {
            Some(store) => !store.is_empty(),
            None => false,
        }
    }

    /// Converts all marks of the source type to the target type.
    pub fn convert_marks_to(&mut self, src_ty: MarkType, target_ty: MarkType) {
        let Some(old_store) = self.per_type.remove(&src_ty) else {
            return;
        };

        self.batch_add(old_store.iter(), target_ty);
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

    /// Iterates over all user marks.
    pub fn iter_user_marks<'store>(&'store self) -> impl Iterator<Item = MarkRef<'store>> {
        self.per_type
            .iter()
            .filter(|(ty, _)| matches!(ty, MarkType::UserMark { .. }))
            .flat_map(|(ty, store)| store.iter().map(|window| MarkRef { window, ty }))
    }

    /// Iterates over all marks of the given type.
    pub fn iter_marks_of_type<'store, 'ty: 'store>(
        &'store self,
        ty: &'ty MarkType,
    ) -> Option<impl Iterator<Item = MarkRef<'store>>> {
        let store = self.per_type.get(ty)?;

        Some(store.iter().map(|window| MarkRef { window, ty }))
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

    /// Returns the different types of marks currently present.
    pub fn types(&self) -> impl Iterator<Item = &MarkType> {
        self.per_type
            .iter()
            .filter_map(|(ty, store)| (!store.is_empty()).then_some(ty))
    }

    /// The total mark count.
    pub fn total_count(&self) -> usize {
        self.per_type.values().map(|store| store.len()).sum()
    }

    /// Returns the number of marks with the given type.
    pub fn count_of_type(&self, ty: &MarkType) -> usize {
        self.per_type.get(ty).map(|store| store.len()).unwrap_or(0)
    }

    /// Returns the hovered mark, if any.
    pub fn hovered(&self) -> Option<&Mark> {
        self.hovered_location.get()
    }

    /// Marks the given mark as hovered.
    pub fn mark_hovered(&mut self, mark: Mark) {
        self.hovered_location.set(mark);
    }

    /// Marks the given locations as the hovered parsed value.
    pub fn mark_hovered_parsed_value(&mut self, locations: Vec<Window>) {
        self.hovered_parsed_value.set(locations);
    }

    /// Marks the given locations as the hovered parse error.
    pub fn mark_hovered_parse_err(&mut self, locations: Vec<Window>) {
        self.hovered_parse_err.set(locations);
    }

    /// Marks the end of the frame, updating the marked location.
    pub fn end_of_frame(&mut self) {
        self.clear_marks_of_type(MarkType::HoveredParsed);
        self.clear_marks_of_type(MarkType::HoveredParseErr);

        self.hovered_location.end_of_frame();
        self.hovered_parsed_value.end_of_frame();
        self.hovered_parse_err.end_of_frame();

        if let Some(locations) = self.hovered_parsed_value.get() {
            // add manually to satify the borrow checker
            let store = self.per_type.entry(MarkType::HoveredParsed).or_default();
            store.extend(locations.iter().copied());
            store.consolidate();
        }
        if let Some(locations) = self.hovered_parse_err.get() {
            // add manually to satify the borrow checker
            let store = self.per_type.entry(MarkType::HoveredParseErr).or_default();
            store.extend(locations.iter().copied());
            store.consolidate();
        }
    }
}

impl Default for MarkStore {
    fn default() -> Self {
        MarkStore::new()
    }
}
