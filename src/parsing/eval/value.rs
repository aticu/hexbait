//! Implements values in the language.

use std::fmt;

use crate::parsing::language::{Int, ast::Symbol};

use super::{Path, PathComponent, Provenance};

/// Represents a parsed value.
#[derive(Debug, Clone, PartialEq)]
pub struct Value {
    /// The kind of the value.
    pub kind: ValueKind,
    /// The provenance of the value.
    pub provenance: Provenance,
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
    /// Represents a struct with named fields.
    Struct(Vec<(Symbol, Value)>),
    /// Represents an array of values.
    Array(Vec<Value>),
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
            Self::Struct(r#struct) => f
                .debug_map()
                .entries(r#struct.iter().map(|(name, val)| (name, val)))
                .finish(),
            Self::Array(array) => f.debug_list().entries(array).finish(),
        }
    }
}

impl Value {
    /// Expects the value to be an boolean, panicking if this is false.
    ///
    /// # Panics
    /// This function will panic if the value is not a boolean.
    #[track_caller]
    pub fn expect_bool(&self) -> bool {
        match &self.kind {
            ValueKind::Boolean(value) => *value,
            _ => panic!("expected value to be a boolean"),
        }
    }

    /// Expects the value to be an integer, panicking if this is false.
    ///
    /// # Panics
    /// This function will panic if the value is not an integer.
    #[track_caller]
    pub fn expect_int(&self) -> &Int {
        match &self.kind {
            ValueKind::Integer(value) => value,
            _ => panic!("expected value to be an integer"),
        }
    }

    /// Expects the value to be a float, panicking if this is false.
    ///
    /// # Panics
    /// This function will panic if the value is not a float.
    #[track_caller]
    pub fn expect_float(&self) -> f64 {
        match &self.kind {
            ValueKind::Float(value) => *value,
            _ => panic!("expected value to be a float"),
        }
    }

    /// Expects the value to be of type bytes, panicking if this is false.
    ///
    /// # Panics
    /// This function will panic if the value is not of type bytes.
    #[track_caller]
    pub fn expect_bytes(&self) -> &[u8] {
        match &self.kind {
            ValueKind::Bytes(value) => value,
            _ => panic!("expected value to be of type bytes"),
        }
    }

    /// Expects the value to be a struct, panicking if this is false.
    ///
    /// # Panics
    /// This function will panic if the value is not a struct.
    #[track_caller]
    pub fn expect_struct(&self) -> &[(Symbol, Value)] {
        match &self.kind {
            ValueKind::Struct(value) => value,
            _ => panic!("expected value to be a struct"),
        }
    }

    /// Returns the value at the give path.
    pub fn subvalue_at_path(&self, path: &Path) -> Option<&Value> {
        let mut current_value = self;

        for component in path.iter() {
            match component {
                PathComponent::FieldAccess(field) => {
                    let ValueKind::Struct(items) = &current_value.kind else {
                        return None;
                    };

                    'find_entry: {
                        for (name, value) in items {
                            if name == field {
                                current_value = value;
                                break 'find_entry;
                            }
                        }
                        return None;
                    }
                }
                PathComponent::Indexing(index) => {
                    let ValueKind::Array(array) = &current_value.kind else {
                        return None;
                    };

                    current_value = array.get(*index)?;
                }
            }
        }

        Some(current_value)
    }
}
