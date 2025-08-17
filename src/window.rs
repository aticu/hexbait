//! Models "windows" as regions of the input.

use std::ops::RangeInclusive;

/// Represents a region of the input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Window {
    /// The index of the first byte in the region.
    start: u64,
    /// The index one past the last byte in the region.
    end: u64,
}

impl Window {
    /// Creates a new window.
    pub fn new(start: u64, end: u64) -> Window {
        if start < end {
            Window { start, end }
        } else {
            Window {
                start: end,
                end: start,
            }
        }
    }

    /// Creates a window from a start offset and a length.
    pub fn from_start_len(start: u64, len: u64) -> Window {
        Window {
            start,
            end: start + len,
        }
    }

    /// Creates the joined window between `self` and `other` if they are adjacent.
    pub fn joined(self, other: Window) -> Option<Window> {
        if self.end() == other.start() {
            Some(Window {
                start: self.start(),
                end: other.end(),
            })
        } else {
            None
        }
    }

    /// The start of the window.
    pub fn start(self) -> u64 {
        self.start
    }

    /// The end of the window.
    pub fn end(self) -> u64 {
        self.end
    }

    /// The size of the window in bytes.
    pub fn size(self) -> u64 {
        self.end() - self.start()
    }

    /// Determines if the window is empty.
    pub fn is_empty(self) -> bool {
        self.start() == self.end()
    }

    /// Determines if the window contains the given offset.
    pub fn contains(self, offset: u64) -> bool {
        self.start() <= offset && offset < self.end()
    }

    /// Determines if the window overlaps with the other window.
    pub fn overlaps(self, other: Window) -> bool {
        self.start() < other.end() && other.start() < self.end()
    }

    /// Returns the window as a [`RangeInclusive`] instead, if it is non-empty.
    pub fn range_inclusive(self) -> Option<RangeInclusive<u64>> {
        if self.start() < self.end() {
            Some(self.start()..=(self.end() - 1))
        } else {
            None
        }
    }

    /// Expands this window such that both the start and end are aligned to `align`.
    ///
    /// `align` must be a power of two.
    pub fn expand_to_align(self, align: u64) -> Window {
        let start = align_down(self.start(), align);
        let end = align_up(self.end(), align);

        Window { start, end }
    }

    /// Returns three subwindows `(before, aligned, after)`.
    ///
    /// The second returned window has alignment `align` for both its start and end and will be
    /// fully contained in the original window if it exists.
    /// It will also be the maximum size window that fulfills these conditions.
    /// Any of the windows may be empty.
    /// All windows joined together will span the whole original window.
    ///
    /// `None` is returned if no aligned subwindow exists within `self`.
    ///
    /// `align` must be a power of two.
    ///
    /// # Example
    /// ```rust
    /// # use hexbait::window::Window;
    /// assert_eq!(
    ///     Window::new(3, 25).align(8),
    ///     Some((
    ///         Window::new(3, 8),
    ///         Window::new(8, 24),
    ///         Window::new(24, 25),
    ///     ))
    /// );
    /// assert_eq!(
    ///     Window::new(3, 8).align(8),
    ///     Some((
    ///         Window::new(3, 8),
    ///         Window::new(8, 8),
    ///         Window::new(8, 8),
    ///     ))
    /// );
    /// assert_eq!(
    ///     Window::new(7, 11).align(8),
    ///     Some((
    ///         Window::new(7, 8),
    ///         Window::new(8, 8),
    ///         Window::new(8, 11),
    ///     ))
    /// );
    /// assert_eq!(Window::new(3, 25).align(32), None);
    /// ```
    pub fn align(self, align: u64) -> Option<(Window, Window, Window)> {
        let start = align_up(self.start(), align);
        let end = align_down(self.end(), align);

        if start <= end {
            Some((
                Window {
                    start: self.start(),
                    end: start,
                },
                Window { start, end },
                Window {
                    start: end,
                    end: self.end(),
                },
            ))
        } else {
            None
        }
    }
}

impl From<RangeInclusive<u64>> for Window {
    fn from(value: RangeInclusive<u64>) -> Self {
        Window::new(*value.start(), value.end() + 1)
    }
}

/// Aligns the given number towards the maximum value.
///
/// `align` must be a power of two.
const fn align_up(num: u64, align: u64) -> u64 {
    align_down(num + (align - 1), align)
}

/// Aligns the given number towards zero.
///
/// `align` must be a power of two.
const fn align_down(num: u64, align: u64) -> u64 {
    num & !(align - 1)
}
