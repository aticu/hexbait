//! Defines the views that are the source of the parsing.

use std::{
    fs,
    io::{self, Read as _, Seek as _, SeekFrom},
    ops::Range,
};

use super::provenance::Provenance;

/// A view describes a source that can be parsed from.
#[derive(Debug)]
pub enum View<'src> {
    /// Parses directly from the given file.
    File {
        /// The file to read from.
        file: &'src fs::File,
        /// The length of the file.
        ///
        /// This assumes that this will never change.
        len: u64,
    },
    /// Parses directly from the underlying bytes.
    Bytes(&'src [u8]),
    /// Parses out of a subview of a larger view.
    Subview {
        /// The view to parse from.
        view: &'src View<'src>,
        /// The range of the parent view that is valid.
        valid_range: Range<u64>,
    },
}

impl View<'_> {
    /// Returns the length of the view in bytes.
    pub fn len(&self) -> u64 {
        match self {
            View::File { len, .. } => *len,
            View::Bytes(bytes) => {
                u64::try_from(bytes.len()).expect("length of in memory array fits into `u64`")
            }
            View::Subview { view, valid_range } => {
                assert!(valid_range.end >= valid_range.start);

                let len = view.len();
                if valid_range.start > len {
                    0
                } else {
                    std::cmp::min(len, valid_range.end) - valid_range.start
                }
            }
        }
    }

    /// Returns `true` if the view is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Reads data into the buffer at the given offset.
    pub(crate) fn read_at<'buf>(&self, offset: u64, buf: &'buf mut [u8]) -> io::Result<&'buf [u8]> {
        if offset > self.len() {
            return Err(io::Error::other("offset is beyond input"));
        }

        let out_buf = match self {
            View::File { file, len } => {
                let mut file = &**file;

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
                        usize::try_from(valid_range.end - offset).unwrap_or(usize::MAX),
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
            View::File { .. } | View::Bytes(_) => Provenance::from_range(range),
            View::Subview { view, valid_range } => view.provenance_from_range(
                range.start + valid_range.start..range.end + valid_range.start,
            ),
        }
    }
}

impl<'src> TryFrom<&'src fs::File> for View<'src> {
    type Error = io::Error;

    fn try_from(file: &'src fs::File) -> Result<Self, Self::Error> {
        let len = (&*file).seek(SeekFrom::End(0))?;

        Ok(View::File { file, len })
    }
}

impl<'src> From<&'src [u8]> for View<'src> {
    fn from(bytes: &'src [u8]) -> Self {
        View::Bytes(bytes)
    }
}
