//! Patch application — transform a FileTree by applying a patch.
//!
//! Each patch operation type has well-defined semantics:
//! - **Create**: Add a new file to the tree
//! - **Modify**: Update an existing file's blob hash
//! - **Delete**: Remove a file from the tree
//! - **Move**: Rename a file in the tree
//! - **Metadata**: Update path permissions/timestamps (no tree change in v0.1)
//! - **Merge**: Special commit combining two parents (apply both parents' chains)
//! - **Identity**: No-op

use crate::engine::tree::FileTree;
use crate::patch::types::{OperationType, Patch, TouchSet};
use thiserror::Error;

/// Errors that can occur during patch application.
#[derive(Error, Debug)]
pub enum ApplyError {
    #[error("patch not found in DAG: {0}")]
    PatchNotFound(String),

    #[error("file not found for delete: {0}")]
    FileNotFound(String),

    #[error("file already exists for create: {0}")]
    FileAlreadyExists(String),

    #[error("cannot apply patch: {0}")]
    Custom(String),
}

/// Apply a single patch to a FileTree, producing a new FileTree.
///
/// # Operation Semantics
///
/// | Operation | Precondition | Effect |
/// |-----------|-------------|--------|
/// | Create | Path must NOT exist | Insert path → blob hash |
/// | Modify | Path must exist | Update blob hash |
/// | Delete | Path must exist | Remove path |
/// | Move | Old path must exist, new must NOT | Rename |
/// | Metadata | Path must exist (if specified) | No tree change |
/// | Merge | N/A | No tree change (merge commits carry no payload) |
/// | Identity | N/A | No change |
///
/// # Arguments
///
/// * `tree` - The current file tree state
/// * `patch` - The patch to apply
/// * `get_payload_blob` - Function to resolve the patch payload to a CAS hash.
///   For Modify/Create, the payload is a hex-encoded CAS hash; this function
///   parses it and returns the actual hash.
pub fn apply_patch<F>(
    tree: &FileTree,
    patch: &Patch,
    mut get_payload_blob: F,
) -> Result<FileTree, ApplyError>
where
    F: FnMut(&Patch) -> Option<suture_common::Hash>,
{
    let mut new_tree = tree.clone();

    // Handle Batch patches — iterate file changes and apply each in-place
    // (avoids O(N²) cloning: clone once, then mutate)
    if patch.operation_type == OperationType::Batch {
        if let Some(changes) = patch.file_changes() {
            for change in &changes {
                apply_single_op_mut(
                    &mut new_tree,
                    &change.op,
                    &change.path,
                    &change.payload,
                );
            }
        }
        return Ok(new_tree);
    }

    // Skip identity, merge, and root patches (no target_path)
    if patch.is_identity()
        || patch.operation_type == OperationType::Merge
        || patch.target_path.is_none()
    {
        return Ok(new_tree);
    }

    // Safe to unwrap — we checked is_none above
    let Some(target_path) = patch.target_path.as_deref() else {
        return Ok(new_tree);
    };

    apply_single_op(
        &new_tree,
        &patch.operation_type,
        target_path,
        &patch.payload,
        &mut get_payload_blob,
    )
}

fn apply_single_op<F>(
    tree: &FileTree,
    op: &OperationType,
    target_path: &str,
    payload: &[u8],
    mut get_payload_blob: F,
) -> Result<FileTree, ApplyError>
where
    F: FnMut(&Patch) -> Option<suture_common::Hash>,
{
    let mut new_tree = tree.clone();

    match op {
        OperationType::Create => {
            if new_tree.contains(target_path) {
                return Ok(new_tree);
            }
            let tmp_patch = Patch::new(
                OperationType::Create,
                TouchSet::single(target_path),
                Some(target_path.to_string()),
                payload.to_vec(),
                vec![],
                String::new(),
                String::new(),
            );
            if let Some(blob_hash) = get_payload_blob(&tmp_patch) {
                new_tree.insert(target_path.to_string(), blob_hash);
            }
        }
        OperationType::Modify => {
            if !new_tree.contains(target_path) {
                return Ok(new_tree);
            }
            let tmp_patch = Patch::new(
                OperationType::Modify,
                TouchSet::single(target_path),
                Some(target_path.to_string()),
                payload.to_vec(),
                vec![],
                String::new(),
                String::new(),
            );
            if let Some(blob_hash) = get_payload_blob(&tmp_patch) {
                new_tree.insert(target_path.to_string(), blob_hash);
            }
        }
        OperationType::Delete => {
            new_tree.remove(target_path);
        }
        OperationType::Move => {
            let new_path = String::from_utf8(payload.to_vec())
                .map_err(|_| ApplyError::Custom("Move payload must be valid UTF-8 path".into()))?;
            new_tree.rename(target_path, new_path);
        }
        OperationType::Metadata => {}
        OperationType::Merge | OperationType::Identity | OperationType::Batch => {}
    }

    Ok(new_tree)
}

/// Apply a chain of patches (from oldest to newest) to produce a final FileTree.
///
/// The patches should be in application order (root first, tip last).
/// This function applies each patch sequentially, threading the FileTree
/// through each transformation.
///
/// # Arguments
///
/// * `patches` - Ordered list of patches to apply (oldest first)
/// * `get_payload_blob` - Function to resolve patch payload to CAS hash
pub fn apply_patch_chain<F>(
    patches: &[Patch],
    mut get_payload_blob: F,
) -> Result<FileTree, ApplyError>
where
    F: FnMut(&Patch) -> Option<suture_common::Hash>,
{
    let mut tree = FileTree::empty();

    for patch in patches {
        tree = apply_patch(&tree, patch, &mut get_payload_blob)?;
    }

    Ok(tree)
}

fn resolve_hex_to_hash(payload: &[u8]) -> Option<suture_common::Hash> {
    if payload.is_empty() {
        return None;
    }
    let hex = std::str::from_utf8(payload).ok()?;
    suture_common::Hash::from_hex(hex).ok()
}

fn apply_single_op_mut(
    tree: &mut FileTree,
    op: &OperationType,
    target_path: &str,
    payload: &[u8],
) {
    match op {
        OperationType::Create => {
            if tree.contains(target_path) {
                return;
            }
            if let Some(blob_hash) = resolve_hex_to_hash(payload) {
                tree.insert(target_path.to_string(), blob_hash);
            }
        }
        OperationType::Modify => {
            if !tree.contains(target_path) {
                return;
            }
            if let Some(blob_hash) = resolve_hex_to_hash(payload) {
                tree.insert(target_path.to_string(), blob_hash);
            }
        }
        OperationType::Delete => {
            tree.remove(target_path);
        }
        OperationType::Move => {
            if let Ok(new_path) = String::from_utf8(payload.to_vec()) {
                tree.rename(target_path, new_path);
            }
        }
        OperationType::Metadata => {}
        OperationType::Merge | OperationType::Identity | OperationType::Batch => {}
    }
}

/// Resolve a patch's payload to a CAS blob hash.
///
/// The payload in suture-core patches stores the hex-encoded BLAKE3 hash
/// of the blob in the CAS. This function parses it back into a Hash.
pub fn resolve_payload_to_hash(patch: &Patch) -> Option<suture_common::Hash> {
    resolve_hex_to_hash(&patch.payload)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patch::types::{FileChange, TouchSet};

    fn make_patch(op: OperationType, path: &str, payload: &[u8]) -> Patch {
        let op_name = format!("{:?}", op);
        Patch::new(
            op,
            TouchSet::single(path),
            Some(path.to_string()),
            payload.to_vec(),
            vec![],
            "test".to_string(),
            format!("{} {}", op_name, path),
        )
    }

    fn blob_hash(data: &[u8]) -> Vec<u8> {
        suture_common::Hash::from_data(data).to_hex().into_bytes()
    }

    #[test]
    fn test_apply_create() {
        let tree = FileTree::empty();
        let data = b"hello world";
        let patch = make_patch(OperationType::Create, "hello.txt", &blob_hash(data));
        let result = apply_patch(&tree, &patch, resolve_payload_to_hash).unwrap();
        assert!(result.contains("hello.txt"));
    }

    #[test]
    fn test_apply_modify() {
        let mut tree = FileTree::empty();
        let old_hash = suture_common::Hash::from_data(b"old content");
        tree.insert("file.txt".to_string(), old_hash);

        let new_data = b"new content";
        let new_hash = suture_common::Hash::from_data(new_data);
        let patch = make_patch(OperationType::Modify, "file.txt", &blob_hash(new_data));
        let result = apply_patch(&tree, &patch, resolve_payload_to_hash).unwrap();
        assert_eq!(result.get("file.txt"), Some(&new_hash));
    }

    #[test]
    fn test_apply_delete() {
        let mut tree = FileTree::empty();
        tree.insert(
            "file.txt".to_string(),
            suture_common::Hash::from_data(b"data"),
        );

        let patch = make_patch(OperationType::Delete, "file.txt", &[]);
        let result = apply_patch(&tree, &patch, resolve_payload_to_hash).unwrap();
        assert!(!result.contains("file.txt"));
        assert!(result.is_empty());
    }

    #[test]
    fn test_apply_move() {
        let mut tree = FileTree::empty();
        let hash = suture_common::Hash::from_data(b"data");
        tree.insert("old.txt".to_string(), hash);

        let patch = make_patch(OperationType::Move, "old.txt", b"new.txt");
        let result = apply_patch(&tree, &patch, resolve_payload_to_hash).unwrap();
        assert!(!result.contains("old.txt"));
        assert!(result.contains("new.txt"));
        assert_eq!(result.get("new.txt"), Some(&hash));
    }

    #[test]
    fn test_apply_identity() {
        let mut tree = FileTree::empty();
        tree.insert(
            "file.txt".to_string(),
            suture_common::Hash::from_data(b"data"),
        );

        let parent = suture_common::Hash::ZERO;
        let identity = Patch::identity(parent, "test".to_string());
        let result = apply_patch(&tree, &identity, resolve_payload_to_hash).unwrap();
        assert_eq!(result, tree);
    }

    #[test]
    fn test_apply_chain() {
        let p1 = make_patch(OperationType::Create, "a.txt", &blob_hash(b"content a"));
        let p2 = make_patch(OperationType::Create, "b.txt", &blob_hash(b"content b"));
        let p3 = make_patch(OperationType::Modify, "a.txt", &blob_hash(b"content a v2"));

        let tree = apply_patch_chain(&[p1, p2, p3], resolve_payload_to_hash).unwrap();
        assert_eq!(tree.len(), 2);
        assert_eq!(
            tree.get("a.txt"),
            Some(&suture_common::Hash::from_data(b"content a v2"))
        );
        assert_eq!(
            tree.get("b.txt"),
            Some(&suture_common::Hash::from_data(b"content b"))
        );
    }

    #[test]
    fn test_apply_chain_with_delete() {
        let p1 = make_patch(OperationType::Create, "a.txt", &blob_hash(b"data"));
        let p2 = make_patch(OperationType::Delete, "a.txt", &[]);

        let tree = apply_patch_chain(&[p1, p2], resolve_payload_to_hash).unwrap();
        assert!(tree.is_empty());
    }

    #[test]
    fn test_resolve_payload_to_hash() {
        let hash = suture_common::Hash::from_data(b"test");
        let patch = make_patch(
            OperationType::Create,
            "file.txt",
            &hash.to_hex().into_bytes(),
        );
        let resolved = resolve_payload_to_hash(&patch).unwrap();
        assert_eq!(resolved, hash);
    }

    #[test]
    fn test_resolve_empty_payload() {
        let patch = make_patch(OperationType::Delete, "file.txt", &[]);
        assert!(resolve_payload_to_hash(&patch).is_none());
    }

    #[test]
    fn test_apply_batch() {
        let tree = FileTree::empty();
        let file_changes = vec![
            FileChange {
                op: OperationType::Create,
                path: "a.txt".to_string(),
                payload: blob_hash(b"content a"),
            },
            FileChange {
                op: OperationType::Create,
                path: "b.txt".to_string(),
                payload: blob_hash(b"content b"),
            },
            FileChange {
                op: OperationType::Modify,
                path: "a.txt".to_string(),
                payload: blob_hash(b"content a v2"),
            },
        ];
        let batch = Patch::new_batch(
            file_changes,
            vec![],
            "test".to_string(),
            "batch commit".to_string(),
        );
        let result = apply_patch(&tree, &batch, resolve_payload_to_hash).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(
            result.get("a.txt"),
            Some(&suture_common::Hash::from_data(b"content a v2"))
        );
        assert_eq!(
            result.get("b.txt"),
            Some(&suture_common::Hash::from_data(b"content b"))
        );
    }

    #[test]
    fn test_apply_batch_with_delete() {
        let mut tree = FileTree::empty();
        tree.insert("a.txt".to_string(), suture_common::Hash::from_data(b"old"));
        tree.insert("b.txt".to_string(), suture_common::Hash::from_data(b"keep"));

        let file_changes = vec![
            FileChange {
                op: OperationType::Modify,
                path: "a.txt".to_string(),
                payload: blob_hash(b"new"),
            },
            FileChange {
                op: OperationType::Delete,
                path: "b.txt".to_string(),
                payload: vec![],
            },
        ];
        let batch = Patch::new_batch(
            file_changes,
            vec![],
            "test".to_string(),
            "batch with delete".to_string(),
        );
        let result = apply_patch(&tree, &batch, resolve_payload_to_hash).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(
            result.get("a.txt"),
            Some(&suture_common::Hash::from_data(b"new"))
        );
        assert!(!result.contains("b.txt"));
    }

    #[test]
    fn test_create_on_existing_path_with_same_hash() {
        let mut tree = FileTree::empty();
        let hash = suture_common::Hash::from_data(b"hello");
        tree.insert("file.txt".to_string(), hash);

        let patch = make_patch(OperationType::Create, "file.txt", &blob_hash(b"hello"));
        let result = apply_patch(&tree, &patch, resolve_payload_to_hash).unwrap();
        assert_eq!(result, tree);
    }

    #[test]
    fn test_create_on_existing_path_with_different_hash() {
        let mut tree = FileTree::empty();
        let original_hash = suture_common::Hash::from_data(b"original");
        tree.insert("file.txt".to_string(), original_hash);

        let patch = make_patch(OperationType::Create, "file.txt", &blob_hash(b"different"));
        let result = apply_patch(&tree, &patch, resolve_payload_to_hash).unwrap();
        assert_eq!(result.get("file.txt"), Some(&original_hash));
    }

    #[test]
    fn test_modify_on_nonexistent_path() {
        let tree = FileTree::empty();
        let patch = make_patch(
            OperationType::Modify,
            "ghost.txt",
            &blob_hash(b"new content"),
        );
        let result = apply_patch(&tree, &patch, resolve_payload_to_hash).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_modify_on_existing_path() {
        let mut tree = FileTree::empty();
        tree.insert(
            "file.txt".to_string(),
            suture_common::Hash::from_data(b"old"),
        );

        let patch = make_patch(OperationType::Modify, "file.txt", &blob_hash(b"new"));
        let result = apply_patch(&tree, &patch, resolve_payload_to_hash).unwrap();
        assert_eq!(
            result.get("file.txt"),
            Some(&suture_common::Hash::from_data(b"new"))
        );
    }

    #[test]
    fn test_delete_on_nonexistent_path() {
        let tree = FileTree::empty();
        let patch = make_patch(OperationType::Delete, "ghost.txt", &[]);
        let result = apply_patch(&tree, &patch, resolve_payload_to_hash).unwrap();
        assert!(result.is_empty());
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

        fn blob_hash_for(h: &Hash) -> Vec<u8> {
            h.to_hex().into_bytes()
        }

        proptest! {
            #[test]
            fn apply_delete_removes_file(path in valid_path(), hash in hash_strategy()) {
                let mut tree = FileTree::empty();
                tree.insert(path.clone(), hash);
                let patch = make_patch(OperationType::Delete, &path, &[]);
                let result = apply_patch(&tree, &patch, resolve_payload_to_hash).unwrap();
                prop_assert!(!result.contains(&path));
            }

            #[test]
            fn apply_create_adds_file(path in valid_path(), hash in hash_strategy()) {
                let tree = FileTree::empty();
                let patch = make_patch(OperationType::Create, &path, &blob_hash_for(&hash));
                let result = apply_patch(&tree, &patch, resolve_payload_to_hash).unwrap();
                prop_assert!(result.contains(&path));
                prop_assert_eq!(result.get(&path), Some(&hash));
            }

            #[test]
            fn apply_modify_updates_hash(
                path in valid_path(),
                hash1 in hash_strategy(),
                hash2 in hash_strategy()
            ) {
                prop_assume!(hash1 != hash2);
                let mut tree = FileTree::empty();
                tree.insert(path.clone(), hash1);
                let patch = make_patch(OperationType::Modify, &path, &blob_hash_for(&hash2));
                let result = apply_patch(&tree, &patch, resolve_payload_to_hash).unwrap();
                prop_assert_eq!(result.get(&path), Some(&hash2));
            }

            #[test]
            fn apply_chain_order_matters(
                path_a in valid_path(),
                path_b in valid_path(),
                hash1 in hash_strategy(),
                hash2 in hash_strategy()
            ) {
                prop_assume!(path_a != path_b);
                let p1 = make_patch(OperationType::Create, &path_a, &blob_hash_for(&hash1));
                let p2 = make_patch(OperationType::Create, &path_b, &blob_hash_for(&hash2));

                let tree_ab = apply_patch_chain(&[p1.clone(), p2.clone()], resolve_payload_to_hash).unwrap();
                prop_assert!(tree_ab.contains(&path_a));
                prop_assert!(tree_ab.contains(&path_b));

                let tree_ba = apply_patch_chain(&[p2.clone(), p1.clone()], resolve_payload_to_hash).unwrap();
                prop_assert!(tree_ba.contains(&path_a));
                prop_assert!(tree_ba.contains(&path_b));
            }
        }
    }
}
