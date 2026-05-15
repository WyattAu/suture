//! # Patch Types
//!
//! Core types for the patch algebra that underpins Suture's version control.
//!
//! A `Patch` represents a single atomic change to the repository. Patches form
//! a directed acyclic graph (DAG) through parent references.
//!
//! A `Patch` transforms a project state by modifying a set of addresses
//! (its "touch set"). Patches are immutable once created.
//! Each patch has:
//! - A unique identifier (BLAKE3 hash of its content)
//! - A set of parent patch IDs (typically one, two for merge commits)
//! - An operation type and payload
//! - A touch set (the addresses/resources this patch modifies)
//! - Metadata (timestamp, author, message)

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};
use suture_common::Hash;

/// Unique identifier for a patch (BLAKE3 hash of serialized patch content).
pub type PatchId = Hash;

/// A single file change within a batched commit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileChange {
    /// The type of operation on this file.
    pub op: OperationType,
    /// The file path affected.
    pub path: String,
    /// The payload (hex-encoded blob hash for Create/Modify, empty for Delete).
    pub payload: Vec<u8>,
}

/// The type of operation a patch represents.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OperationType {
    /// Create a new file or resource.
    Create,
    /// Delete a file or resource.
    Delete,
    /// Modify the content of a file or resource.
    Modify,
    /// Move/rename a file or resource.
    Move,
    /// Update metadata (permissions, timestamps, etc.).
    Metadata,
    /// A merge commit (combines two or more parent patches).
    Merge,
    /// No-op / identity patch.
    Identity,
    /// A batched commit containing multiple file changes.
    /// The payload contains a JSON-serialized `Vec<FileChange>`.
    Batch,
}

impl std::fmt::Display for OperationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Create => write!(f, "create"),
            Self::Delete => write!(f, "delete"),
            Self::Modify => write!(f, "modify"),
            Self::Move => write!(f, "move"),
            Self::Metadata => write!(f, "metadata"),
            Self::Merge => write!(f, "merge"),
            Self::Identity => write!(f, "identity"),
            Self::Batch => write!(f, "batch"),
        }
    }
}

/// A touch set — the set of addresses/resources that a patch modifies.
///
/// Touch sets are the basis for commutativity detection:
/// two patches commute if and only if their touch sets are disjoint.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TouchSet {
    inner: BTreeSet<String>,
}

impl TouchSet {
    /// Create an empty touch set (identity patch).
    #[must_use]
    pub fn empty() -> Self {
        Self {
            inner: BTreeSet::new(),
        }
    }

    /// Create a touch set from a single address.
    pub fn single(addr: impl Into<String>) -> Self {
        let mut set = BTreeSet::new();
        set.insert(addr.into());
        Self { inner: set }
    }

    /// Create a touch set from a list of addresses.
    pub fn from_addrs(addrs: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            inner: addrs.into_iter().map(Into::into).collect(),
        }
    }

    /// Check if this touch set is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = &String> {
        self.inner.iter()
    }

    #[inline]
    pub fn insert(&mut self, addr: impl Into<String>) {
        self.inner.insert(addr.into());
    }

    #[inline]
    #[must_use]
    pub fn intersects(&self, other: &Self) -> bool {
        self.inner.intersection(&other.inner).next().is_some()
    }

    /// Get the intersection of two touch sets.
    #[must_use]
    pub fn intersection(&self, other: &Self) -> Self {
        Self {
            inner: self.inner.intersection(&other.inner).cloned().collect(),
        }
    }

    /// Get all addresses as a sorted Vec.
    #[must_use]
    pub fn addresses(&self) -> Vec<String> {
        self.inner.iter().cloned().collect()
    }

    #[inline]
    #[must_use]
    pub fn contains(&self, addr: &str) -> bool {
        self.inner.contains(addr)
    }

    /// Compute the union of two touch sets.
    #[must_use]
    pub fn union(&self, other: &Self) -> Self {
        Self {
            inner: self.inner.union(&other.inner).cloned().collect(),
        }
    }

    /// Subtract addresses from this touch set.
    #[must_use]
    pub fn subtract(&self, other: &Self) -> Self {
        Self {
            inner: self.inner.difference(&other.inner).cloned().collect(),
        }
    }
}

/// A patch — the fundamental unit of change in Suture.
///
/// A patch transforms a project state by modifying a set of addresses
/// (its "touch set"). Patches are immutable once created.
///
/// Patches form a DAG through `parent_ids`. Each patch has:
/// - A unique BLAKE3 hash (`id`)
/// - Zero or more parent patches
/// - An operation type (create, modify, delete, rename, merge)
/// - A touch set (files affected)
/// - An optional payload (file content or metadata)
#[derive(Debug, Serialize, Deserialize)]
pub struct Patch {
    /// Unique identifier (BLAKE3 hash of patch content).
    pub id: PatchId,

    /// Parent patch IDs. Typically one parent; merge patches have two.
    pub parent_ids: Vec<PatchId>,

    /// The type of operation this patch performs.
    pub operation_type: OperationType,

    /// The set of addresses this patch modifies.
    /// Used for commutativity and conflict detection.
    pub touch_set: TouchSet,

    /// The target file path (if applicable).
    pub target_path: Option<String>,

    /// The operation payload (serialized operation data).
    /// Format depends on the operation type and the driver.
    pub payload: Vec<u8>,

    /// Creation timestamp (Unix epoch seconds).
    pub timestamp: u64,

    /// Author identifier.
    pub author: String,

    /// Human-readable description of the change.
    pub message: String,

    /// Cached deserialization of batch file changes.
    /// Populated lazily on first call to `file_changes()`.
    /// Reset on clone (clone is rare; re-parse is cheap).
    #[serde(skip)]
    pub(crate) cached_file_changes: OnceLock<Option<Vec<FileChange>>>,
}

impl Clone for Patch {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            parent_ids: self.parent_ids.clone(),
            operation_type: self.operation_type,
            touch_set: self.touch_set.clone(),
            target_path: self.target_path.clone(),
            payload: self.payload.clone(),
            timestamp: self.timestamp,
            author: self.author.clone(),
            message: self.message.clone(),
            cached_file_changes: OnceLock::new(),
        }
    }
}

impl Patch {
    /// Create a new patch.
    #[must_use]
    pub fn new(
        operation_type: OperationType,
        touch_set: TouchSet,
        target_path: Option<String>,
        payload: Vec<u8>,
        parent_ids: Vec<PatchId>,
        author: String,
        message: String,
    ) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        // Build the patch without ID first, then compute ID from content
        let mut patch = Self {
            id: Hash::ZERO, // Placeholder
            parent_ids,
            operation_type,
            touch_set,
            target_path,
            payload,
            timestamp,
            author,
            message,
            cached_file_changes: OnceLock::new(),
        };

        // Compute ID from the patch's content (everything except the ID itself)
        patch.id = patch.compute_id();
        patch
    }

    /// Create a patch with an explicit ID (used by Hub when reconstructing from proto/network).
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn with_id(
        id: Hash,
        operation_type: OperationType,
        touch_set: TouchSet,
        target_path: Option<String>,
        payload: Vec<u8>,
        parent_ids: Vec<PatchId>,
        author: String,
        message: String,
        timestamp: u64,
    ) -> Self {
        Self {
            id,
            parent_ids,
            operation_type,
            touch_set,
            target_path,
            payload,
            timestamp,
            author,
            message,
            cached_file_changes: OnceLock::new(),
        }
    }

    /// Create an identity (no-op) patch.
    #[must_use]
    pub fn identity(parent: PatchId, author: String) -> Self {
        Self::new(
            OperationType::Identity,
            TouchSet::empty(),
            None,
            Vec::new(),
            vec![parent],
            author,
            "identity".to_owned(),
        )
    }

    /// Compute the BLAKE3 hash of this patch's content.
    ///
    /// The hash is computed over all fields except the ID itself,
    /// providing content-addressed patch identification.
    fn compute_id(&self) -> Hash {
        use crate::cas::hasher::hash_with_context;
        let data = serde_json::to_vec(&PatchForHash {
            parent_ids: &self.parent_ids,
            operation_type: &self.operation_type,
            touch_set: &self.touch_set,
            target_path: &self.target_path,
            payload: &self.payload,
            author: &self.author,
            message: &self.message,
        })
        .unwrap_or_default();
        hash_with_context("suture-patch", &data)
    }

    /// Check if this is an identity (no-op) patch.
    #[must_use]
    pub fn is_identity(&self) -> bool {
        self.operation_type == OperationType::Identity && self.touch_set.is_empty()
    }

    /// Create a batched commit patch that groups multiple file changes.
    #[must_use]
    pub fn new_batch(
        mut file_changes: Vec<FileChange>,
        parent_ids: Vec<PatchId>,
        author: String,
        message: String,
    ) -> Self {
        // Sort for deterministic hashing regardless of caller order.
        // The payload is part of the patch ID hash, so ordering must be stable.
        file_changes.sort_by(|a, b| a.path.cmp(&b.path));
        let touch_set = TouchSet::from_addrs(file_changes.iter().map(|fc| fc.path.clone()));
        let payload = serde_json::to_vec(&file_changes).unwrap_or_default();
        Self::new(
            OperationType::Batch,
            touch_set,
            None,
            payload,
            parent_ids,
            author,
            message,
        )
    }

    /// If this is a Batch patch, deserialize the file changes.
    ///
    /// The result is cached in `cached_file_changes` after the first call,
    /// so repeated calls on the same patch are O(1) instead of re-parsing JSON.
    #[must_use]
    pub fn file_changes(&self) -> Option<Vec<FileChange>> {
        if self.operation_type == OperationType::Batch {
            self.cached_file_changes
                .get_or_init(|| serde_json::from_slice(&self.payload).ok())
                .clone()
        } else {
            None
        }
    }

    /// Whether this patch is a batched commit.
    #[must_use]
    pub fn is_batch(&self) -> bool {
        self.operation_type == OperationType::Batch
    }
}

/// Helper struct for serializing a patch for hashing (excludes the ID field).
///
/// NOTE: `timestamp` is intentionally excluded from the hash so that the same
/// logical patch created at different wall-clock times produces the same PatchId.
/// This is critical for reproducibility and deterministic merge behavior.
#[derive(Serialize)]
struct PatchForHash<'a> {
    parent_ids: &'a Vec<PatchId>,
    operation_type: &'a OperationType,
    touch_set: &'a TouchSet,
    target_path: &'a Option<String>,
    payload: &'a Vec<u8>,
    author: &'a String,
    message: &'a String,
}

/// A general operation that can be applied to a state.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Operation {
    /// The operation type.
    pub op_type: OperationType,
    /// The file/resource path this operation targets.
    pub path: String,
    /// The addresses this operation modifies (for conflict detection).
    pub addresses: Vec<String>,
    /// The operation data (format-specific).
    pub data: Vec<u8>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_patch(addr: &str, author: &str) -> Patch {
        Patch::new(
            OperationType::Modify,
            TouchSet::single(addr),
            Some(format!("file_{}", addr)),
            vec![],
            vec![],
            author.to_string(),
            format!("edit {}", addr),
        )
    }

    #[test]
    fn test_patch_deterministic_id() {
        let p1 = make_test_patch("A1", "alice");
        let p2 = make_test_patch("A1", "alice");
        assert_eq!(p1.id, p2.id, "Same patch content must produce same ID");
    }

    #[test]
    fn test_patch_different_id() {
        let p1 = make_test_patch("A1", "alice");
        let p2 = make_test_patch("B1", "alice");
        assert_ne!(p1.id, p2.id, "Different patches must have different IDs");
    }

    #[test]
    fn test_touch_set_empty() {
        let ts = TouchSet::empty();
        assert!(ts.is_empty());
        assert_eq!(ts.len(), 0);
    }

    #[test]
    fn test_touch_set_intersects() {
        let ts1 = TouchSet::from_addrs(["A1", "B1", "C1"]);
        let ts2 = TouchSet::from_addrs(["C1", "D1", "E1"]);
        let ts3 = TouchSet::from_addrs(["X1", "Y1"]);

        assert!(
            ts1.intersects(&ts2),
            "A1,B1,C1 and C1,D1,E1 should intersect at C1"
        );
        assert!(
            !ts1.intersects(&ts3),
            "A1,B1,C1 and X1,Y1 should not intersect"
        );
    }

    #[test]
    fn test_touch_set_intersection() {
        let ts1 = TouchSet::from_addrs(["A1", "B1", "C1"]);
        let ts2 = TouchSet::from_addrs(["C1", "D1"]);
        let intersection = ts1.intersection(&ts2);
        assert_eq!(intersection.len(), 1);
        assert!(intersection.contains("C1"));
    }

    #[test]
    fn test_identity_patch() {
        let parent = Hash::from_hex(&"0".repeat(64)).unwrap();
        let id = Patch::identity(parent, "alice".to_string());
        assert!(id.is_identity());
        assert!(id.touch_set.is_empty());
    }

    #[test]
    fn test_patch_serialization_roundtrip() {
        let patch = make_test_patch("X1", "bob");
        let json = serde_json::to_string(&patch).unwrap();
        let deserialized: Patch = serde_json::from_str(&json).unwrap();
        assert_eq!(patch.id, deserialized.id);
        assert_eq!(patch.touch_set, deserialized.touch_set);
    }

    #[test]
    fn test_touch_set_union() {
        let ts1 = TouchSet::from_addrs(["A1", "B1", "C1"]);
        let ts2 = TouchSet::from_addrs(["C1", "D1", "E1"]);
        let union = ts1.union(&ts2);
        assert_eq!(union.len(), 5);
        assert!(union.contains("A1"));
        assert!(union.contains("B1"));
        assert!(union.contains("C1"));
        assert!(union.contains("D1"));
        assert!(union.contains("E1"));
    }

    #[test]
    fn test_touch_set_union_empty() {
        let ts1 = TouchSet::from_addrs(["A1"]);
        let ts2 = TouchSet::empty();
        let union = ts1.union(&ts2);
        assert_eq!(union.len(), 1);
        assert!(union.contains("A1"));
    }

    #[test]
    fn test_touch_set_subtract() {
        let ts1 = TouchSet::from_addrs(["A1", "B1", "C1", "D1"]);
        let ts2 = TouchSet::from_addrs(["B1", "D1"]);
        let result = ts1.subtract(&ts2);
        assert_eq!(result.len(), 2);
        assert!(result.contains("A1"));
        assert!(result.contains("C1"));
        assert!(!result.contains("B1"));
        assert!(!result.contains("D1"));
    }

    #[test]
    fn test_touch_set_subtract_empty() {
        let ts1 = TouchSet::from_addrs(["A1", "B1"]);
        let ts2 = TouchSet::empty();
        let result = ts1.subtract(&ts2);
        assert_eq!(result.len(), 2);
    }

    mod proptests {
        use super::*;
        use proptest::prelude::*;

        #[test]
        fn test_batch_patch_deterministic_regardless_of_order() {
            let changes_a = vec![
                FileChange {
                    path: "z.txt".into(),
                    op: OperationType::Create,
                    payload: b"z".to_vec(),
                },
                FileChange {
                    path: "a.txt".into(),
                    op: OperationType::Create,
                    payload: b"a".to_vec(),
                },
            ];
            let changes_b = vec![
                FileChange {
                    path: "a.txt".into(),
                    op: OperationType::Create,
                    payload: b"a".to_vec(),
                },
                FileChange {
                    path: "z.txt".into(),
                    op: OperationType::Create,
                    payload: b"z".to_vec(),
                },
            ];
            let p1 = Patch::new_batch(changes_a, vec![], "alice".into(), "batch".into());
            let p2 = Patch::new_batch(changes_b, vec![], "alice".into(), "batch".into());
            assert_eq!(
                p1.id, p2.id,
                "batch patches with same changes in different order must have same ID"
            );
        }

        proptest! {
            #[test]
            fn patch_id_deterministic(addr in "[a-zA-Z0-9]{1,50}") {
                let p1 = Patch::new(
                    OperationType::Modify,
                    TouchSet::single(&addr),
                    Some(format!("file_{}", addr)),
                    addr.clone().into_bytes(),
                    vec![],
                    "proptest".to_string(),
                    format!("edit {}", addr),
                );
                let p2 = Patch::new(
                    OperationType::Modify,
                    TouchSet::single(&addr),
                    Some(format!("file_{}", addr)),
                    addr.clone().into_bytes(),
                    vec![],
                    "proptest".to_string(),
                    format!("edit {}", addr),
                );
                prop_assert_eq!(p1.id, p2.id);
            }

            #[test]
            fn touch_set_insert_contains(addr in "[a-z]{1,30}") {
                let mut ts = TouchSet::empty();
                ts.insert(&addr);
                prop_assert!(ts.contains(&addr));
                prop_assert_eq!(ts.len(), 1);
            }

            #[test]
            fn touch_set_union_sizes(
                n1 in 0usize..20,
                n2 in 0usize..20,
                seed in 0u64..10_000
            ) {
                let mut rng = simple_rng(seed);
                let addrs1: Vec<String> = (0..n1).map(|i| format!("addr_{:04}_{}", rng(), i)).collect();
                let addrs2: Vec<String> = (0..n2).map(|i| format!("addr_{:04}_{}", rng(), i)).collect();
                let ts1 = TouchSet::from_addrs(addrs1.iter());
                let ts2 = TouchSet::from_addrs(addrs2.iter());
                let union = ts1.union(&ts2);
                let expected = addrs1.iter().chain(addrs2.iter()).collect::<std::collections::HashSet<_>>();
                prop_assert_eq!(union.len(), expected.len());
            }

            #[test]
            fn touch_set_intersection_symmetric(
                addrs1 in proptest::collection::vec("[a-z]{1,5}", 0..20),
                addrs2 in proptest::collection::vec("[a-z]{1,5}", 0..20),
            ) {
                let ts1 = TouchSet::from_addrs(addrs1.iter());
                let ts2 = TouchSet::from_addrs(addrs2.iter());
                let inter1 = ts1.intersection(&ts2);
                let inter2 = ts2.intersection(&ts1);
                prop_assert_eq!(inter1.len(), inter2.len());
            }

            #[test]
            fn touch_set_subtract_removes(
                addrs1 in proptest::collection::vec("[a-z]{1,5}", 1..20),
                addrs2 in proptest::collection::vec("[a-z]{1,5}", 0..20),
            ) {
                let ts1 = TouchSet::from_addrs(addrs1.iter());
                let ts2 = TouchSet::from_addrs(addrs2.iter());
                let result = ts1.subtract(&ts2);
                for addr in &addrs2 {
                    prop_assert!(!result.contains(addr));
                }
            }

            #[test]
            fn disjoint_touch_sets_commute(
                addr1 in "[a-z]{1,10}",
                addr2 in "[a-z]{1,10}",
            ) {
                prop_assume!(addr1 != addr2);
                let p1 = Patch::new(
                    OperationType::Modify,
                    TouchSet::single(&addr1),
                    Some(format!("file_{}", addr1)),
                    vec![],
                    vec![],
                    "proptest".to_string(),
                    format!("edit {}", addr1),
                );
                let p2 = Patch::new(
                    OperationType::Modify,
                    TouchSet::single(&addr2),
                    Some(format!("file_{}", addr2)),
                    vec![],
                    vec![],
                    "proptest".to_string(),
                    format!("edit {}", addr2),
                );
                prop_assert!(!p1.touch_set.intersects(&p2.touch_set));
            }

            #[test]
            fn same_addr_touch_sets_overlap(addr in "[a-z]{1,10}") {
                let ts1 = TouchSet::single(&addr);
                let ts2 = TouchSet::single(&addr);
                prop_assert!(ts1.intersects(&ts2));
                let inter = ts1.intersection(&ts2);
                prop_assert_eq!(inter.len(), 1);
                prop_assert!(inter.contains(&addr));
            }
        }

        fn simple_rng(mut seed: u64) -> impl FnMut() -> u64 {
            move || {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                seed
            }
        }
    }
}
