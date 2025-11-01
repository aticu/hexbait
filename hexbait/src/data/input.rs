//! Handles the input to the program.

use std::{fs::File, io, path::PathBuf, sync::Arc};

use hexbait_lang::View;

use super::DataSource;

/// The input file to examine.
#[derive(Debug)]
pub enum Input {
    /// The input is the given file.
    File {
        /// The path of the file.
        path: PathBuf,
        /// The open file handle.
        file: File,
    },
    /// The input was read from stdin.
    Stdin(Arc<[u8]>),
}

impl Input {
    /// Clones the given input.
    pub fn clone(&self) -> io::Result<Input> {
        match self {
            Input::File { path, .. } => File::open(path).map(|file| Input::File {
                path: path.clone(),
                file,
            }),
            Input::Stdin(buf) => Ok(Input::Stdin(Arc::clone(buf))),
        }
    }
}

impl DataSource for Input {
    type Error = io::Error;

    fn len(&mut self) -> Result<u64, Self::Error> {
        match self {
            Input::File { file, .. } => file.len(),
            Input::Stdin(stdin) => {
                let mut as_ref = &**stdin;
                <&[u8] as DataSource>::len(&mut as_ref).map_err(io::Error::other)
            }
        }
    }

    fn window_at<'buf>(
        &mut self,
        offset: u64,
        buf: &'buf mut [u8],
    ) -> Result<&'buf [u8], Self::Error> {
        match self {
            Input::File { file, .. } => file.window_at(offset, buf),
            Input::Stdin(stdin) => {
                let mut as_ref = &**stdin;
                <&[u8] as DataSource>::window_at(&mut as_ref, offset, buf).map_err(io::Error::other)
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
