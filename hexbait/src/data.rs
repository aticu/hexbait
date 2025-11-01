//! Models how the raw data is accessed in hexamine.

mod file;
mod input;
mod slice;

use std::fmt;

use hexbait_lang::View;
pub use input::Input;

/// A data source for hexamine to work with.
pub trait DataSource {
    /// The error type for fallible sources.
    type Error: fmt::Debug + fmt::Display;

    /// The length of the data.
    fn len(&mut self) -> Result<u64, Self::Error>;

    /// Determines if the data source is empty.
    fn is_empty(&mut self) -> Result<bool, Self::Error> {
        Ok(self.len()? == 0)
    }

    /// Fills the buffer with the data at the given offset in the data, returning the filled slice.
    fn window_at<'buf>(
        &mut self,
        offset: u64,
        buf: &'buf mut [u8],
    ) -> Result<&'buf [u8], Self::Error>;

    /// Returns the data source as a parsing view.
    fn as_view<'this>(&'this self) -> Result<View<'this>, Self::Error>;
}
