//! Handles all code for deciding cache sizes.

use super::CacheSize;

/// Generates cache sizes from a specification.
///
/// Example specification:
///
/// ```
/// # use hexbait::cache_sizes;
/// cache_sizes! {
///     CacheSize {
///         32 KiB with 256 entries,
///         1 MiB with 256 entries,
///         128 MiB with 256 entries,
///         2 GiB with 256 entries,
///     }
/// }
/// ```
#[macro_export]
macro_rules! cache_sizes {
    (
        $enum_name:ident {
            $($tt:tt)*
        }
    ) => {
        cache_sizes! { __tt_muncher: $enum_name (0) [$($tt)*] [] [
            Size0 Size1 Size2 Size3 Size4 Size5 Size6 Size7
            Size8 Size9 Size10 Size11 Size12 Size13 Size14 Size15
        ] }
    };
    (
        __tt_muncher: $enum_name:ident ($count:expr) [$size:literal $size_mod:tt with $num_entries:literal entries, $($rest:tt)*] [$($parsed:tt)*] [$name:ident $($other_idents:tt)*]
    ) => {
        cache_sizes! { __tt_muncher: $enum_name ($count + 1) [$($rest)*] [($name $size $size_mod $num_entries $count) $($parsed)*] [$($other_idents)*] }
    };
    (
        __tt_muncher: $enum_name:ident ($count:expr) [] [$($parsed:tt)*] [$($maybe_idents:tt)*]
    ) => {
        cache_sizes! { __build: $enum_name $($parsed)* }
    };
    (
        __build: $enum_name:ident $(
            ($name:ident $size:literal $size_mod:tt $num_entries:literal $count:expr)
        )*
    ) => {
        /// The size of a cached window.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        enum $enum_name {
            $(
                #[doc = concat!("The cached window is ", stringify!($size), stringify!($size_mod), " large.")]
                #[doc = ""]
                #[doc = concat!("There are ", stringify!($num_entries), " entries of this size.")]
                $name,
            )*
        }

        impl $enum_name {
            /// The size of the cache entry.
            const fn size(self) -> u64 {
                match self {
                    $(
                        $enum_name::$name => $size * cache_sizes!(__size_mod: $size_mod),
                    )*
                }
            }

            /// Turns this cache size into an index.
            const fn index(self) -> usize {
                match self {
                    $(
                        $enum_name::$name => $count,
                    )*
                }
            }

            /// Turns this cache size into an index.
            const fn try_from_index(index: usize) -> Option<CacheSize> {
                match index {
                    $(
                        num if num == $count => Some($enum_name::$name),
                    )*
                    _ => None,
                }
            }

            /// Returns the number of entries to use for this cache size.
            const fn num_entries(self) -> usize {
                match self {
                    $(
                        $enum_name::$name => $num_entries,
                    )*
                }
            }
        }
    };
    // this is admittedly a bit of a hack to not have to give explicit names
    (__size_mod: KiB) => { 1024 };
    (__size_mod: MiB) => { 1024 * 1024 };
    (__size_mod: GiB) => { 1024 * 1024 * 1024 };
    (__size_mod: TiB) => { 1024 * 1024 * 1024 * 1024 };
    (__size_mod: PiB) => { 1024 * 1024 * 1024 * 1024 * 1024 };
    (__size_mod: EiB) => { 1024 * 1024 * 1024 * 1024 * 1024 * 1024 };
}

impl CacheSize {
    /// The number of cache sizes.
    pub(super) const NUM_SIZES: usize = {
        let mut size = CacheSize::SMALLEST;
        let mut count = 1;

        while let Some(next_size) = size.next() {
            size = next_size;
            count += 1;
        }

        count
    };

    /// The smallest cache size.
    pub(super) const SMALLEST: CacheSize = CacheSize::try_from_index(0).unwrap();

    /// The next cache size.
    pub(super) const fn next(self) -> Option<CacheSize> {
        CacheSize::try_from_index(self.index() + 1)
    }

    /// Iterates through the different cache sizes from small to large.
    pub(super) fn iter_through_sizes() -> impl Iterator<Item = CacheSize> {
        const SIZES: [CacheSize; CacheSize::NUM_SIZES] = {
            let mut out = [CacheSize::SMALLEST; CacheSize::NUM_SIZES];

            let mut i = 1;
            while i < out.len() {
                out[i] = CacheSize::try_from_index(i).unwrap();
                i += 1;
            }

            out
        };

        SIZES.into_iter()
    }
}

impl TryFrom<u64> for CacheSize {
    type Error = ();

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        for size in CacheSize::iter_through_sizes() {
            if size.size() == value {
                return Ok(size);
            }
        }

        Err(())
    }
}
