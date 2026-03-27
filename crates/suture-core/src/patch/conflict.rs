//! Conflict types for non-commutative patch pairs.

use crate::patch::types::PatchId;
use crate::patch::types::TouchSet;
use serde::{Deserialize, Serialize};

/// A conflict between two patches that cannot commute.
///
/// A conflict preserves full information from BOTH patches so that
/// a human can resolve it later. Per THM-CONF-001, no data is lost.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Conflict {
    /// The patch from branch A.
    pub patch_a_id: PatchId,
    /// The patch from branch B.
    pub patch_b_id: PatchId,
    /// The addresses where both patches touch (the conflict addresses).
    pub conflict_addresses: Vec<String>,
}

/// A conflict node in the Patch-DAG.
///
/// Unlike a regular patch, a conflict node stores references to two
/// conflicting patches and the base state. It can be resolved later
/// by choosing one side or manually merging.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConflictNode {
    /// The patch from branch A.
    pub patch_a_id: PatchId,
    /// The patch from branch B.
    pub patch_b_id: PatchId,
    /// The base (common ancestor) patch ID.
    pub base_patch_id: PatchId,
    /// The addresses where the conflict occurs.
    pub touch_set: TouchSet,
    /// Human-readable description of the conflict.
    pub description: String,
    /// Resolution status.
    pub status: ConflictStatus,
}

/// The resolution status of a conflict.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConflictStatus {
    /// Conflict is unresolved.
    Unresolved,
    /// Conflict was resolved by choosing side A.
    ResolvedA,
    /// Conflict was resolved by choosing side B.
    ResolvedB,
    /// Conflict was resolved with a manual merge.
    ResolvedManual,
}

impl Conflict {
    /// Create a new conflict between two patches.
    pub fn new(patch_a_id: PatchId, patch_b_id: PatchId, conflict_addresses: Vec<String>) -> Self {
        let mut sorted = conflict_addresses.clone();
        sorted.sort();
        Self {
            patch_a_id,
            patch_b_id,
            conflict_addresses: sorted,
        }
    }
}

impl ConflictNode {
    /// Create a new unresolved conflict node.
    pub fn new(
        patch_a_id: PatchId,
        patch_b_id: PatchId,
        base_patch_id: PatchId,
        touch_set: TouchSet,
        description: String,
    ) -> Self {
        Self {
            patch_a_id,
            patch_b_id,
            base_patch_id,
            touch_set,
            description,
            status: ConflictStatus::Unresolved,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use suture_common::Hash;

    fn test_hash(s: &str) -> PatchId {
        Hash::from_data(s.as_bytes())
    }

    #[test]
    fn test_conflict_creation() {
        let c = Conflict::new(
            test_hash("patch_a"),
            test_hash("patch_b"),
            vec!["A1".to_string(), "B1".to_string()],
        );
        assert_eq!(c.conflict_addresses.len(), 2);
    }

    #[test]
    fn test_conflict_node_creation() {
        let node = ConflictNode::new(
            test_hash("patch_a"),
            test_hash("patch_b"),
            test_hash("base"),
            TouchSet::from_addrs(["A1", "B1"]),
            "Both edited A1".to_string(),
        );
        assert_eq!(node.status, ConflictStatus::Unresolved);
        assert_eq!(node.touch_set.len(), 2);
    }
}
