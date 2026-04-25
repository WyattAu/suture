//! DAG-aware merge operations.
//!
//! Extends the patch-level merge algorithm with DAG-aware operations:
//! - Finding common ancestors between branches
//! - Computing merge bases
//! - Generating merge plans

use crate::dag::graph::{DagError, PatchDag};
use crate::patch::merge::{self, MergeResult};
use crate::patch::types::PatchId;
use std::collections::HashMap;
use suture_common::BranchName;

impl PatchDag {
    /// Compute a merge plan for two branches.
    ///
    /// Returns a `MergeResult` containing the patches to merge and any conflicts.
    /// This does NOT modify the DAG — it only computes the plan.
    ///
    /// Uses DAG-aware ancestry: patches unique to each branch are computed
    /// as `ancestors(tip) - ancestors(lca)`, which correctly handles merge
    /// commits with multiple parents.
    pub fn merge_branches(
        &self,
        branch_a: &BranchName,
        branch_b: &BranchName,
    ) -> Result<MergeResult, DagError> {
        let target_a = self
            .get_branch(branch_a)
            .ok_or_else(|| DagError::BranchNotFound(branch_a.as_str().to_string()))?;
        let target_b = self
            .get_branch(branch_b)
            .ok_or_else(|| DagError::BranchNotFound(branch_b.as_str().to_string()))?;

        // Find LCA
        let lca_id = self
            .lca(&target_a, &target_b)
            .ok_or_else(|| DagError::Custom("no common ancestor found".to_string()))?;

        // DAG-aware: compute patches unique to each branch as
        // (ancestors(tip) ∪ {tip}) - (ancestors(lca) ∪ {lca}).
        // ancestors() excludes the node itself, so we add the tip/lca
        // explicitly to get the complete set.
        let lca_ancestors = self.ancestors(&lca_id);
        let mut lca_set: std::collections::HashSet<PatchId> =
            lca_ancestors.iter().copied().collect();
        lca_set.insert(lca_id);

        let tip_a_ancestors = self.ancestors(&target_a);
        let branch_a_patches: Vec<PatchId> = tip_a_ancestors
            .iter()
            .chain(std::iter::once(&target_a))
            .filter(|id| !lca_set.contains(id))
            .copied()
            .collect();

        let tip_b_ancestors = self.ancestors(&target_b);
        let branch_b_patches: Vec<PatchId> = tip_b_ancestors
            .iter()
            .chain(std::iter::once(&target_b))
            .filter(|id| !lca_set.contains(id))
            .copied()
            .collect();

        // Build patch lookup
        let all_patches: HashMap<PatchId, _> = self
            .nodes
            .iter()
            .map(|(id, node)| (*id, node.patch.clone()))
            .collect();

        // Use the patch-level merge algorithm
        merge::merge(
            &[lca_id],
            &branch_a_patches,
            &branch_b_patches,
            &all_patches,
        )
        .map_err(|e| DagError::Custom(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use crate::dag::graph::PatchDag;
    use crate::patch::types::{OperationType, Patch, TouchSet};
    use suture_common::BranchName;

    fn make_patch(addr: &str) -> Patch {
        Patch::new(
            OperationType::Modify,
            TouchSet::single(addr),
            Some(format!("file_{}", addr)),
            vec![],
            vec![],
            "test".to_string(),
            format!("edit {}", addr),
        )
    }

    #[test]
    fn test_merge_clean_divergent_branches() {
        let mut dag = PatchDag::new();

        // Root commit
        let root = make_patch("root");
        let root_id = dag.add_patch(root, vec![]).unwrap();

        // Verify DAG structure
        assert!(dag.has_patch(&root_id), "root should be in DAG");

        // Branch A: edit A1
        let a1 = make_patch("A1");
        let a1_id = dag.add_patch(a1, vec![root_id]).unwrap();
        assert!(dag.has_patch(&a1_id), "a1 should be in DAG");

        // Branch B: edit B1
        let b1 = make_patch("B1");
        let b1_id = dag.add_patch(b1, vec![root_id]).unwrap();
        assert!(dag.has_patch(&b1_id), "b1 should be in DAG");

        // Verify ancestors
        let anc_a = dag.ancestors(&a1_id);
        let anc_b = dag.ancestors(&b1_id);
        assert!(anc_a.contains(&root_id), "root should be ancestor of a1");
        assert!(anc_b.contains(&root_id), "root should be ancestor of b1");

        // Verify LCA via ancestors intersection
        let intersection: std::collections::HashSet<_> = anc_a.intersection(&anc_b).collect();
        assert_eq!(intersection.len(), 1, "should have 1 common ancestor");
        assert!(
            intersection.contains(&root_id),
            "common ancestor should be root_id"
        );

        // Direct LCA test
        let direct_lca = dag.lca(&a1_id, &b1_id);
        assert_eq!(direct_lca, Some(root_id), "LCA of a1 and b1 should be root");

        let branch_a = BranchName::new("branch_a").unwrap();
        let branch_b = BranchName::new("branch_b").unwrap();

        dag.create_branch(branch_a.clone(), a1_id).unwrap();
        dag.create_branch(branch_b.clone(), b1_id).unwrap();

        let result = dag.merge_branches(&branch_a, &branch_b).unwrap();
        assert!(result.is_clean);
        assert!(result.conflicts.is_empty());
    }

    #[test]
    fn test_merge_conflicting_branches() {
        let mut dag = PatchDag::new();

        let root = make_patch("root");
        let root_id = dag.add_patch(root, vec![]).unwrap();

        // Both branches edit the SAME address "shared_addr" but with different payloads
        // so they get different patch IDs but still conflict on the touch set.
        let a1 = Patch::new(
            OperationType::Modify,
            TouchSet::from_addrs(["shared_addr"]),
            Some("file_shared_addr".to_string()),
            vec![1], // payload A
            vec![],
            "alice".to_string(),
            "edit shared_addr (version A)".to_string(),
        );
        let a1_id = dag.add_patch(a1, vec![root_id]).unwrap();

        let b1 = Patch::new(
            OperationType::Modify,
            TouchSet::from_addrs(["shared_addr"]),
            Some("file_shared_addr".to_string()),
            vec![2], // payload B (different!)
            vec![],
            "bob".to_string(),
            "edit shared_addr (version B)".to_string(),
        );
        let b1_id = dag.add_patch(b1, vec![root_id]).unwrap();

        let branch_a = BranchName::new("branch_a").unwrap();
        let branch_b = BranchName::new("branch_b").unwrap();

        dag.create_branch(branch_a.clone(), a1_id).unwrap();
        dag.create_branch(branch_b.clone(), b1_id).unwrap();

        let result = dag.merge_branches(&branch_a, &branch_b).unwrap();
        assert!(
            !result.is_clean,
            "branches editing the same address should conflict"
        );
        assert_eq!(result.conflicts.len(), 1, "should have exactly 1 conflict");
    }
}
