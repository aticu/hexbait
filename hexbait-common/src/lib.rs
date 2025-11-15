//! Defines common types and functions used by all hexbait `crate`s.

use std::{
    fmt,
    ops::{Add, AddAssign, Mul, MulAssign, Sub, SubAssign},
};

/// Defines an absolute offset into a file.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AbsoluteOffset(u64);

impl AbsoluteOffset {
    /// An absolute offset of `0`, representing the beginning of the file.
    pub const ZERO: AbsoluteOffset = AbsoluteOffset::from(0);

    /// Creates an absolute offset from a `u64`.
    pub const fn from(offset: u64) -> AbsoluteOffset {
        AbsoluteOffset(offset)
    }

    /// Whether the offset refers to the start of the file.
    pub const fn is_start_of_file(self) -> bool {
        self.0 == 0
    }

    /// Aligns this offset up towards the given alignment.
    ///
    /// The alignment must be a power of two.
    ///
    /// # Panics
    /// This function MAY panic if the alignment is not a power of two.
    pub const fn align_up(self, align: u64) -> Self {
        Self(align_up(self.0, align))
    }

    /// Aligns this offset down towards the given alignment.
    ///
    /// The alignment must be a power of two.
    ///
    /// # Panics
    /// This function MAY panic if the alignment is not a power of two.
    pub const fn align_down(self, align: u64) -> Self {
        Self(align_down(self.0, align))
    }

    /// Determines if this offset is aligned to a given alignment.
    ///
    /// The alignment must be a power of two.
    ///
    /// # Panics
    /// This function MAY panic if the alignment is not a power of two.
    pub const fn is_aligned(self, align: u64) -> bool {
        is_aligned(self.0, align)
    }

    /// Returns this offset as a `u64`.
    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

impl fmt::Debug for AbsoluteOffset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<T> From<T> for AbsoluteOffset
where
    u64: From<T>,
{
    fn from(offset: T) -> Self {
        AbsoluteOffset::from(u64::from(offset))
    }
}

impl Sub<AbsoluteOffset> for AbsoluteOffset {
    type Output = Len;

    fn sub(self, rhs: AbsoluteOffset) -> Self::Output {
        Len(self.0 - rhs.0)
    }
}

impl Add<RelativeOffset> for AbsoluteOffset {
    type Output = AbsoluteOffset;

    fn add(self, rhs: RelativeOffset) -> Self::Output {
        AbsoluteOffset(self.0 + rhs.0)
    }
}

impl AddAssign<RelativeOffset> for AbsoluteOffset {
    fn add_assign(&mut self, rhs: RelativeOffset) {
        self.0 += rhs.0;
    }
}

impl Add<Len> for AbsoluteOffset {
    type Output = AbsoluteOffset;

    fn add(self, rhs: Len) -> Self::Output {
        AbsoluteOffset(self.0 + rhs.0)
    }
}

impl AddAssign<Len> for AbsoluteOffset {
    fn add_assign(&mut self, rhs: Len) {
        self.0 += rhs.0;
    }
}

impl Sub<Len> for AbsoluteOffset {
    type Output = AbsoluteOffset;

    fn sub(self, rhs: Len) -> Self::Output {
        AbsoluteOffset(self.0 - rhs.0)
    }
}

impl SubAssign<Len> for AbsoluteOffset {
    fn sub_assign(&mut self, rhs: Len) {
        self.0 -= rhs.0;
    }
}

/// An offset that is relative to some other offset.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RelativeOffset(u64);

impl RelativeOffset {
    /// A relative offset of `0`.
    pub const ZERO: RelativeOffset = RelativeOffset::from(0);

    /// Creates a relative offset from a `u64`.
    pub const fn from(offset: u64) -> RelativeOffset {
        RelativeOffset(offset)
    }

    /// Returns this offset as a `u64`.
    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

impl fmt::Debug for RelativeOffset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<T> From<T> for RelativeOffset
where
    u64: From<T>,
{
    fn from(offset: T) -> Self {
        RelativeOffset::from(u64::from(offset))
    }
}

/// A length of a section of data.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Len(u64);

impl Len {
    /// A length of `0`.
    pub const ZERO: Len = Len::from(0);

    /// Creates a length from a `u64`.
    pub const fn from(offset: u64) -> Len {
        Len(offset)
    }

    /// Whether the length is `0`.
    pub const fn is_zero(self) -> bool {
        self.0 == 0
    }

    /// Returns this length as a `u64`.
    pub const fn as_u64(self) -> u64 {
        self.0
    }

    /// Rounds this length up towards the given alignment.
    ///
    /// The alignment must be a power of two.
    ///
    /// # Panics
    /// This function MAY panic if the alignment is not a power of two.
    pub const fn round_up(self, align: u64) -> Self {
        Self(align_up(self.0, align))
    }
}

impl Add<Len> for Len {
    type Output = Len;

    fn add(self, rhs: Len) -> Self::Output {
        Len(self.0 + rhs.0)
    }
}

impl AddAssign<Len> for Len {
    fn add_assign(&mut self, rhs: Len) {
        self.0 += rhs.0;
    }
}

impl Mul<u64> for Len {
    type Output = Len;

    fn mul(self, rhs: u64) -> Self::Output {
        Len(self.0 * rhs)
    }
}

impl Mul<Len> for u64 {
    type Output = Len;

    fn mul(self, rhs: Len) -> Self::Output {
        Len(self * rhs.0)
    }
}

impl MulAssign<u64> for Len {
    fn mul_assign(&mut self, rhs: u64) {
        self.0 *= rhs;
    }
}

impl Sub<Len> for Len {
    type Output = Len;

    fn sub(self, rhs: Len) -> Self::Output {
        Len(self.0 - rhs.0)
    }
}

impl SubAssign<Len> for Len {
    fn sub_assign(&mut self, rhs: Len) {
        self.0 -= rhs.0;
    }
}

impl fmt::Debug for Len {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<T> From<T> for Len
where
    u64: From<T>,
{
    fn from(offset: T) -> Self {
        Len::from(u64::from(offset))
    }
}

/// Aligns the given number towards the maximum value.
///
/// `align` must be a power of two.
///
/// # Panics
/// This function MAY panic if the alignment is not a power of two.
const fn align_up(num: u64, align: u64) -> u64 {
    debug_assert!(align.is_power_of_two());
    align_down(num + (align - 1), align)
}

/// Aligns the given number towards zero.
///
/// `align` must be a power of two.
///
/// # Panics
/// This function MAY panic if the alignment is not a power of two.
const fn align_down(num: u64, align: u64) -> u64 {
    debug_assert!(align.is_power_of_two());
    num & !(align - 1)
}

/// Determines if the given number is aligned.
///
/// `align` must be a power of two.
///
/// # Panics
/// This function MAY panic if the alignment is not a power of two.
const fn is_aligned(num: u64, align: u64) -> bool {
    debug_assert!(align.is_power_of_two());
    num & (align - 1) == 0
}
