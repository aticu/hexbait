//! Implements things shared by the modules rendering bars.

/// The width of the side bar.
pub const SIDE_BAR_WIDTH: usize = 4;

/// How far apart alignment markers need to be to be large.
pub const LARGE_ALIGNMENT_MARKER_DIFF: u64 = 10;

/// Returns the value in `(start, end]` with the highest power-of-two * 10 alignment.
pub fn highest_aligned_value(start: u64, end: u64) -> u64 {
    for k in (1..=6).rev() {
        let shift = 10 * k;
        let candidate = (end >> shift) << shift; // largest multiple of 1024^k that is <= end
        if candidate > start {
            return 1 << shift;
        }
    }
    1
}
