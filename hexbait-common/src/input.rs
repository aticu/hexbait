//! Models how the raw data is accessed in hexamine.

use std::{io, path::PathBuf, sync::Arc};

use positioned_io::{RandomAccessFile, ReadAt as _, Size as _};

use crate::{AbsoluteOffset, Len};

#[derive(Debug, Clone)]
pub struct Input(Arc<InputType>);

/// The input file to examine.
#[derive(Debug)]
enum InputType {
    /// The input is the given file.
    File {
        /// The open file handle.
        file: RandomAccessFile,
        /// The length of the file in bytes.
        len: u64,
    },
    /// The input was read from stdin.
    Stdin(Box<[u8]>),
}

impl Input {
    /// Creates an input from the given path.
    pub fn from_path(path: impl Into<PathBuf>) -> io::Result<Input> {
        let path = path.into();

        let file = positioned_io::RandomAccessFile::open(&path).unwrap();
        let len = file
            .size()?
            .ok_or_else(|| io::Error::other("cannot get file size"))?;

        Ok(Input(Arc::new(InputType::File { file, len })))
    }

    /// Creates an input from stdin.
    ///
    /// This should only be called once since it consumes stdin.
    pub fn from_stdin() -> io::Result<Input> {
        let mut buf = Vec::new();
        io::Read::read_to_end(&mut io::stdin(), &mut buf)?;

        Ok(Input(Arc::new(InputType::Stdin(buf.into()))))
    }

    /// The length of the data.
    pub fn len(&self) -> Len {
        match &*self.0 {
            InputType::File { len, .. } => Len::from(*len),
            InputType::Stdin(stdin) => Len::from(
                u64::try_from(stdin.len())
                    .expect("non `u64`-fitting length would not fit into memory"),
            ),
        }
    }

    /// Determines if the input is empty.
    pub fn is_empty(&self) -> bool {
        self.len().is_zero()
    }

    /// Fills the buffer with the data at the given offset in the input, returning the filled slice.
    pub fn window_at<'buf>(
        &self,
        offset: AbsoluteOffset,
        buf: &'buf mut [u8],
    ) -> Result<&'buf [u8], io::Error> {
        match &*self.0 {
            InputType::File { file, len, .. } => {
                if offset.as_u64() > *len {
                    return Err(io::Error::other("offset is beyond input"));
                }

                let len_left = *len - offset.as_u64();
                let output_size = std::cmp::min(len_left, buf.len().try_into().unwrap_or(u64::MAX));
                let truncated_buf = &mut buf[..output_size
                    .try_into()
                    .expect("we used min above, so this must fit into `buf`")];

                file.read_exact_at(offset.as_u64(), truncated_buf)?;

                Ok(truncated_buf)
            }
            InputType::Stdin(stdin) => {
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
}
