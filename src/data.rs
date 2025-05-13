//! Models how the raw data is accessed in hexamine.

mod file;
mod slice;

/// A data source for hexamine to work with.
pub trait DataSource {
    /// The error type for fallible sources.
    type Error;

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
}
