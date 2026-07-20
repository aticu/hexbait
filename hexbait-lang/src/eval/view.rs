//! Defines the views that are the source of the parsing.

use std::{io, ops::Range, sync::Arc};

use hexbait_common::{Input, Len, ReadBytes, RelativeOffset};

use crate::BytesValue;

use super::provenance::Provenance;

/// A view is the input to the parser.
#[derive(Debug, Clone)]
pub struct View(Arc<ViewType>);

/// A view describes a source that can be parsed from.
#[derive(Debug, Clone)]
enum ViewType {
    /// Parses out of the raw given input.
    ///
    /// This is the root-level view.
    Input(Input),
    /// Parses out of a subview of a larger view.
    Subview {
        /// The view to parse from.
        view: View,
        /// The range of the parent view that is valid.
        valid_range: Range<RelativeOffset>,
    },
    /// Parses out of the given bytes.
    Bytes(BytesValue),
}

impl View {
    /// Creates a view of the input.
    pub fn from_input(input: Input) -> View {
        View(Arc::new(ViewType::Input(input)))
    }

    /// Creates a view from bytes.
    pub fn from_bytes(bytes: BytesValue) -> View {
        View(Arc::new(ViewType::Bytes(bytes)))
    }

    /// Creates a subview with the given range in the current view.
    ///
    /// This function does not check any bounds, so the view may be invalid.
    pub fn subview(&self, range: Range<RelativeOffset>) -> View {
        if let ViewType::Subview { view, valid_range } = &*self.0 {
            // avoid long chains of sub-views to improve read performance
            let offset = Len::from(valid_range.start.as_u64());

            let start = range.start + offset;
            let end = std::cmp::min(range.end + offset, valid_range.end);
            let valid_range = start..end;

            View(Arc::new(ViewType::Subview {
                view: view.clone(),
                valid_range,
            }))
        } else {
            View(Arc::new(ViewType::Subview {
                view: self.clone(),
                valid_range: range,
            }))
        }
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
            ViewType::Bytes(bytes) => Len::from(bytes.len() as u64),
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
            ViewType::Bytes(bytes) => {
                let mut out = vec![0; len.as_u64() as usize];

                bytes.fill_buf_at(offset.as_u64() as usize, &mut out)?;

                ReadBytes::from_vec(out)
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
            ViewType::Bytes(bytes) => bytes.provenance_range(range),
        }
    }
}
