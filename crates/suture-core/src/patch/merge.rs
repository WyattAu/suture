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

    // Detect conflicts between unique patches
    let mut conflicts = Vec::new();

    for patch_a_id in &patches_a_only {
        let patch_a = all_patches
            .get(patch_a_id)
            .ok_or_else(|| MergeError::PatchNotFound(patch_a_id.to_hex()))?;

        // Skip identity patches for conflict detection
        if patch_a.is_identity() {
            continue;
        }

        for patch_b_id in &patches_b_only {
            let patch_b = all_patches
                .get(patch_b_id)
                .ok_or_else(|| MergeError::PatchNotFound(patch_b_id.to_hex()))?;

            if patch_b.is_identity() {
                continue;
            }

            match commute(patch_a, patch_b) {
                crate::patch::CommuteResult::DoesNotCommute { conflict_addresses } => {
                    conflicts.push(Conflict::new(*patch_a_id, *patch_b_id, conflict_addresses));
                }
                crate::patch::CommuteResult::Commutes => {
                    // No conflict, both can be included
                }
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

    for patch_a in patches_a {
        if patch_a.is_identity() {
            continue;
        }
        for patch_b in patches_b {
            if patch_b.is_identity() {
                continue;
            }
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
}
