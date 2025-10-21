//! Implements paths to subvalues.

use crate::ir::Symbol;

/// A path to a value.
#[derive(Debug, Clone)]
pub struct Path {
    /// The components that make up this path.
    components: Vec<PathComponent>,
}

impl Path {
    /// Create a new empty path.
    pub fn new() -> Path {
        Path {
            components: Vec::new(),
        }
    }

    /// Adds the given component to the given path.
    pub fn push(&mut self, component: PathComponent) {
        self.components.push(component);
    }

    /// Returns an iterator over the components of the path.
    pub fn iter(&self) -> impl Iterator<Item = &PathComponent> {
        self.components.iter()
    }
}

impl Default for Path {
    fn default() -> Self {
        Path::new()
    }
}

/// A single path component.
#[derive(Debug, Clone)]
pub enum PathComponent {
    /// Access to a field in a struct.
    FieldAccess(Symbol),
    /// Access to an element in an array of values.
    Indexing(usize),
}
