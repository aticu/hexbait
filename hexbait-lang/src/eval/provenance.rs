//! Implements tracking where values originated.

use std::ops::{Add, AddAssign, Range, RangeInclusive};

use range_set_blaze::RangeSetBlaze;

/// Tracks where parsed values originated.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Provenance {
    /// The byte ranges where the tracked value originated.
    byte_ranges: RangeSetBlaze<u64>,
}

impl Provenance {
    /// Creates a new empty provenance.
    pub fn empty() -> Provenance {
        Provenance {
            byte_ranges: RangeSetBlaze::new(),
        }
    }

    /// Creates a new provenance from the give window.
    pub fn from_range(range: Range<u64>) -> Provenance {
        let mut byte_ranges = RangeSetBlaze::new();
        if !range.is_empty() {
            byte_ranges.ranges_insert(range.start..=range.end - 1);
        }

        Provenance { byte_ranges }
    }

    /// Returns whether the provenance is empty.
    ///
    /// This is the case if no bytes of the input were used to arrive at the value.
    /// One example of empty provenance values are values that are constants in the parser
    /// description.
    pub fn is_empty(&self) -> bool {
        self.byte_ranges.is_empty()
    }

    /// Returns an iterator over the byte ranges that make up this provenance.
    pub fn byte_ranges(&self) -> impl Iterator<Item = RangeInclusive<u64>> {
        self.byte_ranges.ranges()
    }
}

impl From<Range<u64>> for Provenance {
    fn from(value: Range<u64>) -> Self {
        Provenance::from_range(value)
    }
}

impl Default for Provenance {
    fn default() -> Self {
        Provenance::empty()
    }
}

impl Add for &Provenance {
    type Output = Provenance;

    #[allow(clippy::suspicious_arithmetic_impl)]
    fn add(self, rhs: &Provenance) -> Self::Output {
        Provenance {
            byte_ranges: &self.byte_ranges | &rhs.byte_ranges,
        }
    }
}

impl AddAssign<&Provenance> for Provenance {
    #[allow(clippy::suspicious_op_assign_impl)]
    fn add_assign(&mut self, rhs: &Self) {
        self.byte_ranges |= &rhs.byte_ranges;
    }
}
