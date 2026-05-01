// SPDX-License-Identifier: MIT OR Apache-2.0
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

/// LRU-style cache for merge results, keyed by content hashes.
///
/// Avoids re-running expensive semantic merges when the same triple
/// (base, ours, theirs) is seen repeatedly (e.g., in CI rebases or
/// cherry-pick workflows).
///
/// # Example
///
/// ```
/// use suture_driver::MergeCache;
///
/// let mut cache = MergeCache::new(64);
/// cache.insert("base", "ours", "theirs", "merged result".to_string());
/// assert_eq!(cache.get("base", "ours", "theirs"), Some("merged result"));
/// ```
pub struct MergeCache {
    entries: HashMap<(u64, u64, u64), String>,
    max_entries: usize,
}

impl MergeCache {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: HashMap::new(),
            max_entries,
        }
    }

    pub fn get(&self, base: &str, ours: &str, theirs: &str) -> Option<&str> {
        let key = (hash(base), hash(ours), hash(theirs));
        self.entries.get(&key).map(|s| s.as_str())
    }

    pub fn insert(&mut self, base: &str, ours: &str, theirs: &str, result: String) {
        if self.entries.len() >= self.max_entries
            && let Some(first_key) = self.entries.keys().next().copied()
        {
            self.entries.remove(&first_key);
        }
        let key = (hash(base), hash(ours), hash(theirs));
        self.entries.insert(key, result);
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

fn hash(s: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_hit() {
        let mut cache = MergeCache::new(16);
        cache.insert("b", "o", "t", "result".to_string());
        assert_eq!(cache.get("b", "o", "t"), Some("result"));
    }

    #[test]
    fn test_cache_miss() {
        let cache = MergeCache::new(16);
        assert_eq!(cache.get("b", "o", "t"), None);
    }

    #[test]
    fn test_cache_eviction() {
        let mut cache = MergeCache::new(2);
        cache.insert("a", "b", "c", "1".to_string());
        cache.insert("d", "e", "f", "2".to_string());
        cache.insert("g", "h", "i", "3".to_string());
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn test_cache_clear() {
        let mut cache = MergeCache::new(16);
        cache.insert("a", "b", "c", "1".to_string());
        cache.clear();
        assert!(cache.is_empty());
    }

    #[test]
    fn test_cache_different_order_is_different() {
        let mut cache = MergeCache::new(16);
        cache.insert("base", "ours", "theirs", "result1".to_string());
        cache.insert("base", "theirs", "ours", "result2".to_string());
        assert_eq!(cache.get("base", "ours", "theirs"), Some("result1"));
        assert_eq!(cache.get("base", "theirs", "ours"), Some("result2"));
    }
}
