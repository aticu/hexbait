//! Implements the values parsed by the language.

use std::fmt;

use crate::{
    Int,
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
    Bytes(Vec<u8>),
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
            Self::Integer(int) => write!(f, "{int} (0x{int:x})"),
            Self::Float(float) => float.fmt(f),
            Self::Bytes(bytes) => match bytes.len() {
                0 => write!(f, "[]"),
                1..=16 => {
                    write!(f, "[{:02x}", bytes[0])?;
                    for byte in &bytes[1..] {
                        write!(f, " {byte:02x}")?;
                    }
                    write!(f, "]")
                }
                17.. => {
                    write!(f, "[{:02x}", bytes[0])?;
                    for byte in &bytes[1..8] {
                        write!(f, " {byte:02x}")?;
                    }
                    write!(f, " ...")?;
                    for byte in &bytes[bytes.len() - 8..] {
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
            _ => unreachable!("impossible because of static analysis"),
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
            _ => unreachable!("impossible because of static analysis"),
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
            _ => unreachable!("impossible because of static analysis"),
        }
    }

    /// Expects the value to be of type bytes, panicking if this is false.
    ///
    /// # Panics
    /// This function will panic if the value is not of type bytes.
    #[track_caller]
    pub fn expect_bytes(&self) -> &[u8] {
        match self {
            ValueKind::Bytes(value) => value,
            _ => unreachable!("impossible because of static analysis"),
        }
    }

    /// Expects the value to be a struct, panicking if this is false.
    ///
    /// # Panics
    /// This function will panic if the value is not a struct.
    #[track_caller]
    pub fn expect_struct(&self) -> &Vec<(Symbol, Value)> {
        match self {
            ValueKind::Struct { fields, .. } => fields,
            _ => unreachable!("impossible because of static analysis"),
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
                    this == other
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
