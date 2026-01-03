//! Defines common types and functions used by all hexbait `crate`s.

pub use endianness::Endianness;
pub use input::Input;
pub use quantities::{AbsoluteOffset, Len, RelativeOffset};

mod endianness;
mod input;
mod quantities;

/// Indicates whether something changed or remained the same between frames.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum StateChangeFlag {
    /// The state in question remained unchanged.
    Unchanged,
    /// The state in question changed.
    Changed,
}

impl StateChangeFlag {
    /// Whether the state was changed.
    pub fn is_changed(self) -> bool {
        self == StateChangeFlag::Changed
    }
}
