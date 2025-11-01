//! Implements slices as a data source.

use hexbait_lang::View;

use super::DataSource;

impl DataSource for &[u8] {
    type Error = &'static str;

    fn len(&mut self) -> Result<u64, Self::Error> {
        <[u8]>::len(self)
            .try_into()
            .map_err(|_| "length does not fit into `u64`")
    }

    fn window_at<'buf>(
        &mut self,
        offset: u64,
        buf: &'buf mut [u8],
    ) -> Result<&'buf [u8], Self::Error> {
        let offset_usize: usize = offset
            .try_into()
            .map_err(|_| "offset does not fit into `usize`")?;

        let len = <[u8]>::len(self);

        if offset_usize > len {
            return Err("offset is beyond input");
        }

        let len_left = len - offset_usize;
        let output_size = std::cmp::min(len_left, buf.len());

        buf[..output_size].copy_from_slice(&self[offset_usize..offset_usize + output_size]);

        Ok(&buf[..output_size])
    }

    fn as_view<'this>(&'this self) -> Result<View<'this>, Self::Error> {
        Ok(View::from(*self))
    }
}
