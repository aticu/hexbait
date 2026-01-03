//! Defines the views that are the source of the parsing.

use std::{io, ops::Range};

use hexbait_common::{Input, Len, RelativeOffset};

use super::provenance::Provenance;

/// A view describes a source that can be parsed from.
#[derive(Debug)]
pub enum View<'src> {
    Input(Input),
    /// Parses out of a subview of a larger view.
    Subview {
        /// The view to parse from.
        view: &'src View<'src>,
        /// The range of the parent view that is valid.
        valid_range: Range<RelativeOffset>,
    },
}

impl View<'_> {
    /// Returns the length of the view in bytes.
    pub fn len(&self) -> Len {
        match self {
            View::Input(input) => input.len(),
            View::Subview { view, valid_range } => {
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
    pub(crate) fn read_at<'buf>(
        &self,
        offset: RelativeOffset,
        buf: &'buf mut [u8],
    ) -> io::Result<&'buf [u8]> {
        if offset.as_u64() > self.len().as_u64() {
            return Err(io::Error::other("offset is beyond input"));
        }

        let out_buf = match self {
            View::Input(input) => input.window_at(offset.to_absolute(), buf)?,
            View::Subview { view, valid_range } => {
                let buf_len = buf.len();
                view.read_at(
                    valid_range.start + Len::from(offset.as_u64()),
                    &mut buf[..std::cmp::min(
                        usize::try_from((valid_range.end - offset).as_u64()).unwrap_or(usize::MAX),
                        buf_len,
                    )],
                )?
            }
        };

        Ok(out_buf)
    }

    /// Creates a provenance for the view from the given range.
    pub(crate) fn provenance_from_range(&self, range: Range<RelativeOffset>) -> Provenance {
        match self {
            View::Input(_) => {
                Provenance::from_range(range.start.to_absolute()..range.end.to_absolute())
            }
            View::Subview { view, valid_range } => view.provenance_from_range(
                range.start + Len::from(valid_range.start.as_u64())
                    ..range.end + Len::from(valid_range.start.as_u64()),
            ),
        }
    }
}
