//! Defines the views that are the source of the parsing.

use std::{io, ops::Range, sync::Arc};

use hexbait_common::{Input, Len, ReadBytes, RelativeOffset};

use super::provenance::Provenance;

#[derive(Debug, Clone)]
pub struct View(Arc<ViewType>);

/// A view describes a source that can be parsed from.
#[derive(Debug, Clone)]
enum ViewType {
    Input(Input),
    /// Parses out of a subview of a larger view.
    Subview {
        /// The view to parse from.
        view: View,
        /// The range of the parent view that is valid.
        valid_range: Range<RelativeOffset>,
    },
}

impl View {
    /// Creates a view of the input.
    pub fn from_input(input: Input) -> View {
        View(Arc::new(ViewType::Input(input)))
    }

    /// Creates a subview with the given range in the current view.
    ///
    /// This function does not check any bounds, so the view may be invalid.
    pub fn subview(&self, range: Range<RelativeOffset>) -> View {
        View(Arc::new(ViewType::Subview {
            view: self.clone(),
            valid_range: range,
        }))
    }

    /// Returns the length of the view in bytes.
    pub fn len(&self) -> Len {
        match &*self.0 {
            ViewType::Input(input) => input.len(),
            ViewType::Subview { view, valid_range } => {
                assert!(valid_range.end >= valid_range.start);

                let len = view.len();
                if valid_range.start.as_u64() > len.as_u64() {
                    Len::ZERO
                } else {
                    Len::from(
                        std::cmp::min(len.as_u64(), valid_range.end.as_u64())
                            - valid_range.start.as_u64(),
                    )
                }
            }
        }
    }

    /// Returns `true` if the view is empty.
    pub fn is_empty(&self) -> bool {
        self.len().is_zero()
    }

    /// Reads data into the buffer at the given offset.
    pub(crate) fn read_at(&self, offset: RelativeOffset, len: Len) -> io::Result<ReadBytes<'_>> {
        if offset.as_u64() > self.len().as_u64() {
            return Err(io::Error::other("offset is beyond input"));
        }

        let out_buf = match &*self.0 {
            ViewType::Input(input) => input.read_at(offset.to_absolute(), len, None)?,
            ViewType::Subview { view, valid_range } => {
                view.read_at(valid_range.start + Len::from(offset.as_u64()), len)?
            }
        };

        Ok(out_buf)
    }

    /// Creates a provenance for the view from the given range.
    pub(crate) fn provenance_from_range(&self, range: Range<RelativeOffset>) -> Provenance {
        match &*self.0 {
            ViewType::Input(_) => {
                Provenance::from_range(range.start.to_absolute()..range.end.to_absolute())
            }
            ViewType::Subview { view, valid_range } => view.provenance_from_range(
                range.start + Len::from(valid_range.start.as_u64())
                    ..range.end + Len::from(valid_range.start.as_u64()),
            ),
        }
    }
}
