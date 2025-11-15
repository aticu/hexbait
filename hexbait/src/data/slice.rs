//! Implements slices as a data source.

use hexbait_common::{AbsoluteOffset, Len};
use hexbait_lang::View;

use super::DataSource;

impl DataSource for &[u8] {
    type Error = &'static str;

    fn len(&mut self) -> Result<Len, Self::Error> {
        u64::try_from(<[u8]>::len(self))
            .map_err(|_| "length does not fit into `u64`")
            .map(Len::from)
    }

    fn window_at<'buf>(
        &mut self,
        offset: AbsoluteOffset,
        buf: &'buf mut [u8],
    ) -> Result<&'buf [u8], Self::Error> {
        let offset_usize: usize = offset
            .as_u64()
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
