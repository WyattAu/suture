//! Conflict types for non-commutative patch pairs.

use crate::patch::types::Patch;
use crate::patch::types::PatchId;
use crate::patch::types::TouchSet;
use serde::{Deserialize, Serialize};

/// Classification of a conflict's severity and resolvability.
///
/// Per THM-CONFCLASS-001 (YP-ALGEBRA-PATCH-001):
/// Conflict classification determines the merge strategy:
/// - Type I: Auto-resolve (identical changes)
/// - Type II: Attempt driver merge
/// - Type III: Human intervention required
/// - Type IV: Structural restructuring (highest complexity)
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConflictClass {
    /// Both sides made the identical change — auto-resolvable.
    AutoResolvable,
    /// Both changed different aspects of the same element — may be driver-resolvable.
    DriverResolvable,
    /// Both changed the same element differently — genuine conflict.
    Genuine,
    /// One side restructured, other modified — structural conflict.
    Structural,
}

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
    #[must_use] 
    pub fn new(patch_a_id: PatchId, patch_b_id: PatchId, conflict_addresses: Vec<String>) -> Self {
        let mut sorted = conflict_addresses;
        sorted.sort();
        Self {
            patch_a_id,
            patch_b_id,
            conflict_addresses: sorted,
        }
    }

    /// Classify this conflict based on patch payloads.
    ///
    /// Returns a ConflictClass indicating the severity and potential resolvability.
    #[must_use] 
    pub fn classify(&self, patch_a: Option<&Patch>, patch_b: Option<&Patch>) -> ConflictClass {
        match (patch_a, patch_b) {
            (Some(pa), Some(pb)) => {
                if pa.payload == pb.payload && pa.operation_type == pb.operation_type {
                    ConflictClass::AutoResolvable
                } else if pa.operation_type == pb.operation_type {
                    let a_sub: Vec<String> = self
                        .conflict_addresses
                        .iter()
                        .filter(|a| pa.touch_set.contains(a))
                        .cloned()
                        .collect();
                    let b_sub: Vec<String> = self
                        .conflict_addresses
                        .iter()
                        .filter(|a| pb.touch_set.contains(a))
                        .cloned()
                        .collect();
                    if a_sub == b_sub {
                        ConflictClass::Genuine
                    } else {
                        ConflictClass::DriverResolvable
                    }
                } else {
                    ConflictClass::Structural
                }
            }
            _ => ConflictClass::Genuine,
        }
    }
}

impl ConflictNode {
    /// Create a new unresolved conflict node.
    #[must_use] 
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

    #[test]
    fn test_classify_auto_resolvable() {
        let root = Hash::from_data(b"root");
        let pa = Patch::new(
            crate::patch::types::OperationType::Modify,
            TouchSet::from_addrs(["f1"]),
            Some("f1".to_string()),
            b"same content".to_vec(),
            vec![root],
            "alice".to_string(),
            "edit".to_string(),
        );
        let pb = Patch::new(
            crate::patch::types::OperationType::Modify,
            TouchSet::from_addrs(["f1"]),
            Some("f1".to_string()),
            b"same content".to_vec(),
            vec![root],
            "bob".to_string(),
            "edit".to_string(),
        );

        let conflict = Conflict::new(pa.id, pb.id, vec!["f1".to_string()]);
        assert_eq!(
            conflict.classify(Some(&pa), Some(&pb)),
            ConflictClass::AutoResolvable
        );
    }

    #[test]
    fn test_classify_genuine() {
        let root = Hash::from_data(b"root");
        let pa = Patch::new(
            crate::patch::types::OperationType::Modify,
            TouchSet::from_addrs(["f1"]),
            Some("f1".to_string()),
            b"version A".to_vec(),
            vec![root],
            "alice".to_string(),
            "edit A".to_string(),
        );
        let pb = Patch::new(
            crate::patch::types::OperationType::Modify,
            TouchSet::from_addrs(["f1"]),
            Some("f1".to_string()),
            b"version B".to_vec(),
            vec![root],
            "bob".to_string(),
            "edit B".to_string(),
        );

        let conflict = Conflict::new(pa.id, pb.id, vec!["f1".to_string()]);
        assert_eq!(
            conflict.classify(Some(&pa), Some(&pb)),
            ConflictClass::Genuine
        );
    }

    #[test]
    fn test_classify_structural() {
        let root = Hash::from_data(b"root");
        let pa = Patch::new(
            crate::patch::types::OperationType::Modify,
            TouchSet::from_addrs(["f1"]),
            Some("f1".to_string()),
            b"modified content".to_vec(),
            vec![root],
            "alice".to_string(),
            "modify".to_string(),
        );
        let pb = Patch::new(
            crate::patch::types::OperationType::Delete,
            TouchSet::from_addrs(["f1"]),
            Some("f1".to_string()),
            vec![],
            vec![root],
            "bob".to_string(),
            "delete".to_string(),
        );

        let conflict = Conflict::new(pa.id, pb.id, vec!["f1".to_string()]);
        assert_eq!(
            conflict.classify(Some(&pa), Some(&pb)),
            ConflictClass::Structural
        );
    }

    #[test]
    fn test_classify_driver_resolvable() {
        let root = Hash::from_data(b"root");
        let pa = Patch::new(
            crate::patch::types::OperationType::Modify,
            TouchSet::from_addrs(["f1.key_a"]),
            Some("f1".to_string()),
            b"val_a".to_vec(),
            vec![root],
            "alice".to_string(),
            "edit key_a".to_string(),
        );
        let pb = Patch::new(
            crate::patch::types::OperationType::Modify,
            TouchSet::from_addrs(["f1.key_b"]),
            Some("f1".to_string()),
            b"val_b".to_vec(),
            vec![root],
            "bob".to_string(),
            "edit key_b".to_string(),
        );

        let conflict = Conflict::new(
            pa.id,
            pb.id,
            vec!["f1.key_a".to_string(), "f1.key_b".to_string()],
        );
        assert_eq!(
            conflict.classify(Some(&pa), Some(&pb)),
            ConflictClass::DriverResolvable
        );
    }

    #[test]
    fn test_classify_missing_patches() {
        let conflict = Conflict::new(test_hash("a"), test_hash("b"), vec!["f1".to_string()]);
        assert_eq!(conflict.classify(None, None), ConflictClass::Genuine);
        assert_eq!(conflict.classify(None, None), ConflictClass::Genuine);
    }
}
