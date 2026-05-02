// SPDX-License-Identifier: MIT OR Apache-2.0
use std::collections::HashMap;

/// String interner for key paths to avoid repeated allocations.
///
/// Useful when the same JSON/YAML key paths are compared many times
/// during diff or merge operations on large files.
///
/// # Example
///
/// ```
/// use suture_driver::KeyInterner;
///
/// let mut interner = KeyInterner::new();
/// let a = interner.intern("config/server/host");
/// let b = interner.intern("config/server/host");
/// assert_eq!(a, b);
/// assert_eq!(interner.resolve(a), "config/server/host");
/// ```
pub struct KeyInterner {
    strings: HashMap<String, u32>,
    values: Vec<String>,
}

impl KeyInterner {
    #[must_use] 
    pub fn new() -> Self {
        Self {
            strings: HashMap::new(),
            values: Vec::new(),
        }
    }

    pub fn intern(&mut self, s: &str) -> u32 {
        if let Some(&id) = self.strings.get(s) {
            return id;
        }
        let id = self.values.len() as u32;
        self.values.push(s.to_owned());
        self.strings.insert(s.to_owned(), id);
        id
    }

    #[must_use] 
    pub fn resolve(&self, id: u32) -> &str {
        &self.values[id as usize]
    }

    #[must_use] 
    pub fn len(&self) -> usize {
        self.values.len()
    }

    #[must_use] 
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    #[must_use] 
    pub fn contains(&self, s: &str) -> bool {
        self.strings.contains_key(s)
    }
}

impl Default for KeyInterner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_intern_returns_same_id() {
        let mut interner = KeyInterner::new();
        let a = interner.intern("hello");
        let b = interner.intern("hello");
        assert_eq!(a, b);
    }

    #[test]
    fn test_intern_different_strings() {
        let mut interner = KeyInterner::new();
        let a = interner.intern("alpha");
        let b = interner.intern("beta");
        assert_ne!(a, b);
    }

    #[test]
    fn test_resolve() {
        let mut interner = KeyInterner::new();
        let id = interner.intern("path/to/key");
        assert_eq!(interner.resolve(id), "path/to/key");
    }

    #[test]
    fn test_len_and_empty() {
        let mut interner = KeyInterner::new();
        assert!(interner.is_empty());
        interner.intern("a");
        interner.intern("b");
        assert_eq!(interner.len(), 2);
    }

    #[test]
    fn test_contains() {
        let mut interner = KeyInterner::new();
        interner.intern("exists");
        assert!(interner.contains("exists"));
        assert!(!interner.contains("missing"));
    }
}
