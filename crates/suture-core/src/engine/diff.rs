//! Diff computation — compare two FileTrees to find changes.
//!
//! Given two FileTree snapshots (typically from different commits),
//! this module computes the set of changes between them.

use crate::engine::tree::FileTree;
use std::fmt;
use suture_common::Hash;

/// The type of change detected between two trees.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DiffType {
    /// A file was added (exists in new tree but not old).
    Added,
    /// A file was modified (exists in both, different blob hash).
    Modified,
    /// A file was deleted (exists in old tree but not new).
    Deleted,
    /// A file was renamed (removed from old path, added at new path with same hash).
    Renamed { old_path: String, new_path: String },
}

impl fmt::Display for DiffType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Added => write!(f, "added"),
            Self::Modified => write!(f, "modified"),
            Self::Deleted => write!(f, "deleted"),
            Self::Renamed { old_path, new_path } => {
                write!(f, "renamed {old_path} → {new_path}")
            }
        }
    }
}

/// A single diff entry representing a change between two trees.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiffEntry {
    /// The path of the changed file (new path for renames).
    pub path: String,
    /// The type of change.
    pub diff_type: DiffType,
    /// The blob hash in the old tree (None for additions).
    pub old_hash: Option<Hash>,
    /// The blob hash in the new tree (None for deletions).
    pub new_hash: Option<Hash>,
}

impl fmt::Display for DiffEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.diff_type {
            DiffType::Added => {
                write!(
                    f,
                    "  + {} ({})",
                    self.path,
                    self.new_hash.unwrap_or(Hash::ZERO)
                )
            }
            DiffType::Modified => {
                write!(
                    f,
                    "  M {} ({})",
                    self.path,
                    self.new_hash.unwrap_or(Hash::ZERO)
                )
            }
            DiffType::Deleted => {
                write!(f, "  - {}", self.path)
            }
            DiffType::Renamed { old_path, new_path } => {
                write!(f, "  R {old_path} → {new_path}")
            }
        }
    }
}

/// Compute the diff between two FileTrees.
///
/// Returns a list of DiffEntry describing all changes. The entries are
/// sorted by path for deterministic output.
///
/// # Algorithm
///
/// 1. Files only in `old_tree` → Deleted
/// 2. Files only in `new_tree` → Added
/// 3. Files in both with same hash → Unchanged (omitted)
/// 4. Files in both with different hash → Modified
/// 5. Heuristic rename detection: a deleted file whose hash matches an
///    added file is reported as a rename.
#[must_use] 
pub fn diff_trees(old_tree: &FileTree, new_tree: &FileTree) -> Vec<DiffEntry> {
    let mut diffs = Vec::new();

    // Files only in old tree → deleted (or renamed)
    let mut deleted_paths: Vec<String> = Vec::new();
    for (path, old_hash) in old_tree.iter() {
        match new_tree.get(path) {
            Some(new_hash) if old_hash == new_hash => {
                // Unchanged — skip
            }
            Some(_new_hash) => {
                // Modified
                diffs.push(DiffEntry {
                    path: path.clone(),
                    diff_type: DiffType::Modified,
                    old_hash: Some(*old_hash),
                    new_hash: new_tree.get(path).copied(),
                });
            }
            None => {
                deleted_paths.push(path.clone());
            }
        }
    }

    // Files only in new tree → added (or renamed target)
    let mut added_paths: Vec<(String, Hash)> = Vec::new();
    for (path, new_hash) in new_tree.iter() {
        if !old_tree.contains(path) {
            added_paths.push((path.clone(), *new_hash));
        }
    }

    // Rename detection: build a hash→path index from added files for O(1) lookup
    let mut hash_to_added: std::collections::HashMap<Hash, Vec<&str>> =
        std::collections::HashMap::with_capacity(added_paths.len());
    for (add_path, add_hash) in &added_paths {
        hash_to_added.entry(*add_hash).or_default().push(add_path);
    }

    let mut matched_deletes: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut matched_adds: std::collections::HashSet<String> = std::collections::HashSet::new();

    for (del_path, del_hash) in old_tree.iter() {
        if !new_tree.contains(del_path)
            && let Some(candidates) = hash_to_added.get(del_hash)
        {
            for add_path in candidates {
                if !matched_adds.contains(*add_path)
                    && !matched_deletes.contains(del_path)
                {
                    diffs.push(DiffEntry {
                        path: add_path.to_string(),
                        diff_type: DiffType::Renamed {
                            old_path: del_path.clone(),
                            new_path: add_path.to_string(),
                        },
                        old_hash: Some(*del_hash),
                        new_hash: Some(*del_hash),
                    });
                    matched_deletes.insert(del_path.clone());
                    matched_adds.insert(add_path.to_string());
                    break;
                }
            }
        }
    }

    // Remaining deletes
    for del_path in &deleted_paths {
        if !matched_deletes.contains(del_path) {
            diffs.push(DiffEntry {
                path: del_path.clone(),
                diff_type: DiffType::Deleted,
                old_hash: old_tree.get(del_path).copied(),
                new_hash: None,
            });
        }
    }

    // Remaining adds
    for (add_path, add_hash) in &added_paths {
        if !matched_adds.contains(add_path) {
            diffs.push(DiffEntry {
                path: add_path.clone(),
                diff_type: DiffType::Added,
                old_hash: None,
                new_hash: Some(*add_hash),
            });
        }
    }

    // Sort by path for deterministic output
    diffs.sort_by_key(|a| a.path.clone());
    diffs
}

/// Check if two trees are identical.
#[must_use] 
pub fn trees_equal(a: &FileTree, b: &FileTree) -> bool {
    if a.len() != b.len() {
        return false;
    }
    for (path, hash) in a.iter() {
        if b.get(path) != Some(hash) {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(data: &[u8]) -> Hash {
        Hash::from_data(data)
    }

    #[test]
    fn test_diff_identical() {
        let mut tree = FileTree::empty();
        tree.insert("a.txt".to_string(), h(b"hello"));
        tree.insert("b.txt".to_string(), h(b"world"));
        let diffs = diff_trees(&tree, &tree);
        assert!(diffs.is_empty());
    }

    #[test]
    fn test_diff_empty_to_full() {
        let empty = FileTree::empty();
        let mut full = FileTree::empty();
        full.insert("a.txt".to_string(), h(b"a"));
        full.insert("b.txt".to_string(), h(b"b"));

        let diffs = diff_trees(&empty, &full);
        assert_eq!(diffs.len(), 2);
        assert!(diffs.iter().all(|d| d.diff_type == DiffType::Added));
    }

    #[test]
    fn test_diff_addition() {
        let mut old = FileTree::empty();
        old.insert("a.txt".to_string(), h(b"a"));
        let mut new = old.clone();
        new.insert("b.txt".to_string(), h(b"b"));

        let diffs = diff_trees(&old, &new);
        assert_eq!(diffs.len(), 1);
        assert_eq!(diffs[0].path, "b.txt");
        assert_eq!(diffs[0].diff_type, DiffType::Added);
    }

    #[test]
    fn test_diff_modification() {
        let mut old = FileTree::empty();
        old.insert("a.txt".to_string(), h(b"v1"));
        let mut new = FileTree::empty();
        new.insert("a.txt".to_string(), h(b"v2"));

        let diffs = diff_trees(&old, &new);
        assert_eq!(diffs.len(), 1);
        assert_eq!(diffs[0].diff_type, DiffType::Modified);
        assert_eq!(diffs[0].old_hash, Some(h(b"v1")));
        assert_eq!(diffs[0].new_hash, Some(h(b"v2")));
    }

    #[test]
    fn test_diff_deletion() {
        let mut old = FileTree::empty();
        old.insert("a.txt".to_string(), h(b"data"));
        let new = FileTree::empty();

        let diffs = diff_trees(&old, &new);
        assert_eq!(diffs.len(), 1);
        assert_eq!(diffs[0].diff_type, DiffType::Deleted);
    }

    #[test]
    fn test_diff_rename() {
        let data = b"same content";
        let mut old = FileTree::empty();
        old.insert("old.txt".to_string(), h(data));
        let mut new = FileTree::empty();
        new.insert("new.txt".to_string(), h(data));

        let diffs = diff_trees(&old, &new);
        assert_eq!(diffs.len(), 1);
        assert!(matches!(
            &diffs[0].diff_type,
            DiffType::Renamed { old_path, new_path } if old_path == "old.txt" && new_path == "new.txt"
        ));
    }

    #[test]
    fn test_diff_complex() {
        let mut old = FileTree::empty();
        old.insert("keep.txt".to_string(), h(b"same"));
        old.insert("modify.txt".to_string(), h(b"old"));
        old.insert("delete.txt".to_string(), h(b"gone"));

        let mut new = FileTree::empty();
        new.insert("keep.txt".to_string(), h(b"same"));
        new.insert("modify.txt".to_string(), h(b"new"));
        new.insert("add.txt".to_string(), h(b"new file"));

        let diffs = diff_trees(&old, &new);
        assert_eq!(diffs.len(), 3);

        let types: Vec<&DiffType> = diffs.iter().map(|d| &d.diff_type).collect();
        assert!(types.contains(&&DiffType::Added));
        assert!(types.contains(&&DiffType::Modified));
        assert!(types.contains(&&DiffType::Deleted));
    }

    #[test]
    fn test_trees_equal() {
        let mut t1 = FileTree::empty();
        let mut t2 = FileTree::empty();
        t1.insert("a.txt".to_string(), h(b"x"));
        t2.insert("a.txt".to_string(), h(b"x"));
        assert!(trees_equal(&t1, &t2));

        t2.insert("a.txt".to_string(), h(b"y"));
        assert!(!trees_equal(&t1, &t2));
    }

    #[test]
    fn test_diff_display() {
        let mut old = FileTree::empty();
        old.insert("file.txt".to_string(), h(b"old"));
        let mut new = FileTree::empty();
        new.insert("file.txt".to_string(), h(b"new"));
        let diffs = diff_trees(&old, &new);
        let display = format!("{}", diffs[0]);
        assert!(display.contains("M file.txt"));
    }

    mod proptests {
        use super::*;
        use proptest::prelude::*;
        use suture_common::Hash;

        fn valid_path() -> impl Strategy<Value = String> {
            proptest::string::string_regex("[a-zA-Z0-9_/:-]{1,100}").unwrap()
        }

        fn hash_strategy() -> impl Strategy<Value = Hash> {
            proptest::array::uniform32(proptest::num::u8::ANY).prop_map(Hash::from)
        }

        proptest! {
            #[test]
            fn diff_empty_vs_full(entries in proptest::collection::btree_map(valid_path(), hash_strategy(), 1..20)) {
                prop_assume!(!entries.is_empty());
                let empty = FileTree::empty();
                let full = FileTree::from_map(entries);
                let diffs = diff_trees(&empty, &full);
                prop_assert!(!diffs.is_empty());
                prop_assert!(diffs.iter().all(|d| d.diff_type == DiffType::Added),
                    "all entries should be Added when diffing empty vs full");
            }

            #[test]
            fn diff_full_vs_empty(entries in proptest::collection::btree_map(valid_path(), hash_strategy(), 1..20)) {
                prop_assume!(!entries.is_empty());
                let full = FileTree::from_map(entries);
                let empty = FileTree::empty();
                let diffs = diff_trees(&full, &empty);
                prop_assert!(!diffs.is_empty());
                for d in &diffs {
                    prop_assert!(
                        d.diff_type == DiffType::Deleted || matches!(d.diff_type, DiffType::Renamed { .. }),
                        "expected Deleted or Renamed, got {:?}", d.diff_type
                    );
                }
            }

            #[test]
            fn diff_identical(entries in proptest::collection::btree_map(valid_path(), hash_strategy(), 0..20)) {
                let tree = FileTree::from_map(entries);
                let diffs = diff_trees(&tree, &tree);
                prop_assert!(diffs.is_empty(), "diff of identical trees should be empty");
            }

            #[test]
            fn diff_symmetry_inverse(
                entries_a in proptest::collection::btree_map(valid_path(), hash_strategy(), 1..15),
                entries_b in proptest::collection::btree_map(valid_path(), hash_strategy(), 1..15)
            ) {
                prop_assume!(entries_a != entries_b);
                let tree_a = FileTree::from_map(entries_a);
                let tree_b = FileTree::from_map(entries_b);

                let diffs_ab = diff_trees(&tree_a, &tree_b);
                let diffs_ba = diff_trees(&tree_b, &tree_a);

                for d_ab in &diffs_ab {
                    let inverse = diffs_ba.iter().find(|d| d.path == d_ab.path);
                    match &d_ab.diff_type {
                        DiffType::Added => {
                            if let Some(d_ba) = inverse {
                                prop_assert!(
                                    d_ba.diff_type == DiffType::Deleted || matches!(d_ba.diff_type, DiffType::Renamed { .. }),
                                    "Added in A->B should be Deleted or Renamed in B->A, got {:?}", d_ba.diff_type
                                );
                            }
                        }
                        DiffType::Deleted => {
                            if let Some(d_ba) = inverse {
                                prop_assert!(
                                    d_ba.diff_type == DiffType::Added || matches!(d_ba.diff_type, DiffType::Renamed { .. }),
                                    "Deleted in A->B should be Added or Renamed in B->A, got {:?}", d_ba.diff_type
                                );
                            }
                        }
                        DiffType::Modified => {
                            if let Some(d_ba) = inverse {
                                prop_assert_eq!(&d_ba.diff_type, &DiffType::Modified,
                                    "Modified in A->B should stay Modified in B->A");
                            }
                        }
                        DiffType::Renamed { old_path, new_path: _ } => {
                            if let Some(d_ba) = inverse {
                                prop_assert!(
                                    matches!(d_ba.diff_type, DiffType::Renamed { .. }) || d_ba.diff_type == DiffType::Deleted,
                                    "Renamed in A->B should be Renamed or Deleted in B->A, got {:?}", d_ba.diff_type
                                );
                            }
                            let old_inv = diffs_ba.iter().find(|d| d.path == *old_path);
                            if let Some(d_old) = old_inv {
                                prop_assert!(
                                    d_old.diff_type == DiffType::Added || matches!(d_old.diff_type, DiffType::Renamed { .. }),
                                    "old_path of rename should be Added or Renamed in B->A, got {:?}", d_old.diff_type
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}
