//! Implements cached values that are invalidated when a key changes.

/// Represents a cached value that is invalidates when a key changes.
///
/// The value is protected against staleness by a key.
/// If the key is unchanged, the old value is returned.
/// Otherwise the value is recomputed.
pub struct Cached<K: PartialEq, V> {
    /// The key that checks for staleness.
    key: Option<K>,
    /// The computed value.
    value: Option<V>,
    /// Whether the value was manually invalidated.
    invalidated: bool,
}

impl<K: PartialEq, V> Cached<K, V> {
    /// Creates a new cached value.
    pub fn new() -> Cached<K, V> {
        Cached {
            key: None,
            value: None,
            invalidated: false,
        }
    }

    /// Returns the cached value.
    pub fn get(&mut self, key: K, compute: impl for<'old> FnOnce(Option<&'old V>) -> V) -> &V {
        let can_keep = self.value.is_some() && !self.invalidated && Some(&key) == self.key.as_ref();

        if can_keep {
            return self.value.as_ref().unwrap();
        }

        self.key = Some(key);
        self.invalidated = false;

        let result = compute(self.value.as_ref());
        self.value.insert(result)
    }

    /// Manually invalidates the cached value, ensuring a guaranteed recomputation.
    pub fn invalidate(&mut self) {
        self.invalidated = true;
    }
}

impl<K: PartialEq, V> Default for Cached<K, V> {
    fn default() -> Self {
        Cached::new()
    }
}
