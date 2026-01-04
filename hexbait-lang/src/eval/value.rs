//! Implements the values parsed by the language.

use std::{fmt, io, sync::Arc};

use hexbait_common::{Len, RelativeOffset};

use crate::{
    Int, View,
    eval::parse::ParseErrId,
    ir::{
        Lit, Symbol,
        path::{Path, PathComponent},
    },
};

use super::provenance::Provenance;

/// Represents a parsed value.
#[derive(Clone)]
pub struct Value {
    /// The kind of the value.
    pub kind: ValueKind,
    /// The provenance of the value.
    pub provenance: Provenance,
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind
    }
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:#?} from {:?}",
            self.kind,
            Vec::from_iter(self.provenance.byte_ranges())
        )
    }
}

/// The different kinds of values that can be parsed.
#[derive(Clone, PartialEq)]
pub enum ValueKind {
    /// A boolean value.
    Boolean(bool),
    /// An integer value.
    Integer(Int),
    /// A float value.
    Float(f64),
    /// A number of bytes as a value.
    Bytes(BytesValue),
    /// Represents a `struct` with named fields.
    ///
    /// This is a `Vec` and not a map, to preserve field ordering for the purposes of displaying
    /// them.
    Struct {
        /// The fields of the `struct`.
        fields: Vec<(Symbol, Value)>,
        /// An error that occurred while parsing the `struct`.
        error: Option<ParseErrId>,
    },
    /// Represents an array of values.
    Array {
        /// The items in the array.
        items: Vec<Value>,
        /// An error that occurred while parsing the array.
        error: Option<ParseErrId>,
    },
}

impl fmt::Debug for ValueKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Boolean(val) => write!(f, "{val:?}"),
            Self::Integer(int) => {
                if int.sign() == num_bigint::Sign::Minus {
                    write!(f, "{int} (-0x{:x})", -int)
                } else {
                    write!(f, "{int} (0x{int:x})")
                }
            }
            Self::Float(float) => float.fmt(f),
            Self::Bytes(bytes) => match bytes.preview_slice() {
                Ok(&[]) => write!(f, "[]"),
                Ok(slice) => {
                    write!(f, "[{:02x}", slice[0])?;
                    for byte in &slice[1..] {
                        write!(f, " {byte:02x}")?;
                    }
                    write!(f, "]")
                }
                Err((prefix, suffix)) => {
                    write!(f, "[{:02x}", prefix[0])?;
                    for byte in &prefix[1..8] {
                        write!(f, " {byte:02x}")?;
                    }
                    write!(f, " ...")?;
                    for byte in suffix {
                        write!(f, " {byte:02x}")?;
                    }
                    write!(f, "]")
                }
            },
            Self::Struct { fields, error } => {
                let mut debug_struct = f.debug_struct("struct");
                for (name, value) in fields {
                    debug_struct.field(name.as_str(), value);
                }
                if let Some(err) = error {
                    debug_struct.field("__error", &err);
                }
                debug_struct.finish()
            }
            Self::Array { items, error } => {
                let mut arr = f.debug_list();
                arr.entries(items);

                if let Some(err) = error {
                    arr.entry(&format!("__error: {err:?}"));
                    arr.finish_non_exhaustive()
                } else {
                    arr.finish()
                }
            }
        }
    }
}

impl ValueKind {
    /// Expects the value to be an boolean, panicking if this is false.
    ///
    /// # Panics
    /// This function will panic if the value is not a boolean.
    #[track_caller]
    pub fn expect_bool(&self) -> bool {
        match self {
            ValueKind::Boolean(value) => *value,
            _ => unreachable!("expected a boolean value"),
        }
    }

    /// Expects the value to be an integer, panicking if this is false.
    ///
    /// # Panics
    /// This function will panic if the value is not an integer.
    #[track_caller]
    pub fn expect_int(&self) -> &Int {
        match self {
            ValueKind::Integer(value) => value,
            _ => unreachable!("expected an integer value"),
        }
    }

    /// Expects the value to be a float, panicking if this is false.
    ///
    /// # Panics
    /// This function will panic if the value is not a float.
    #[track_caller]
    pub fn expect_float(&self) -> f64 {
        match self {
            ValueKind::Float(value) => *value,
            _ => unreachable!("expected a float value"),
        }
    }

    /// Expects the value to be of type bytes, panicking if this is false.
    ///
    /// # Panics
    /// This function will panic if the value is not of type bytes.
    #[track_caller]
    pub fn expect_bytes(&self) -> &BytesValue {
        match self {
            ValueKind::Bytes(value) => value,
            _ => unreachable!("expected a bytes value"),
        }
    }

    /// Expects the value to be a struct, panicking if this is false.
    ///
    /// # Panics
    /// This function will panic if the value is not a struct.
    #[track_caller]
    pub fn expect_struct(&self) -> &[(Symbol, Value)] {
        match self {
            ValueKind::Struct { fields, .. } => fields,
            _ => unreachable!("expected a struct value"),
        }
    }

    /// Expects the value to be an array, panicking if this is false.
    ///
    /// # Panics
    /// This function will panic if the value is not an array.
    #[track_caller]
    pub fn expect_array(&self) -> &[Value] {
        match self {
            ValueKind::Array { items, .. } => items,
            _ => unreachable!("expected an array value"),
        }
    }
}

impl Value {
    /// Returns the value at the give path.
    pub fn subvalue_at_path(&self, path: &Path) -> Option<&Value> {
        let mut current_value = self;

        for component in path.iter() {
            match component {
                PathComponent::FieldAccess(field) => {
                    let ValueKind::Struct { fields, .. } = &current_value.kind else {
                        return None;
                    };

                    'find_entry: {
                        for (name, value) in fields {
                            if name == field {
                                current_value = value;
                                break 'find_entry;
                            }
                        }
                        return None;
                    }
                }
                PathComponent::Indexing(index) => {
                    let ValueKind::Array { items, .. } = &current_value.kind else {
                        return None;
                    };

                    current_value = items.get(*index)?;
                }
            }
        }

        Some(current_value)
    }
}

impl PartialEq<Lit> for ValueKind {
    fn eq(&self, other: &Lit) -> bool {
        match other {
            Lit::Int(other) => {
                if let ValueKind::Integer(this) = self {
                    this == other
                } else {
                    false
                }
            }
            Lit::Bytes(other) => {
                if let ValueKind::Bytes(this) = self {
                    *this == BytesValue::Lit(Arc::clone(other))
                } else {
                    false
                }
            }
            Lit::Bool(other) => {
                if let ValueKind::Boolean(this) = self {
                    this == other
                } else {
                    false
                }
            }
        }
    }
}

/// Bytes that were parsed from some input.
#[derive(Clone)]
pub enum BytesValue {
    /// The bytes are stored inline.
    Inline {
        /// The buffer where the bytes are stored.
        ///
        /// Only the first `len` elements are valid.
        buf: [u8; Self::INLINE_LEN],
        /// The number of valid bytes.
        len: u8,
    },
    /// The bytes are from a literal.
    Lit(Arc<[u8]>),
    /// The bytes are derived from the given view.
    FromView {
        /// The view where the bytes are read from.
        view: View,
        /// The within the view where the bytes are stored.
        start: RelativeOffset,
        /// The length of these bytes.
        len: Len,
        /// The prefix of the bytes for fast previews.
        prefix: [u8; 8],
        /// The suffix of the bytes for fast previews.
        suffix: [u8; 8],
    },
}

impl BytesValue {
    /// The number of bytes that can be stored inline.
    pub const INLINE_LEN: usize = 16;

    /// Converts the bytes into a [`Vec`] containing the bytes.
    pub fn as_vec(&self) -> io::Result<Vec<u8>> {
        match self {
            BytesValue::Inline { buf, len } => Ok(buf[..*len as usize].to_vec()),
            BytesValue::Lit(lit) => Ok(lit.to_vec()),
            BytesValue::FromView {
                view, start, len, ..
            } => {
                let mut buf = vec![0; len.as_u64() as usize];
                view.read_at(*start, &mut buf)?;

                Ok(buf)
            }
        }
    }

    /// Returns the bytes as a slice for preview.
    ///
    /// If they are not fully stored inline, instead a prefix and a suffix are returned.
    pub fn preview_slice(&self) -> Result<&[u8], (&[u8], &[u8])> {
        match self {
            BytesValue::Inline { buf, len } => Ok(&buf[..*len as usize]),
            BytesValue::Lit(lit) => {
                if lit.len() <= 16 {
                    Ok(lit)
                } else {
                    Err((&lit[..8], &lit[lit.len() - 8..]))
                }
            }
            BytesValue::FromView { prefix, suffix, .. } => Err((prefix, suffix)),
        }
    }

    /// The length of the bytes.
    pub fn len(&self) -> usize {
        match self {
            BytesValue::Inline { len, .. } => *len as usize,
            BytesValue::Lit(lit) => lit.len(),
            BytesValue::FromView { len, .. } => len.as_u64() as usize,
        }
    }

    /// Whether or not the bytes are empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl PartialEq for BytesValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::Inline {
                    buf: l_buf,
                    len: l_len,
                },
                Self::Inline {
                    buf: r_buf,
                    len: r_len,
                },
            ) => l_buf[..*l_len as usize] == r_buf[..*r_len as usize],
            (
                Self::FromView {
                    view: l_view,
                    start: l_start,
                    len: l_len,
                    prefix: l_prefix,
                    suffix: l_suffix,
                },
                Self::FromView {
                    view: r_view,
                    start: r_start,
                    len: r_len,
                    prefix: r_prefix,
                    suffix: r_suffix,
                },
            ) => {
                l_len == r_len && l_prefix == r_prefix && l_suffix == r_suffix && {
                    let prefix_len = Len::from(l_prefix.len() as u64);
                    let len = (*l_len
                        - (Len::from(l_prefix.len() as u64) + Len::from(l_suffix.len() as u64)))
                    .as_u64() as usize;

                    let mut l_buf = vec![0; len];
                    let mut r_buf = vec![0; len];

                    let Ok(_) = l_view.read_at(*l_start + prefix_len, &mut l_buf) else {
                        return false;
                    };
                    let Ok(_) = r_view.read_at(*r_start + prefix_len, &mut r_buf) else {
                        return false;
                    };

                    l_buf == r_buf
                }
            }
            (Self::Lit(l_lit), Self::Lit(r_lit)) => l_lit == r_lit,
            (Self::Lit(lit), Self::Inline { buf, len })
            | (Self::Inline { buf, len }, Self::Lit(lit)) => buf[..*len as usize] == **lit,
            (
                Self::Lit(lit),
                Self::FromView {
                    view, start, len, ..
                },
            )
            | (
                Self::FromView {
                    view, start, len, ..
                },
                Self::Lit(lit),
            ) => {
                let mut buf = vec![0; len.as_u64() as usize];
                let Ok(_) = view.read_at(*start, &mut buf) else {
                    return false;
                };

                buf == **lit
            }
            _ => false,
        }
    }
}
