//! Defines new types for various quantities.

use std::{
    fmt::{self},
    ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Sub, SubAssign},
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

    /// Displays the length from the file start as a human-readable number.
    pub const fn human(self) -> impl fmt::Display {
        self.as_len().human()
    }

    /// Displays the length from the file start as a detailed and a human-readable number.
    pub const fn detailed(self) -> impl fmt::Display {
        self.as_len().detailed()
    }

    /// Aligns this offset up towards the given alignment.
    ///
    /// The alignment must be a power of two.
    ///
    /// # Panics
    /// This function MAY panic if the alignment is not a power of two.
    pub const fn align_up(self, align: Len) -> Self {
        Self(align_up(self.0, align.0))
    }

    /// Aligns this offset down towards the given alignment.
    ///
    /// The alignment must be a power of two.
    ///
    /// # Panics
    /// This function MAY panic if the alignment is not a power of two.
    pub const fn align_down(self, align: Len) -> Self {
        Self(align_down(self.0, align.0))
    }

    /// Determines if this offset is aligned to a given alignment.
    ///
    /// The alignment must be a power of two.
    ///
    /// # Panics
    /// This function MAY panic if the alignment is not a power of two.
    pub const fn is_aligned(self, align: Len) -> bool {
        is_aligned(self.0, align.0)
    }

    /// Returns this offset as a `u64`.
    pub const fn as_u64(self) -> u64 {
        self.0
    }

    /// Returns this offset as a relative offset to the start of the input.
    pub const fn to_relative(self) -> RelativeOffset {
        RelativeOffset(self.0)
    }

    /// Returns the length of bytes between the beginning of the input and this offset.
    pub const fn as_len(self) -> Len {
        Len(self.0)
    }
}

impl fmt::Debug for AbsoluteOffset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl fmt::Display for AbsoluteOffset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl fmt::LowerHex for AbsoluteOffset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl fmt::UpperHex for AbsoluteOffset {
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

    #[track_caller]
    fn sub(self, rhs: AbsoluteOffset) -> Self::Output {
        Len(self.0 - rhs.0)
    }
}

impl Add<RelativeOffset> for AbsoluteOffset {
    type Output = AbsoluteOffset;

    #[track_caller]
    fn add(self, rhs: RelativeOffset) -> Self::Output {
        AbsoluteOffset(self.0 + rhs.0)
    }
}

impl AddAssign<RelativeOffset> for AbsoluteOffset {
    #[track_caller]
    fn add_assign(&mut self, rhs: RelativeOffset) {
        self.0 += rhs.0;
    }
}

impl Add<Len> for AbsoluteOffset {
    type Output = AbsoluteOffset;

    #[track_caller]
    fn add(self, rhs: Len) -> Self::Output {
        AbsoluteOffset(self.0 + rhs.0)
    }
}

impl AddAssign<Len> for AbsoluteOffset {
    #[track_caller]
    fn add_assign(&mut self, rhs: Len) {
        self.0 += rhs.0;
    }
}

impl Sub<Len> for AbsoluteOffset {
    type Output = AbsoluteOffset;

    #[track_caller]
    fn sub(self, rhs: Len) -> Self::Output {
        AbsoluteOffset(self.0 - rhs.0)
    }
}

impl SubAssign<Len> for AbsoluteOffset {
    #[track_caller]
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

    /// Aligns this offset up towards the given alignment.
    ///
    /// The alignment must be a power of two.
    ///
    /// # Panics
    /// This function MAY panic if the alignment is not a power of two.
    pub const fn align_up(self, align: Len) -> Self {
        Self(align_up(self.0, align.0))
    }

    /// Aligns this offset down towards the given alignment.
    ///
    /// The alignment must be a power of two.
    ///
    /// # Panics
    /// This function MAY panic if the alignment is not a power of two.
    pub const fn align_down(self, align: Len) -> Self {
        Self(align_down(self.0, align.0))
    }

    /// Returns this offset as a `u64`.
    pub const fn as_u64(self) -> u64 {
        self.0
    }

    /// Adds a length to the offset if this would still result in a valid offset.
    #[must_use = "does not modify the value in place"]
    pub const fn checked_add(self, len: Len) -> Option<RelativeOffset> {
        // manual map to stay `const`
        match self.0.checked_add(len.0) {
            Some(result) => Some(RelativeOffset::from(result)),
            None => None,
        }
    }

    /// Subtracts the given length from this relative offset.
    ///
    /// Returns the length left after the subtraction if this would underflow the offset.
    pub const fn remove_len(&mut self, len: Len) -> Len {
        if self.0 >= len.0 {
            self.0 -= len.0;
            Len::ZERO
        } else {
            self.0 = 0;
            Len(len.0 - self.0)
        }
    }

    /// Adds the given length to this relative offset.
    ///
    /// Returns the length left if the offset after the addition would be larger than `max` and sets
    /// `self` to `max`.
    pub const fn add_len(&mut self, len: Len, max: RelativeOffset) -> Len {
        let end = self.0 as u128 + len.0 as u128;
        let max = max.0 as u128;

        // the `as u64` calls here cannot overflow because `max` came from a `u64`
        if end <= max {
            self.0 = end as u64;
            Len::ZERO
        } else {
            self.0 = max as u64;

            let len_left = end - max;
            if len_left > u64::MAX as u128 {
                // this should not happen, but clip here in case it does
                Len(u64::MAX)
            } else {
                Len(len_left as u64)
            }
        }
    }

    /// Returns this offset as an absolute offset.
    ///
    /// This is valid if the base that the offset is relative to is the beginning of the input.
    pub const fn to_absolute(self) -> AbsoluteOffset {
        AbsoluteOffset(self.0)
    }

    /// Returns the length of bytes between the beginning of the relative base and this offset.
    pub const fn as_len(self) -> Len {
        Len(self.0)
    }
}

impl fmt::Debug for RelativeOffset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl fmt::Display for RelativeOffset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl fmt::LowerHex for RelativeOffset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl fmt::UpperHex for RelativeOffset {
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

impl Sub<RelativeOffset> for RelativeOffset {
    type Output = Len;

    #[track_caller]
    fn sub(self, rhs: RelativeOffset) -> Self::Output {
        Len(self.0 - rhs.0)
    }
}

impl Sub<Len> for RelativeOffset {
    type Output = RelativeOffset;

    #[track_caller]
    fn sub(self, rhs: Len) -> Self::Output {
        RelativeOffset(self.0 - rhs.0)
    }
}

impl Add<Len> for RelativeOffset {
    type Output = RelativeOffset;

    #[track_caller]
    fn add(self, rhs: Len) -> Self::Output {
        RelativeOffset(self.0 + rhs.0)
    }
}

impl AddAssign<Len> for RelativeOffset {
    #[track_caller]
    fn add_assign(&mut self, rhs: Len) {
        self.0 += rhs.0;
    }
}

/// A length of a section of data.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Len(u64);

impl Len {
    /// A length of `0`.
    pub const ZERO: Len = Len::from(0);

    /// Creates a length from a `u64`.
    pub const fn from(len: u64) -> Len {
        Len(len)
    }

    /// Creates the given length in MiB.
    pub const fn mib(len_in_mib: u64) -> Len {
        Len(len_in_mib * 1024 * 1024)
    }

    /// Whether the length is `0`.
    pub const fn is_zero(self) -> bool {
        self.0 == 0
    }

    /// Returns this length as a `u64`.
    pub const fn as_u64(self) -> u64 {
        self.0
    }

    /// Subtracts the given length saturating to `Len::ZERO` on overflow.
    #[must_use = "does not modify the value in place"]
    pub const fn saturating_sub(self, other: Len) -> Len {
        Len(self.0.saturating_sub(other.0))
    }

    /// Returns the length that is the next smaller power of two from `self`.
    pub const fn prev_power_of_two(self) -> Len {
        if self.0.is_power_of_two() {
            self
        } else {
            Len(self.0.next_power_of_two() >> 1)
        }
    }

    /// Returns the absolute offset `length` bytes from the input start.
    pub const fn as_offset_from_start(self) -> AbsoluteOffset {
        AbsoluteOffset(self.0)
    }

    /// Returns the relative offset `length` bytes from the reference offset.
    pub const fn as_relative_offset(self) -> RelativeOffset {
        RelativeOffset(self.0)
    }

    /// Shows a human readable interpretation of the length.
    pub const fn human(self) -> impl fmt::Display {
        struct Display(u64);

        impl fmt::Display for Display {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}B", size_format::SizeFormatterBinary::new(self.0))
            }
        }

        Display(self.0)
    }

    /// Shows the detailed length and a human readable version of it.
    pub const fn detailed(self) -> impl fmt::Display {
        struct Display(Len);

        impl fmt::Display for Display {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}, {}", self.0, self.0.human())
            }
        }

        Display(self)
    }

    /// Determines if this length if a multiple of another length.
    pub const fn is_multiple_of(self, other: Len) -> bool {
        self.0.is_multiple_of(other.0)
    }

    /// Divides self by the other length rounding towards zero.
    pub const fn div_floor(self, other: Len) -> u64 {
        self.0 / other.0
    }

    /// Rounds this length up towards the given alignment.
    ///
    /// The alignment must be a power of two.
    ///
    /// # Panics
    /// This function MAY panic if the alignment is not a power of two.
    pub const fn round_up(self, align: Len) -> Self {
        Self(align_up(self.0, align.0))
    }

    /// Aligns this length down towards the given alignment.
    ///
    /// The alignment must be a power of two.
    ///
    /// # Panics
    /// This function MAY panic if the alignment is not a power of two.
    pub const fn align_down(self, align: Len) -> Self {
        Self(align_down(self.0, align.0))
    }
}

impl Add<Len> for Len {
    type Output = Len;

    #[track_caller]
    fn add(self, rhs: Len) -> Self::Output {
        Len(self.0 + rhs.0)
    }
}

impl AddAssign<Len> for Len {
    #[track_caller]
    fn add_assign(&mut self, rhs: Len) {
        self.0 += rhs.0;
    }
}

impl Mul<u64> for Len {
    type Output = Len;

    #[track_caller]
    fn mul(self, rhs: u64) -> Self::Output {
        Len(self.0 * rhs)
    }
}

impl Mul<Len> for u64 {
    type Output = Len;

    #[track_caller]
    fn mul(self, rhs: Len) -> Self::Output {
        Len(self * rhs.0)
    }
}

impl Mul<f32> for Len {
    type Output = Len;

    #[track_caller]
    fn mul(self, rhs: f32) -> Self::Output {
        Len((self.0 as f32 * rhs) as u64)
    }
}

impl Mul<Len> for f32 {
    type Output = Len;

    #[track_caller]
    fn mul(self, rhs: Len) -> Self::Output {
        rhs * self
    }
}

impl MulAssign<u64> for Len {
    #[track_caller]
    fn mul_assign(&mut self, rhs: u64) {
        self.0 *= rhs;
    }
}

impl Div<u64> for Len {
    type Output = Len;

    #[track_caller]
    fn div(self, rhs: u64) -> Self::Output {
        Len(self.0 / rhs)
    }
}

impl Div<f32> for Len {
    type Output = Len;

    #[track_caller]
    fn div(self, rhs: f32) -> Self::Output {
        Len((self.0 as f32 / rhs) as u64)
    }
}

impl Div<Len> for Len {
    type Output = f32;

    #[track_caller]
    fn div(self, rhs: Len) -> Self::Output {
        self.0 as f32 / rhs.0 as f32
    }
}

impl DivAssign<u64> for Len {
    #[track_caller]
    fn div_assign(&mut self, rhs: u64) {
        self.0 /= rhs;
    }
}

impl Sub<Len> for Len {
    type Output = Len;

    #[track_caller]
    fn sub(self, rhs: Len) -> Self::Output {
        Len(self.0 - rhs.0)
    }
}

impl SubAssign<Len> for Len {
    #[track_caller]
    fn sub_assign(&mut self, rhs: Len) {
        self.0 -= rhs.0;
    }
}

impl fmt::Debug for Len {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl fmt::Display for Len {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl fmt::LowerHex for Len {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl fmt::UpperHex for Len {
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
