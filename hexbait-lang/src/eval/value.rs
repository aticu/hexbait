//! Implements the values parsed by the language.

use std::{fmt, io, ops::Range, sync::Arc};

use hexbait_common::{Len, ReadBytes, RelativeOffset};

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
            Self::Bytes(bytes) => {
                let mut buf = [0; _];

                match bytes.preview_slice(&mut buf) {
                    Some(0) => write!(f, "[]"),
                    Some(len) => {
                        let slice = &buf[..len];

                        write!(f, "[{:02x}", slice[0])?;
                        for byte in &slice[1..] {
                            write!(f, " {byte:02x}")?;
                        }
                        write!(f, "]")
                    }
                    None => {
                        let (prefix, suffix) = buf.split_at(buf.len() / 2);

                        write!(f, "[{:02x}", prefix[0])?;
                        for byte in &prefix[1..] {
                            write!(f, " {byte:02x}")?;
                        }
                        write!(f, " ...")?;
                        for byte in suffix {
                            write!(f, " {byte:02x}")?;
                        }
                        write!(f, "]")
                    }
                }
            }
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

    /// Expects the value to be of type bytes, panicking if this is false.
    ///
    /// # Panics
    /// This function will panic if the value is not of type bytes.
    #[track_caller]
    pub fn expect_bytes_take(self) -> BytesValue {
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

    /// Expects the value to be an array, panicking if this is false.
    ///
    /// # Panics
    /// This function will panic if the value is not an array.
    #[track_caller]
    pub fn expect_array_take(self) -> Vec<Value> {
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
#[derive(Debug, Clone)]
pub enum BytesValue {
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
        /// Stores some of the value inline.
        ///
        /// If `len <= Self::INLINE_LEN` this stores the whole value.
        /// If `len > Self::INLINE_LEN` the first half of this buffer stores a prefix of the slice and the second half a suffix.
        buf: [u8; Self::INLINE_LEN],
    },
    /// The bytes are a concatenation of other bytes.
    Concat {
        /// The parts that are concatenated together.
        parts: Vec<BytesValue>,
    },
}

impl BytesValue {
    /// The length of the prefix and suffix stored.
    pub const PREFIX_SUFFIX_LEN: usize = 8;

    /// The number of bytes that can be stored inline.
    pub const INLINE_LEN: usize = Self::PREFIX_SUFFIX_LEN * 2;

    /// Returns the value of the bytes.
    pub fn value(&self) -> io::Result<ReadBytes<'_>> {
        match self {
            BytesValue::Lit(lit) => Ok(ReadBytes::from_buf(lit)),
            BytesValue::FromView {
                view,
                start,
                len,
                buf,
            } if let len = len.as_u64() as usize
                && len < Self::INLINE_LEN =>
            {
                Ok(ReadBytes::from_buf(&buf[..len]))
            }
            BytesValue::FromView {
                view,
                start,
                len,
                buf: _,
            } => view.read_at(*start, *len),
            BytesValue::Concat { parts } => {
                let mut out = Vec::new();
                for part in parts {
                    out.extend_from_slice(&part.value()?);
                }

                Ok(ReadBytes::from_vec(out))
            }
        }
    }

    /// Returns the bytes as a slice for preview.
    ///
    /// If they fit in the `buf` `Some(len)` is returned and `buf[..len]` is filled.
    /// If they do not fit in the `buf` the first half of the buffer is filled with a prefix and the second half with a suffix and `None` is returned.
    pub fn preview_slice(&self, buf: &mut [u8; Self::INLINE_LEN]) -> Option<usize> {
        match self {
            BytesValue::Lit(lit) => {
                if lit.len() <= Self::INLINE_LEN {
                    buf[..lit.len()].copy_from_slice(lit);

                    Some(lit.len())
                } else {
                    buf[..Self::PREFIX_SUFFIX_LEN].copy_from_slice(&lit[..Self::PREFIX_SUFFIX_LEN]);
                    buf[Self::PREFIX_SUFFIX_LEN..]
                        .copy_from_slice(&lit[lit.len() - Self::PREFIX_SUFFIX_LEN..]);

                    None
                }
            }
            BytesValue::FromView {
                len, buf: inline, ..
            } if let len = len.as_u64() as usize
                && len <= Self::INLINE_LEN =>
            {
                buf[..len].copy_from_slice(&inline[..len]);
                Some(len)
            }
            BytesValue::FromView { buf: inner_buf, .. } => {
                buf.copy_from_slice(inner_buf);
                None
            }
            BytesValue::Concat { parts } => {
                let mut fill = 0;
                let mut tmp_buf = [0; Self::INLINE_LEN];
                let mut needs_split = false;

                for part in parts {
                    match part.preview_slice(&mut tmp_buf) {
                        Some(len) if len + fill <= Self::INLINE_LEN => {
                            buf[fill..][..len].copy_from_slice(&tmp_buf[..len]);
                            fill += len;
                        }
                        _ => {
                            if fill < Self::PREFIX_SUFFIX_LEN {
                                buf[fill..Self::PREFIX_SUFFIX_LEN]
                                    .copy_from_slice(&tmp_buf[..Self::PREFIX_SUFFIX_LEN - fill]);
                            }

                            needs_split = true;
                            fill = Self::PREFIX_SUFFIX_LEN;
                            break;
                        }
                    }
                }

                if needs_split {
                    let mut placed = 0;
                    for part in parts.iter().rev() {
                        let needed = Self::PREFIX_SUFFIX_LEN - placed;
                        let end = Self::INLINE_LEN - placed;

                        match part.preview_slice(&mut tmp_buf) {
                            Some(len) if len < needed => {
                                buf[end - len..end].copy_from_slice(&tmp_buf[..len]);
                                placed += len;
                            }
                            Some(len) => {
                                buf[end - needed..end].copy_from_slice(&tmp_buf[len - needed..len]);
                                break;
                            }
                            None => {
                                buf[end - needed..end]
                                    .copy_from_slice(&tmp_buf[Self::INLINE_LEN - needed..]);
                                break;
                            }
                        }
                    }

                    None
                } else {
                    Some(fill)
                }
            }
        }
    }

    /// The length of the bytes.
    pub fn len(&self) -> usize {
        match self {
            BytesValue::Lit(lit) => lit.len(),
            BytesValue::FromView { len, .. } => len.as_u64() as usize,
            BytesValue::Concat { parts } => parts.iter().map(|part| part.len()).sum(),
        }
    }

    /// Whether or not the bytes are empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Fills the given buffer from bytes at the given offset.
    pub fn fill_buf_at(&self, offset: usize, buf: &mut [u8]) -> io::Result<()> {
        match self {
            BytesValue::Lit(lit) => {
                let end = offset.checked_add(buf.len());
                let slice = end.and_then(|end| lit.get(offset..end)).ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "bytes value literal too short",
                    )
                })?;
                buf.copy_from_slice(slice);
            }
            BytesValue::FromView {
                view,
                start,
                len,
                buf: _,
            } => {
                // This could optionally be optimized to use the cached inline values in buf if possible.
                // That would however increase implementation complexity, so for now this is fine and correct.
                let read_offset = *start + Len::from(offset as u64);
                let out_len = Len::from(buf.len() as u64);
                if Len::from(offset as u64) + out_len > *len {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "bytes value from view too short",
                    ));
                }

                let slice = view.read_at(read_offset, out_len)?;
                buf.copy_from_slice(&slice);
            }
            BytesValue::Concat { parts } => {
                let mut to_skip = offset;
                let mut buf = buf;

                for part in parts {
                    let part_len = part.len();
                    if to_skip >= part_len {
                        to_skip -= part_len;
                        continue;
                    }

                    let covered_len = part_len - to_skip;
                    if covered_len > buf.len() {
                        return part.fill_buf_at(to_skip, buf);
                    }

                    let (part_buf, rest_buf) = buf.split_at_mut(covered_len);
                    part.fill_buf_at(to_skip, part_buf)?;
                    buf = rest_buf;
                    to_skip = 0;
                }

                if !buf.is_empty() {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "concatenated bytes value too short",
                    ));
                }
            }
        }

        Ok(())
    }

    /// Returns the provenance of the bytes of the given range.
    pub fn provenance_range(&self, range: Range<RelativeOffset>) -> Provenance {
        match self {
            BytesValue::Lit(_) => Provenance::empty(),
            BytesValue::FromView {
                view, start, len, ..
            } => {
                let clamp = |off: RelativeOffset| std::cmp::min(Len::from(off.as_u64()), *len);

                view.provenance_from_range(*start + clamp(range.start)..*start + clamp(range.end))
            }
            BytesValue::Concat { parts } => {
                let mut provenance = Provenance::empty();
                let mut offset = 0;

                let range = range.start.as_u64() as usize..range.end.as_u64() as usize;

                for part in parts {
                    let part_start = offset;
                    let len = part.len();
                    offset += len;

                    if offset <= range.start {
                        continue;
                    }

                    let provenance_start = RelativeOffset::from(
                        (std::cmp::max(part_start, range.start) - part_start) as u64,
                    );
                    let provenance_end = RelativeOffset::from(
                        (std::cmp::min(offset, range.end) - part_start) as u64,
                    );
                    provenance += &part.provenance_range(provenance_start..provenance_end);

                    if offset >= range.end {
                        break;
                    }
                }

                provenance
            }
        }
    }
}

impl PartialEq for BytesValue {
    fn eq(&self, other: &Self) -> bool {
        // have a fast path for the length to avoid reads
        if self.len() != other.len() {
            return false;
        }

        let Ok(self_val) = self.value() else {
            return false;
        };
        let Ok(other_val) = other.value() else {
            return false;
        };

        *self_val == *other_val
    }
}
