//! Implement [`std::fs::File`] as a data source.

use std::{
    fs::File,
    io::{Error, Read as _, Seek as _, SeekFrom},
};

use super::DataSource;

impl DataSource for File {
    type Error = Error;

    fn len(&mut self) -> Result<u64, Self::Error> {
        self.seek(SeekFrom::End(0))
    }

    fn window_at<'buf>(
        &mut self,
        offset: u64,
        buf: &'buf mut [u8],
    ) -> Result<&'buf [u8], Self::Error> {
        let len = self.len()?;

        if offset > len {
            return Err(Error::new(
                std::io::ErrorKind::Other,
                "offset is beyond input",
            ));
        }

        let len_left = len - offset;
        let output_size = std::cmp::min(len_left, buf.len().try_into().unwrap_or(u64::MAX));
        let truncated_buf = &mut buf[..output_size
            .try_into()
            .expect("we used min above, so this must fit into `buf`")];

        self.seek(SeekFrom::Start(offset))?;
        self.read_exact(truncated_buf)?;

        Ok(truncated_buf)
    }
}
