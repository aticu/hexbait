//! Defines the views that are the source of the parsing.

use std::{
    fs,
    io::{self, Read as _, Seek as _, SeekFrom},
    ops::Range,
};

use super::provenance::Provenance;

/// A view describes a source that can be parsed from.
#[derive(Debug)]
pub enum View<'s> {
    /// Parses directly from the given file.
    File(&'s mut fs::File),
    /// Parses directly from the underlying bytes.
    Bytes(&'s [u8]),
    /// Parses out of a subview of a larger view.
    Subview {
        /// The view to parse from.
        view: &'s mut View<'s>,
        /// The range of the parent view that is valid.
        valid_range: Range<u64>,
    },
}

impl View<'_> {
    /// Returns the length of the view in bytes.
    pub(crate) fn len(&mut self) -> io::Result<u64> {
        match self {
            View::File(file) => file.seek(SeekFrom::End(0)),
            View::Bytes(bytes) => <[u8]>::len(bytes)
                .try_into()
                .map_err(|_| io::Error::other("length does not fit into `u64`")),
            View::Subview { view, valid_range } => {
                assert!(valid_range.end >= valid_range.start);

                let len = view.len()?;
                if len > valid_range.start {
                    Ok(0)
                } else {
                    Ok(std::cmp::min(len, valid_range.end) - valid_range.start)
                }
            }
        }
    }

    /// Reads data into the buffer at the given offset.
    pub(crate) fn read_at<'buf>(
        &mut self,
        offset: u64,
        buf: &'buf mut [u8],
    ) -> io::Result<&'buf [u8]> {
        let len = self.len()?;

        if offset > len {
            return Err(io::Error::other("offset is beyond input"));
        }

        let out_buf = match self {
            View::File(file) => {
                let len_left = len - offset;
                let output_size = std::cmp::min(len_left, buf.len().try_into().unwrap_or(u64::MAX));
                let truncated_buf = &mut buf[..output_size
                    .try_into()
                    .expect("we used min above, so this must fit into `buf`")];

                file.seek(SeekFrom::Start(offset))?;
                file.read_exact(truncated_buf)?;

                truncated_buf
            }
            View::Bytes(bytes) => {
                let len = bytes.len();
                let offset_usize: usize = offset
                    .try_into()
                    .map_err(|_| io::Error::other("offset does not fit into `usize`"))?;
                let len_left = len - offset_usize;
                let output_size = std::cmp::min(len_left, buf.len());

                buf[..output_size]
                    .copy_from_slice(&bytes[offset_usize..offset_usize + output_size]);

                &buf[..output_size]
            }
            View::Subview { view, valid_range } => {
                let buf_len = buf.len();
                view.read_at(
                    valid_range.start + offset,
                    &mut buf[..std::cmp::min(
                        usize::try_from(valid_range.end - offset).unwrap_or(usize::max_value()),
                        buf_len,
                    )],
                )?
            }
        };

        Ok(out_buf)
    }

    /// Creates a provenance for the view from the given range.
    pub(crate) fn provenance_from_range(&self, range: Range<u64>) -> Provenance {
        match self {
            View::File(_) | View::Bytes(_) => Provenance::from_range(range),
            View::Subview { valid_range, .. } => Provenance::from_range(
                range.start + valid_range.start..range.end + valid_range.start,
            ),
        }
    }
}
