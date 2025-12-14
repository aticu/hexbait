//! Defines common types and functions used by all hexbait `crate`s.

pub use endianness::Endianness;
pub use quantities::{AbsoluteOffset, Len, RelativeOffset};

mod endianness;
mod quantities;

/// Indicates whether something changed or remained the same between frames.
pub enum ChangeState {
    /// The state in question remained unchanged.
    Unchanged,
    /// The state in question changed.
    Changed,
}
