//! Handles the input to the program.

use std::{fs::File, io};

use super::DataSource;

/// The input file to examine.
pub enum Input {
    /// The input is the given file.
    File(File),
    /// The input was read from stdin.
    Stdin(Vec<u8>),
}

impl DataSource for Input {
    type Error = io::Error;

    fn len(&mut self) -> Result<u64, Self::Error> {
        match self {
            Input::File(file) => file.len(),
            Input::Stdin(stdin) => {
                let mut as_ref = &**stdin;
                <&[u8] as DataSource>::len(&mut as_ref)
                    .map_err(|err| io::Error::new(io::ErrorKind::Other, err))
            }
        }
    }

    fn window_at<'buf>(
        &mut self,
        offset: u64,
        buf: &'buf mut [u8],
    ) -> Result<&'buf [u8], Self::Error> {
        match self {
            Input::File(file) => file.window_at(offset, buf),
            Input::Stdin(stdin) => {
                let mut as_ref = &**stdin;
                <&[u8] as DataSource>::window_at(&mut as_ref, offset, buf)
                    .map_err(|err| io::Error::new(io::ErrorKind::Other, err))
            }
        }
    }
}
