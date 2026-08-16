//! Implements things shared by the modules rendering bars.

use hexbait_common::{AbsoluteOffset, Len};

/// The width of the side bar.
pub const SIDE_BAR_WIDTH: usize = 4;

/// How far apart alignment markers need to be to be large.
pub const LARGE_ALIGNMENT_MARKER_DIFF: u64 = 10;

/// Returns the value in `(start, end]` with the highest power-of-two * 10 alignment.
pub fn highest_aligned_value(start: AbsoluteOffset, end: AbsoluteOffset) -> Len {
    let start = start.as_u64();
    let end = end.as_u64();
    for k in (1..=6).rev() {
        let shift = 10 * k;
        let candidate = (end >> shift) << shift; // largest multiple of 1024^k that is <= end
        if candidate > start {
            return Len::from(1 << shift);
        }
    }
    Len::from(1)
}
