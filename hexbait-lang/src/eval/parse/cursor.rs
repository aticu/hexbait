//! Implements a cursor for parsing.

use std::ops::Range;

use hexbait_common::{Endianness, Len, ReadBytes, RelativeOffset};

use crate::{
    Provenance, Span, View,
    parse::{Diagnostics, Result, diagnostics::SeekError},
};

/// Reads the specified number of bytes without advancing the cursor.
fn peek_bytes<'view>(
    view: &'view View,
    start: RelativeOffset,
    count: Len,
    span: Span,
    diagnostics: &mut Diagnostics,
) -> Result<(ReadBytes<'view>, Provenance)> {
    let err_provenance = || view.provenance_from_range(start..start + Len::from(1));

    if view.end_offset() < start + count {
        return Err(diagnostics.new_err("view is too short".into(), err_provenance(), span));
    }

    let buf = view
        .read_at(start, count)
        .map_err(|err| diagnostics.new_err(format!("io error: {err}"), err_provenance(), span))?;
    if buf.len() < count.as_u64() as usize {
        return Err(diagnostics.new_err("view is too short".into(), err_provenance(), span));
    }

    let provenance = view.provenance_from_range(start..start + count);

    Ok((buf, provenance))
}

/// Advances the cursor by the given count.
pub fn advance_by(
    offset: &mut RelativeOffset,
    count: Len,
    end: RelativeOffset,
) -> Result<(), SeekError> {
    if let Some(new_offset) = offset.checked_add(count) {
        if new_offset <= end {
            *offset = new_offset;
            Ok(())
        } else {
            Err(SeekError::SeekPastEnd {
                end,
                seek_offset: new_offset,
            })
        }
    } else {
        Err(SeekError::Overflow)
    }
}

/// A cursor for parsing.
#[derive(Debug, Clone)]
pub struct Cursor {
    /// The endianness used for parsing.
    endianness: Endianness,
    /// The current offset used for parsing.
    offset: RelativeOffset,
    /// The view that this scope parses from.
    view: View,
}

impl Cursor {
    /// Creates a new cursor.
    pub fn new(
        view: View,
        offset: RelativeOffset,
        endianness: Endianness,
    ) -> Result<Cursor, SeekError> {
        let end = view.end_offset();

        if offset <= end {
            Ok(Cursor {
                endianness,
                offset,
                view,
            })
        } else {
            Err(SeekError::SeekPastEnd {
                end,
                seek_offset: offset,
            })
        }
    }

    /// Creates a new child scope with the given view and offset.
    pub fn child_with_view_and_offset(
        &self,
        view: View,
        offset: RelativeOffset,
    ) -> Result<Cursor, SeekError> {
        let end = view.end_offset();

        if offset <= end {
            Ok(Cursor {
                endianness: self.endianness,
                view,
                offset,
            })
        } else {
            Err(SeekError::SeekPastEnd {
                end,
                seek_offset: offset,
            })
        }
    }

    /// Creates a child cursor in the same view with the given offset.
    pub fn child_with_same_view(&self, offset: RelativeOffset) -> Result<Cursor, SeekError> {
        let end = self.view.end_offset();

        if offset <= end {
            Ok(Cursor {
                endianness: self.endianness,
                view: self.view.clone(),
                offset,
            })
        } else {
            Err(SeekError::SeekPastEnd {
                end,
                seek_offset: offset,
            })
        }
    }

    /// The current offset of the cursor.
    pub fn offset(&self) -> RelativeOffset {
        self.offset
    }

    /// Advances the cursor by the given count.
    pub fn advance_by(&mut self, count: Len) -> Result<(), SeekError> {
        let end = self.view.end_offset();

        advance_by(&mut self.offset, count, end)
    }

    /// Sets the offset the cursor parses at.
    pub fn set_offset(&mut self, offset: RelativeOffset) -> Result<(), SeekError> {
        self.offset = self.probe_seek(offset)?;
        Ok(())
    }

    /// Returns the underlying view.
    pub fn view(&self) -> &View {
        &self.view
    }

    /// Sets the endianness of the cursor.
    pub fn set_endianness(&mut self, endianness: Endianness) {
        self.endianness = endianness;
    }

    /// Returns the endianness of the cursor.
    pub fn endianness(&self) -> &Endianness {
        &self.endianness
    }

    /// Peeks bytes at the given offset without modifying the cursor.
    pub fn peek_bytes(
        &self,
        start: RelativeOffset,
        count: Len,
        span: Span,
        diagnostics: &mut Diagnostics,
    ) -> Result<(ReadBytes<'_>, Provenance)> {
        peek_bytes(&self.view, start, count, span, diagnostics)
    }

    /// Reads the specified number of bytes, advancing the cursor.
    pub fn read_bytes_and_advance(
        &mut self,
        count: Len,
        span: Span,
        diagnostics: &mut Diagnostics,
    ) -> Result<(ReadBytes<'_>, Provenance)> {
        let end = self.view.end_offset();

        let result = peek_bytes(&self.view, self.offset(), count, span, diagnostics)?;

        advance_by(&mut self.offset, count, end).map_err(|err| {
            diagnostics.seek_err(err, &Provenance::empty(), span, "after reading")
        })?;

        Ok(result)
    }

    /// Checks if a seek to the given offset is possible.
    ///
    /// Returns the offset for convenience.
    pub fn probe_seek(&self, offset: RelativeOffset) -> Result<RelativeOffset, SeekError> {
        let end = self.view.end_offset();

        if offset <= end {
            Ok(offset)
        } else {
            Err(SeekError::SeekPastEnd {
                end,
                seek_offset: offset,
            })
        }
    }

    /// Returns the provenance for the given range in the cursor.
    pub fn provenance_from_range(&self, range: Range<RelativeOffset>) -> Provenance {
        self.view.provenance_from_range(range)
    }
}
