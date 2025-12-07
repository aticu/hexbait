//! Models "windows" as regions of the input.

use std::{fmt, ops::RangeInclusive};

use hexbait_common::{AbsoluteOffset, Len};
use size_format::SizeFormatterBinary;

/// Represents a region of the input.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Window {
    /// The index of the first byte in the region.
    start: AbsoluteOffset,
    /// The index one past the last byte in the region.
    end: AbsoluteOffset,
}

impl Window {
    /// Creates a new window.
    pub fn new(start: AbsoluteOffset, end: AbsoluteOffset) -> Window {
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
    pub fn from_start_len(start: AbsoluteOffset, len: Len) -> Window {
        Window {
            start,
            end: start + len,
        }
    }

    /// Creates an window at the given offset.
    pub fn empty_from_start(start: AbsoluteOffset) -> Window {
        Window { start, end: start }
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
    pub fn start(self) -> AbsoluteOffset {
        self.start
    }

    /// The end of the window.
    pub fn end(self) -> AbsoluteOffset {
        self.end
    }

    /// The size of the window in bytes.
    pub fn size(self) -> Len {
        self.end() - self.start()
    }

    /// Determines if the window is empty.
    pub fn is_empty(self) -> bool {
        self.start() == self.end()
    }

    /// Determines if the window contains the given offset.
    pub fn contains(self, offset: AbsoluteOffset) -> bool {
        self.start() <= offset && offset < self.end()
    }

    /// Determines if the window overlaps with the other window.
    pub fn overlaps(self, other: Window) -> bool {
        self.start() < other.end() && other.start() < self.end()
    }

    /// Returns the window as a [`RangeInclusive`] instead, if it is non-empty.
    pub fn range_inclusive(self) -> Option<RangeInclusive<AbsoluteOffset>> {
        if self.start() < self.end() {
            Some(self.start()..=(self.end() - Len::from(1)))
        } else {
            None
        }
    }

    /// Returns an iterator over smaller windows of the given size.
    ///
    /// `self.size()` must be a multiple of `size`.
    ///
    /// # Panics
    /// This function MAY panic if `self.size()` is not a multiple of `size`.
    pub fn subwindows_of_size(self, size: Len) -> impl Iterator<Item = Window> {
        debug_assert!(self.size().as_u64().is_multiple_of(size.as_u64()));

        (0..self.size().as_u64() / size.as_u64())
            .map(move |i| Window::from_start_len(self.start() + i * size, size))
    }

    /// Expands this window such that both the start and end are aligned to `align`.
    ///
    /// `align` must be a power of two.
    pub fn expand_to_align(self, align: u64) -> Window {
        let start = self.start().align_down(align);
        let end = self.end().align_up(align);

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
    ///
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
        let start = self.start().align_up(align);
        let end = self.end().align_down(align);

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

impl From<RangeInclusive<AbsoluteOffset>> for Window {
    fn from(value: RangeInclusive<AbsoluteOffset>) -> Self {
        Window::new(*value.start(), *value.end() + Len::from(1))
    }
}

impl fmt::Debug for Window {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Window(at: {}B ({:?}), size: {}B ({:?}))",
            SizeFormatterBinary::new(self.start().as_u64()),
            self.start(),
            SizeFormatterBinary::new(self.size().as_u64()),
            self.size(),
        )
    }
}
