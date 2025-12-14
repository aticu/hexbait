//! Implements a type to model endianness.

/// Determines the byte-order of multi-byte structures.
#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum Endianness {
    /// The most significant byte is stored at the highest address.
    Little,
    /// The most significant byte is stored at the lowest address.
    Big,
}

impl Endianness {
    /// The native endianness.
    pub fn native() -> Endianness {
        if cfg!(target_endian = "little") {
            Endianness::Little
        } else {
            Endianness::Big
        }
    }
}

macro_rules! endianness_from_bytes {
    ($(($name:ident: $num:ident),)*) => {
        impl Endianness {
            $(
                #[doc = concat!("Returns the function used to parse a `", stringify!($num), "` from bytes of this endianness.")]
                pub fn $name(self) -> fn([u8; std::mem::size_of::<$num>()]) -> $num {
                    match self {
                        Endianness::Little => $num::from_le_bytes,
                        Endianness::Big => $num::from_be_bytes,
                    }
                }
            )*
        }
    };
}

endianness_from_bytes! {
    (u16_from_bytes: u16),
    (u32_from_bytes: u32),
    (u64_from_bytes: u64),
    (u128_from_bytes: u128),
    (i16_from_bytes: i16),
    (i32_from_bytes: i32),
    (i64_from_bytes: i64),
    (i128_from_bytes: i128),
    (f32_from_bytes: f32),
    (f64_from_bytes: f64),
}
