//! Implements the state for format discovery mode.

use std::{collections::BTreeMap, ops::Range};

/// The default length for format discovery mode.
const DEFAULT_LEN: u64 = 64;

/// The state for format discovery mode.
pub struct FormatDiscoveryState {
    /// The type of mark that is used for the format discovery mode.
    ///
    /// If this is `None`, the mode is inactive.
    mark_name: Option<String>,
    /// The state for each mark type.
    types: BTreeMap<String, TypeState>,
}

impl FormatDiscoveryState {
    /// Creates new state for format discovery mode.
    pub fn new() -> FormatDiscoveryState {
        FormatDiscoveryState {
            mark_name: None,
            types: BTreeMap::new(),
        }
    }

    /// Enters format discovery mode for the given mark name.
    pub fn enter(&mut self, mark_name: String) {
        self.mark_name = Some(mark_name);
    }

    /// Leaves format discovery mode.
    pub fn exit(&mut self) {
        self.mark_name = None;
    }

    /// Whether format discovery mode is currently active.
    pub fn is_in_format_discovery_mode(&self) -> bool {
        self.mark_name.is_some()
    }

    /// The mark type that is being investigated.
    ///
    /// # Panics
    /// This function will panic if not in format discovery mode.
    pub fn mark_name(&self) -> &str {
        self.mark_name.as_deref().unwrap()
    }

    /// Returns mutable access to the length for the current mark type.
    ///
    /// # Panics
    /// This function will panic if not in format discovery mode.
    pub fn type_state_mut(&mut self) -> &mut TypeState {
        self.types
            .entry(self.mark_name.as_ref().unwrap().clone())
            .or_default()
    }
}

impl Default for FormatDiscoveryState {
    fn default() -> Self {
        FormatDiscoveryState::new()
    }
}

/// Stores information about a single type in format discovery mode.
pub struct TypeState {
    /// The length in bytes of each row.
    pub len: u64,
    /// The index of the column for which the context menu is currently open.
    pub context_menu_idx: Option<usize>,
    /// The columns configured by the user.
    columns: Vec<ColumnInfo>,
    /// Which column is currently being dragged and what was the drag start offset.
    currently_dragged_column: Option<(usize, u64)>,
}

impl TypeState {
    /// Creates state for a new type.
    fn new() -> TypeState {
        TypeState {
            len: DEFAULT_LEN,
            context_menu_idx: None,
            columns: Vec::new(),
            currently_dragged_column: None,
        }
    }

    /// Starts a new interaction at the given offset.
    pub fn start_interaction_at(&mut self, offset: u64) {
        for (i, column) in self.columns.iter().enumerate() {
            if column.end() > offset {
                if column.start <= offset {
                    self.currently_dragged_column = Some((i, offset));
                } else {
                    self.columns.insert(i, ColumnInfo::new_at(offset));
                    self.currently_dragged_column = Some((i, offset));
                }
                return;
            }
        }

        self.currently_dragged_column = Some((self.columns.len(), offset));
        self.columns.push(ColumnInfo::new_at(offset));
    }

    /// Let's the front-end signal it's current offset so that interactions can be properly handled.
    pub fn signal_current_offset(&mut self, offset: u64) {
        if let Some((idx, start_offset)) = self.currently_dragged_column {
            let min_offset = if idx == 0 {
                0
            } else {
                self.columns[idx - 1].end()
            };
            let max_offset = if idx == self.columns.len() - 1 {
                self.len
            } else {
                self.columns[idx + 1].start - 1
            };

            let offset = offset.clamp(min_offset, max_offset);

            if offset < start_offset {
                self.columns[idx].start = offset;
                self.columns[idx].len = start_offset - offset + 1;
            } else {
                self.columns[idx].start = start_offset;
                self.columns[idx].len = offset - start_offset + 1;
            }
        }
    }

    /// Stops the current interaction.
    pub fn stop_interaction(&mut self) {
        self.currently_dragged_column = None;
    }

    /// Removes the specified column.
    pub fn remove_col(&mut self, idx: usize) {
        self.columns.remove(idx);
    }

    /// Gives mutable access to the columns.
    pub fn columns_mut(&mut self) -> &mut [ColumnInfo] {
        &mut self.columns
    }
}

impl Default for TypeState {
    fn default() -> Self {
        TypeState::new()
    }
}

/// Stores information about a single column.
#[derive(Debug)]
pub struct ColumnInfo {
    /// The start offset of this column in bytes.
    start: u64,
    /// The length of this column in bytes.
    len: u64,
    /// The name of this column.
    pub name: Option<String>,
    /// The type of this column.
    pub ty: ColumnType,
}

impl ColumnInfo {
    /// Creates a new column at the given offset.
    fn new_at(offset: u64) -> ColumnInfo {
        ColumnInfo {
            start: offset,
            len: 1,
            name: None,
            ty: ColumnType::None,
        }
    }

    /// The end offset of this column.
    fn end(&self) -> u64 {
        self.start + self.len
    }

    /// The covered range of this column.
    pub fn covered_range(&self) -> Range<u64> {
        self.start..self.end()
    }
}

/// The type of a single column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ColumnType {
    /// No type was specified.
    None,
    /// A magic value.
    Magic,
    /// An unsigned integer.
    Uint,
    /// A signed integer.
    Sint,
    /// A UTF-8 string.
    Utf8,
}

impl ColumnType {
    /// An iterator over all column types.
    pub fn iter_all_types() -> impl Iterator<Item = ColumnType> {
        [
            ColumnType::None,
            ColumnType::Magic,
            ColumnType::Uint,
            ColumnType::Sint,
            ColumnType::Utf8,
        ]
        .into_iter()
    }

    /// The user-facing name of the column type.
    pub fn name(&self) -> &'static str {
        match self {
            ColumnType::None => "Untyped",
            ColumnType::Magic => "Signature/Magic",
            ColumnType::Uint => "Unsigned integer",
            ColumnType::Sint => "Signed integer",
            ColumnType::Utf8 => "UTF-8 text",
        }
    }
}
