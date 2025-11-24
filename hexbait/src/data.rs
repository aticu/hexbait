//! Models how the raw data is accessed in hexamine.

use std::{
    fmt,
    fs::File,
    io::{self, Read as _, Seek as _, SeekFrom},
    path::PathBuf,
    sync::Arc,
};

use hexbait_common::{AbsoluteOffset, Len};
use hexbait_lang::View;

use crate::window::Window;

/// A data source for hexamine to work with.
pub trait DataSource {
    /// The error type for fallible sources.
    type Error: fmt::Debug + fmt::Display;

    /// The length of the data.
    fn len(&self) -> Len;

    /// Returns the window spanning the entire input.
    fn full_window(&self) -> Window {
        Window::from_start_len(AbsoluteOffset::ZERO, self.len())
    }

    /// Determines if the data source is empty.
    fn is_empty(&self) -> bool {
        self.len().is_zero()
    }

    /// Fills the buffer with the data at the given offset in the data, returning the filled slice.
    fn window_at<'buf>(
        &mut self,
        offset: AbsoluteOffset,
        buf: &'buf mut [u8],
    ) -> Result<&'buf [u8], Self::Error>;

    /// Returns the data source as a parsing view.
    fn as_view<'this>(&'this self) -> Result<View<'this>, Self::Error>;
}

/// The input file to examine.
#[derive(Debug)]
pub enum Input {
    /// The input is the given file.
    File {
        /// The path of the file.
        path: PathBuf,
        /// The open file handle.
        file: File,
        /// The length of the file in bytes.
        len: u64,
    },
    /// The input was read from stdin.
    Stdin(Arc<[u8]>),
}

impl Input {
    /// Clones the given input.
    pub fn clone(&self) -> io::Result<Input> {
        match self {
            Input::File { path, len, .. } => File::open(path).map(|file| Input::File {
                path: path.clone(),
                file,
                len: *len,
            }),
            Input::Stdin(buf) => Ok(Input::Stdin(Arc::clone(buf))),
        }
    }
}

impl DataSource for Input {
    type Error = io::Error;

    fn len(&self) -> Len {
        match self {
            Input::File { len, .. } => Len::from(*len),
            Input::Stdin(stdin) => Len::from(
                u64::try_from(stdin.len())
                    .expect("non `u64`-fitting length would not fit into memory"),
            ),
        }
    }

    fn window_at<'buf>(
        &mut self,
        offset: AbsoluteOffset,
        buf: &'buf mut [u8],
    ) -> Result<&'buf [u8], Self::Error> {
        match self {
            Input::File { file, len, .. } => {
                if offset.as_u64() > *len {
                    return Err(io::Error::other("offset is beyond input"));
                }

                let len_left = *len - offset.as_u64();
                let output_size = std::cmp::min(len_left, buf.len().try_into().unwrap_or(u64::MAX));
                let truncated_buf = &mut buf[..output_size
                    .try_into()
                    .expect("we used min above, so this must fit into `buf`")];

                file.seek(SeekFrom::Start(offset.as_u64()))?;
                file.read_exact(truncated_buf)?;

                Ok(truncated_buf)
            }
            Input::Stdin(stdin) => {
                let offset_usize: usize = offset
                    .as_u64()
                    .try_into()
                    .expect("offset does not fit into `usize`");

                if offset_usize > stdin.len() {
                    return Err(io::Error::other("offset is beyond input"));
                }

                let len_left = stdin.len() - offset_usize;
                let output_size = std::cmp::min(len_left, buf.len());

                buf[..output_size]
                    .copy_from_slice(&stdin[offset_usize..offset_usize + output_size]);

                Ok(&buf[..output_size])
            }
        }
    }

    fn as_view<'this>(&'this self) -> Result<View<'this>, Self::Error> {
        View::try_from(self)
    }
}

impl<'input> TryFrom<&'input Input> for View<'input> {
    type Error = io::Error;

    fn try_from(value: &'input Input) -> Result<View<'input>, Self::Error> {
        match value {
            Input::File { file, .. } => View::try_from(file),
            Input::Stdin(bytes) => Ok(View::from(&**bytes)),
        }
    }
}
