//! Three-way merge algorithm for patch sets.
//!
//! Given a common base and two branches (A and B), the merge algorithm:
//! 1. Identifies patches unique to each branch
//! 2. Checks for conflicts (overlapping touch sets)
//! 3. Produces a merged patch set with conflict nodes where needed
//!
//! # Correctness
//!
//! Per THM-MERGE-001 (YP-ALGEBRA-PATCH-001):
//! The merge result is deterministic and independent of branch processing order.

use crate::patch::commute::commute;
use crate::patch::conflict::Conflict;
use crate::patch::types::{Patch, PatchId};
use std::collections::{HashMap, HashSet};
use thiserror::Error;

/// Errors that can occur during merge operations.
#[derive(Error, Debug)]
pub enum MergeError {
    #[error("patch not found: {0}")]
    PatchNotFound(String),

    #[error("no common ancestor found between branches")]
    NoCommonAncestor,

    #[error("merge already in progress")]
    MergeInProgress,

    #[error("empty branch: {0}")]
    EmptyBranch(String),
}

/// Result of a merge operation.
#[derive(Clone, Debug)]
pub struct MergeResult {
    /// Patches that are in both branches (already applied, include once).
    pub common_patches: Vec<PatchId>,
    /// Patches unique to branch A (applied in order).
    pub patches_a_only: Vec<PatchId>,
    /// Patches unique to branch B (applied in order).
    pub patches_b_only: Vec<PatchId>,
    /// Conflicts detected between patches from different branches.
    pub conflicts: Vec<Conflict>,
    /// Whether the merge is clean (no conflicts).
    pub is_clean: bool,
}

impl MergeResult {
    /// Get all patch IDs that should be in the merged result.
    #[must_use]
    pub fn all_patch_ids(&self) -> Vec<PatchId> {
        let mut ids = Vec::with_capacity(
            self.common_patches.len() + self.patches_a_only.len() + self.patches_b_only.len(),
        );
        ids.extend(self.common_patches.iter());
        ids.extend(self.patches_a_only.iter());
        ids.extend(self.patches_b_only.iter());
        ids
    }
}

/// Perform a three-way merge of two patch sets.
///
/// # Algorithm (ALG-MERGE-001)
///
/// 1. Compute unique patches on each branch: patches not in the base
/// 2. For each pair (P_a, P_b) where P_a is unique to A and P_b is unique to B:
///    a. Check if they commute (disjoint touch sets)
///    b. If not, create a conflict node
/// 3. Return the merged patch set + conflicts
///
/// # Arguments
///
/// * `base_patches` - Patches in the common ancestor
/// * `branch_a_patches` - Patches on branch A (in application order)
/// * `branch_b_patches` - Patches on branch B (in application order)
/// * `all_patches` - HashMap of PatchId -> Patch for looking up patch details
pub fn merge(
    base_patches: &[PatchId],
    branch_a_patches: &[PatchId],
    branch_b_patches: &[PatchId],
    #[allow(clippy::implicit_hasher)] all_patches: &HashMap<PatchId, Patch>,
) -> Result<MergeResult, MergeError> {
    let base_set: HashSet<&PatchId> = base_patches.iter().collect();

    // Partition patches into common and unique
    let patches_a_only: Vec<PatchId> = branch_a_patches
        .iter()
        .filter(|p| !base_set.contains(p))
        .copied()
        .collect();

    let patches_b_only: Vec<PatchId> = branch_b_patches
        .iter()
        .filter(|p| !base_set.contains(p))
        .copied()
        .collect();

    // Common patches (in base, also in both branches)
    let branch_a_set: HashSet<&PatchId> = branch_a_patches.iter().collect();
    let branch_b_set: HashSet<&PatchId> = branch_b_patches.iter().collect();
    let common_patches: Vec<PatchId> = base_patches
        .iter()
        .filter(|p| branch_a_set.contains(p) && branch_b_set.contains(p))
        .copied()
        .collect();

    let mut conflicts = Vec::new();

    let mut b_index: HashMap<&str, Vec<&PatchId>> = HashMap::new();
    for patch_b_id in &patches_b_only {
        if let Some(patch_b) = all_patches.get(patch_b_id) {
            if patch_b.is_identity() {
                continue;
            }
            for addr in patch_b.touch_set.iter() {
                b_index.entry(addr.as_str()).or_default().push(patch_b_id);
            }
        }
    }

    for patch_a_id in &patches_a_only {
        let patch_a = all_patches
            .get(patch_a_id)
            .ok_or_else(|| MergeError::PatchNotFound(patch_a_id.to_hex()))?;

        if patch_a.is_identity() {
            continue;
        }

        let mut b_candidates: HashSet<&PatchId> = HashSet::new();
        for addr in patch_a.touch_set.iter() {
            if let Some(ids) = b_index.get(addr.as_str()) {
                for id in ids {
                    b_candidates.insert(id);
                }
            }
        }

        let mut b_candidates: Vec<_> = b_candidates.into_iter().collect();
        b_candidates.sort();

        for patch_b_id in &b_candidates {
            let patch_b = all_patches
                .get(patch_b_id)
                .ok_or_else(|| MergeError::PatchNotFound(patch_b_id.to_hex()))?;

            match commute(patch_a, patch_b) {
                crate::patch::CommuteResult::DoesNotCommute { conflict_addresses } => {
                    conflicts.push(Conflict::new(*patch_a_id, **patch_b_id, conflict_addresses));
                }
                crate::patch::CommuteResult::Commutes => {}
            }
        }
    }

    let is_clean = conflicts.is_empty();

    Ok(MergeResult {
        common_patches,
        patches_a_only,
        patches_b_only,
        conflicts,
        is_clean,
    })
}

/// Detect all conflicts between two patch sets without performing a full merge.
///
/// This is useful for showing a preview of what would conflict before
/// actually committing to a merge.
#[must_use]
pub fn detect_conflicts(patches_a: &[Patch], patches_b: &[Patch]) -> Vec<Conflict> {
    let mut conflicts = Vec::new();

    let mut b_index: HashMap<&str, Vec<usize>> = HashMap::new();
    for (idx, patch_b) in patches_b.iter().enumerate() {
        if patch_b.is_identity() {
            continue;
        }
        for addr in patch_b.touch_set.iter() {
            b_index.entry(addr.as_str()).or_default().push(idx);
        }
    }

    for patch_a in patches_a {
        if patch_a.is_identity() {
            continue;
        }
        let mut b_candidates: HashSet<usize> = HashSet::new();
        for addr in patch_a.touch_set.iter() {
            if let Some(indices) = b_index.get(addr.as_str()) {
                for &idx in indices {
                    b_candidates.insert(idx);
                }
            }
        }
        let mut b_candidates: Vec<_> = b_candidates.into_iter().collect();
        b_candidates.sort();

        for &idx in &b_candidates {
            let patch_b = &patches_b[idx];
            match commute(patch_a, patch_b) {
                crate::patch::CommuteResult::DoesNotCommute { conflict_addresses } => {
                    conflicts.push(Conflict::new(patch_a.id, patch_b.id, conflict_addresses));
                }
                crate::patch::CommuteResult::Commutes => {}
            }
        }
    }

    conflicts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patch::types::{OperationType, Patch, TouchSet};

    fn patch(addr: &str, name: &str) -> Patch {
        Patch::new(
            OperationType::Modify,
            TouchSet::single(addr),
            Some(format!("file_{}", addr)),
            vec![],
            vec![],
            name.to_string(),
            format!("edit {}", addr),
        )
    }

    fn make_patches(patches: &[Patch]) -> (Vec<PatchId>, HashMap<PatchId, Patch>) {
        let ids: Vec<PatchId> = patches.iter().map(|p| p.id).collect();
        let map: HashMap<PatchId, Patch> = patches.iter().map(|p| (p.id, p.clone())).collect();
        (ids, map)
    }

    #[test]
    fn test_clean_merge_disjoint() {
        let base = patch("Z0", "base");
        let pa = patch("A1", "branch_a");
        let pb = patch("B1", "branch_b");

        let (base_ids, mut all) = make_patches(std::slice::from_ref(&base));
        let (a_ids, a_map) = make_patches(&[base.clone(), pa.clone()]);
        let (b_ids, b_map) = make_patches(&[base.clone(), pb.clone()]);

        all.extend(a_map);
        all.extend(b_map);

        let result = merge(&base_ids, &a_ids, &b_ids, &all).unwrap();
        assert!(result.is_clean);
        assert!(result.conflicts.is_empty());
        assert!(result.patches_a_only.contains(&pa.id));
        assert!(result.patches_b_only.contains(&pb.id));
    }

    #[test]
    fn test_conflicting_merge() {
        let base = patch("Z0", "base");
        let pa = patch("A1", "branch_a");
        let pb = patch("A1", "branch_b"); // Same address!

        let (base_ids, mut all) = make_patches(std::slice::from_ref(&base));
        let (a_ids, a_map) = make_patches(&[base.clone(), pa.clone()]);
        let (b_ids, b_map) = make_patches(&[base.clone(), pb.clone()]);

        all.extend(a_map);
        all.extend(b_map);

        let result = merge(&base_ids, &a_ids, &b_ids, &all).unwrap();
        assert!(!result.is_clean);
        assert_eq!(result.conflicts.len(), 1);
        assert_eq!(result.conflicts[0].conflict_addresses, vec!["A1"]);
    }

    #[test]
    fn test_empty_branches() {
        let base = patch("Z0", "base");
        let (base_ids, all) = make_patches(&[base]);

        let result = merge(&base_ids, &[], &[], &all).unwrap();
        assert!(result.is_clean);
        assert!(result.patches_a_only.is_empty());
        assert!(result.patches_b_only.is_empty());
    }

    #[test]
    fn test_single_branch_changed() {
        let base = patch("Z0", "base");
        let pa = patch("A1", "branch_a");

        let (base_ids, mut all) = make_patches(std::slice::from_ref(&base));
        let (a_ids, a_map) = make_patches(&[base.clone(), pa.clone()]);
        all.extend(a_map);

        let result = merge(&base_ids, &a_ids, &base_ids, &all).unwrap();
        assert!(result.is_clean);
        assert!(result.patches_a_only.contains(&pa.id));
        assert!(result.patches_b_only.is_empty());
    }

    #[test]
    fn test_merge_deterministic() {
        let base = patch("Z0", "base");
        let pa1 = patch("A1", "a1");
        let pa2 = patch("A2", "a2");
        let pb1 = patch("B1", "b1");
        let pb2 = patch("B2", "b2");

        let (base_ids, mut all) = make_patches(std::slice::from_ref(&base));
        let (a_ids, a_map) = make_patches(&[base.clone(), pa1.clone(), pa2.clone()]);
        let (b_ids, b_map) = make_patches(&[base.clone(), pb1.clone(), pb2.clone()]);
        all.extend(a_map);
        all.extend(b_map);

        let r1 = merge(&base_ids, &a_ids, &b_ids, &all).unwrap();
        let r2 = merge(&base_ids, &b_ids, &a_ids, &all).unwrap();

        // Both results should have the same set of unique patches
        let mut ids1 = r1.all_patch_ids();
        let mut ids2 = r2.all_patch_ids();
        ids1.sort();
        ids2.sort();
        assert_eq!(
            ids1, ids2,
            "Merge must be deterministic regardless of order"
        );
        assert_eq!(r1.conflicts.len(), r2.conflicts.len());
    }

    #[test]
    fn test_detect_conflicts() {
        let pa = patch("A1", "a");
        let pb = patch("A1", "b"); // Same address

        let conflicts = detect_conflicts(std::slice::from_ref(&pa), std::slice::from_ref(&pb));
        assert_eq!(conflicts.len(), 1);

        let pc = patch("C1", "c"); // Different address
        let no_conflicts = detect_conflicts(&[pa], &[pc]);
        assert!(no_conflicts.is_empty());
    }

    #[test]
    fn test_partial_overlap_merge() {
        let base = patch("Z0", "base");
        let pa1 = patch("A1", "a1");
        let pa2 = patch("B1", "a2"); // Overlaps with pb2
        let pb1 = patch("C1", "b1"); // No overlap
        let pb2 = patch("B1", "b2"); // Overlaps with pa2

        let (base_ids, mut all) = make_patches(std::slice::from_ref(&base));
        let (a_ids, a_map) = make_patches(&[base.clone(), pa1.clone(), pa2.clone()]);
        let (b_ids, b_map) = make_patches(&[base.clone(), pb1.clone(), pb2.clone()]);
        all.extend(a_map);
        all.extend(b_map);

        let result = merge(&base_ids, &a_ids, &b_ids, &all).unwrap();
        assert!(!result.is_clean);
        assert_eq!(result.conflicts.len(), 1); // Only B1 conflicts
    }

    mod proptests {
        use super::*;
        use crate::patch::types::{OperationType, Patch, TouchSet};
        use proptest::prelude::*;

        proptest! {
            /// merge(base, A, B) produces the same patch IDs as merge(base, B, A).
            /// Branches are modelled as base + unique patches with disjoint addresses.
            #[test]
            fn merge_commutativity(
                n_base in 0..10usize,
                n_a in 0..20usize,
                n_b in 0..20usize,
                addrs in proptest::collection::vec("[a-z]{1,5}", 0..40),
            ) {
                let mut all_patches = Vec::new();

                for i in 0..n_base {
                    all_patches.push(Patch::new(
                        OperationType::Modify,
                        TouchSet::single(format!("base_{i}")),
                        None,
                        vec![],
                        vec![],
                        "bench".to_string(),
                        format!("base {i}"),
                    ));
                }

                let actual_n_a = n_a.min(addrs.len());
                let actual_n_b = n_b.min(addrs.len().saturating_sub(actual_n_a));

                for (i, addr) in addrs.iter().take(actual_n_a).enumerate() {
                    all_patches.push(Patch::new(
                        OperationType::Modify,
                        TouchSet::single(format!("a_{i}_{addr}")),
                        None,
                        vec![],
                        vec![],
                        "bench".to_string(),
                        format!("a_{i}"),
                    ));
                }

                for (i, addr) in addrs.iter().skip(actual_n_a).take(actual_n_b).enumerate() {
                    all_patches.push(Patch::new(
                        OperationType::Modify,
                        TouchSet::single(format!("b_{i}_{addr}")),
                        None,
                        vec![],
                        vec![],
                        "bench".to_string(),
                        format!("b_{i}"),
                    ));
                }

                let (ids, map): (Vec<PatchId>, HashMap<PatchId, Patch>) = {
                    let ids: Vec<PatchId> = all_patches.iter().map(|p| p.id).collect();
                    let map: HashMap<PatchId, Patch> = all_patches.iter().map(|p| (p.id, p.clone())).collect();
                    (ids, map)
                };

                let base_end = n_base;
                let a_end = n_base + actual_n_a;
                let b_end = a_end + actual_n_b;

                // Branch A = base ++ A-extra,  Branch B = base ++ B-extra
                let branch_a: Vec<PatchId> = ids[..base_end].iter().chain(ids[base_end..a_end].iter()).copied().collect();
                let branch_b: Vec<PatchId> = ids[..base_end].iter().chain(ids[a_end..b_end].iter()).copied().collect();

                let r_ab = merge(&ids[..base_end], &branch_a, &branch_b, &map).unwrap();
                let r_ba = merge(&ids[..base_end], &branch_b, &branch_a, &map).unwrap();

                let mut ids_ab = r_ab.all_patch_ids();
                let mut ids_ba = r_ba.all_patch_ids();
                ids_ab.sort();
                ids_ba.sort();
                prop_assert_eq!(ids_ab, ids_ba, "merge must be commutative");
                prop_assert_eq!(r_ab.conflicts.len(), r_ba.conflicts.len());
            }
        }

        proptest! {
            /// merge(A, A, A) is clean: when base, branch-a, and branch-b are
            /// all identical, there are no unique patches on either side.
            #[test]
            fn merge_idempotency(
                n_base in 0..10usize,
                addrs in proptest::collection::vec("[a-z]{1,5}", 0..20),
            ) {
                let mut all_patches = Vec::new();

                for i in 0..n_base {
                    all_patches.push(Patch::new(
                        OperationType::Modify,
                        TouchSet::single(format!("base_{i}")),
                        None,
                        vec![],
                        vec![],
                        "bench".to_string(),
                        format!("base {i}"),
                    ));
                }

                for (i, addr) in addrs.iter().enumerate() {
                    all_patches.push(Patch::new(
                        OperationType::Modify,
                        TouchSet::single(format!("a_{i}_{addr}")),
                        None,
                        vec![],
                        vec![],
                        "bench".to_string(),
                        format!("a_{i}"),
                    ));
                }

                let (ids, map): (Vec<PatchId>, HashMap<PatchId, Patch>) = {
                    let ids: Vec<PatchId> = all_patches.iter().map(|p| p.id).collect();
                    let map: HashMap<PatchId, Patch> = all_patches.iter().map(|p| (p.id, p.clone())).collect();
                    (ids, map)
                };

                // merge(A, A, A) — all three arguments identical
                let result = merge(&ids, &ids, &ids, &map).unwrap();

                prop_assert!(result.is_clean, "merge(A, A, A) must have no conflicts");
                prop_assert!(result.patches_a_only.is_empty());
                prop_assert!(result.patches_b_only.is_empty());
            }
        }

        proptest! {
            /// Conflict detection is symmetric: if A conflicts with B,
            /// then B conflicts with A with the same addresses.
            #[test]
            fn conflict_symmetry(
                overlap_addr in "[a-z]{1,5}",
                a_only_addrs in proptest::collection::vec("[a-z]{1,5}", 0..10),
                b_only_addrs in proptest::collection::vec("[a-z]{1,5}", 0..10),
            ) {
                let pa = Patch::new(
                    OperationType::Modify,
                    TouchSet::from_addrs(
                        std::iter::once(overlap_addr.as_str())
                            .chain(a_only_addrs.iter().map(|s| s.as_str()))
                    ),
                    None,
                    vec![],
                    vec![],
                    "bench".to_string(),
                    "patch_a".to_string(),
                );
                let pb = Patch::new(
                    OperationType::Modify,
                    TouchSet::from_addrs(
                        std::iter::once(overlap_addr.as_str())
                            .chain(b_only_addrs.iter().map(|s| s.as_str()))
                    ),
                    None,
                    vec![],
                    vec![],
                    "bench".to_string(),
                    "patch_b".to_string(),
                );

                let conflicts_ab = detect_conflicts(std::slice::from_ref(&pa), std::slice::from_ref(&pb));
                let conflicts_ba = detect_conflicts(std::slice::from_ref(&pb), std::slice::from_ref(&pa));

                prop_assert_eq!(conflicts_ab.len(), conflicts_ba.len());

                if !conflicts_ab.is_empty() {
                    let mut addrs_ab: Vec<String> = conflicts_ab[0].conflict_addresses.clone();
                    let mut addrs_ba: Vec<String> = conflicts_ba[0].conflict_addresses.clone();
                    addrs_ab.sort();
                    addrs_ba.sort();
                    prop_assert_eq!(addrs_ab, addrs_ba, "conflict addresses must be symmetric");
                }
            }
        }
    }
}
