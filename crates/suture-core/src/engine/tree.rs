//! FileTree — a virtual filesystem snapshot.
//!
//! A FileTree maps relative paths to CAS blob hashes. It represents the
//! state of a project at a particular point in the patch DAG. FileTrees
//! are immutable snapshots — applying a patch produces a new tree.

use std::collections::BTreeMap;
use suture_common::Hash;

/// A virtual filesystem snapshot: path → CAS blob hash.
///
/// Uses BTreeMap for deterministic ordering (important for hashing and diffs).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileTree {
    /// Map of relative path (e.g., "src/main.rs") to BLAKE3 blob hash.
    entries: BTreeMap<String, Hash>,
}

impl FileTree {
    /// Create an empty file tree.
    #[must_use] 
    pub fn empty() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }

    /// Create a file tree from a pre-populated map.
    #[must_use] 
    pub fn from_map(entries: BTreeMap<String, Hash>) -> Self {
        Self { entries }
    }

    /// Get the blob hash for a given path.
    #[must_use] 
    pub fn get(&self, path: &str) -> Option<&Hash> {
        self.entries.get(path)
    }

    /// Check if a path exists in the tree.
    #[must_use] 
    pub fn contains(&self, path: &str) -> bool {
        self.entries.contains_key(path)
    }

    /// Insert or update a path → hash mapping.
    pub fn insert(&mut self, path: String, hash: Hash) {
        self.entries.insert(path, hash);
    }

    /// Remove a path from the tree.
    pub fn remove(&mut self, path: &str) -> Option<Hash> {
        self.entries.remove(path)
    }

    /// Rename a path in the tree.
    pub fn rename(&mut self, old_path: &str, new_path: String) -> Option<Hash> {
        if let Some(hash) = self.entries.remove(old_path) {
            self.entries.insert(new_path, hash);
            Some(hash)
        } else {
            None
        }
    }

    /// Get the number of files in the tree.
    #[must_use] 
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the tree is empty.
    #[must_use] 
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Iterate over all (path, hash) entries in sorted order.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &Hash)> {
        self.entries.iter()
    }

    /// Get all paths in the tree (sorted).
    #[must_use] 
    pub fn paths(&self) -> Vec<&String> {
        self.entries.keys().collect()
    }

    /// Compute a BLAKE3 hash of the entire tree state.
    ///
    /// This provides a content-addressed identifier for a snapshot.
    #[must_use] 
    pub fn content_hash(&self) -> Hash {
        let mut hasher = blake3::Hasher::new();
        for (path, hash) in &self.entries {
            hasher.update(path.as_bytes());
            hasher.update(b"\0");
            hasher.update(&hash.0);
            hasher.update(b"\0");
        }
        Hash::from(*hasher.finalize().as_bytes())
    }
}

impl Default for FileTree {
    fn default() -> Self {
        Self::empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_tree() {
        let tree = FileTree::empty();
        assert!(tree.is_empty());
        assert_eq!(tree.len(), 0);
    }

    #[test]
    fn test_insert_and_get() {
        let mut tree = FileTree::empty();
        let hash = Hash::from_data(b"hello");
        tree.insert("src/main.rs".to_string(), hash);
        assert_eq!(tree.len(), 1);
        assert!(tree.contains("src/main.rs"));
        assert_eq!(tree.get("src/main.rs"), Some(&hash));
    }

    #[test]
    fn test_remove() {
        let mut tree = FileTree::empty();
        tree.insert("file.txt".to_string(), Hash::from_data(b"data"));
        assert!(tree.contains("file.txt"));
        tree.remove("file.txt");
        assert!(!tree.contains("file.txt"));
        assert!(tree.is_empty());
    }

    #[test]
    fn test_rename() {
        let mut tree = FileTree::empty();
        let hash = Hash::from_data(b"data");
        tree.insert("old.txt".to_string(), hash);
        tree.rename("old.txt", "new.txt".to_string());
        assert!(!tree.contains("old.txt"));
        assert!(tree.contains("new.txt"));
        assert_eq!(tree.get("new.txt"), Some(&hash));
    }

    #[test]
    fn test_content_hash_deterministic() {
        let mut tree1 = FileTree::empty();
        let mut tree2 = FileTree::empty();
        let h1 = Hash::from_data(b"file1");
        let h2 = Hash::from_data(b"file2");
        tree1.insert("a.txt".to_string(), h1);
        tree1.insert("b.txt".to_string(), h2);
        tree2.insert("a.txt".to_string(), h1);
        tree2.insert("b.txt".to_string(), h2);
        assert_eq!(tree1.content_hash(), tree2.content_hash());
    }

    #[test]
    fn test_content_hash_different() {
        let mut tree1 = FileTree::empty();
        let mut tree2 = FileTree::empty();
        tree1.insert("a.txt".to_string(), Hash::from_data(b"v1"));
        tree2.insert("a.txt".to_string(), Hash::from_data(b"v2"));
        assert_ne!(tree1.content_hash(), tree2.content_hash());
    }

    #[test]
    fn test_paths_sorted() {
        let mut tree = FileTree::empty();
        tree.insert("z.txt".to_string(), Hash::ZERO);
        tree.insert("a.txt".to_string(), Hash::ZERO);
        tree.insert("m.txt".to_string(), Hash::ZERO);
        let paths = tree.paths();
        assert_eq!(paths[0], &"a.txt".to_string());
        assert_eq!(paths[1], &"m.txt".to_string());
        assert_eq!(paths[2], &"z.txt".to_string());
    }

    mod proptests {
        use super::*;
        use proptest::prelude::*;
        use suture_common::Hash;

        fn valid_path() -> impl Strategy<Value = String> {
            proptest::string::string_regex("[a-zA-Z0-9_/:-]{1,100}").unwrap()
        }

        fn hash_bytes_strategy() -> impl Strategy<Value = [u8; 32]> {
            proptest::array::uniform32(proptest::num::u8::ANY)
        }

        proptest! {
            #[test]
            fn insert_then_contains(path in valid_path(), hash_bytes in hash_bytes_strategy()) {
                let mut tree = FileTree::empty();
                let hash = Hash::from(hash_bytes);
                tree.insert(path.clone(), hash);
                prop_assert!(tree.contains(&path));
                prop_assert_eq!(tree.get(&path), Some(&hash));
            }

            #[test]
            fn remove_then_not_contains(path in valid_path(), hash_bytes in hash_bytes_strategy()) {
                let mut tree = FileTree::empty();
                let hash = Hash::from(hash_bytes);
                tree.insert(path.clone(), hash);
                tree.remove(&path);
                prop_assert!(!tree.contains(&path));
                prop_assert_eq!(tree.get(&path), None);
            }

            #[test]
            fn insert_remove_insert(path in valid_path(), hash_bytes in hash_bytes_strategy()) {
                let mut tree = FileTree::empty();
                let hash = Hash::from(hash_bytes);
                tree.insert(path.clone(), hash);
                tree.remove(&path);
                prop_assert!(!tree.contains(&path));
                tree.insert(path.clone(), hash);
                prop_assert!(tree.contains(&path));
                prop_assert_eq!(tree.get(&path), Some(&hash));
            }

            #[test]
            fn rename(
                old_path in valid_path(),
                new_path in valid_path(),
                hash_bytes in hash_bytes_strategy()
            ) {
                prop_assume!(old_path != new_path);
                let mut tree = FileTree::empty();
                let hash = Hash::from(hash_bytes);
                tree.insert(old_path.clone(), hash);
                tree.rename(&old_path, new_path.clone());
                prop_assert!(!tree.contains(&old_path));
                prop_assert!(tree.contains(&new_path));
                prop_assert_eq!(tree.get(&new_path), Some(&hash));
            }

            #[test]
            fn trees_equal_self(entries in proptest::collection::btree_map(valid_path(), hash_bytes_strategy().prop_map(Hash::from), 0..20)) {
                let tree = FileTree::from_map(entries);
                prop_assert_eq!(&tree, &tree);
            }

            #[test]
            fn trees_equal_symmetry(entries in proptest::collection::btree_map(valid_path(), hash_bytes_strategy().prop_map(Hash::from), 0..20)) {
                let tree1 = FileTree::from_map(entries.clone());
                let tree2 = FileTree::from_map(entries);
                prop_assert_eq!(tree1, tree2);
            }
        }
    }
}
