//! Branch management utilities for the Patch DAG.
//!
//! Provides convenience methods for common branch operations:
//! - Creating branches from existing points
//! - Listing branch histories
//! - Computing diffs between branches

use crate::dag::graph::{DagError, PatchDag};
use crate::patch::types::PatchId;
use suture_common::BranchName;

impl PatchDag {
    /// Check if a branch exists.
    pub fn branch_exists(&self, name: &BranchName) -> bool {
        self.branches.contains_key(name.as_str())
    }

    /// Get the current HEAD branch (the branch named "main", or the first branch).
    pub fn head(&self) -> Option<(String, PatchId)> {
        self.branches
            .get("main")
            .map(|id| ("main".to_string(), *id))
            .or_else(|| {
                self.branches
                    .iter()
                    .next()
                    .map(|(name, id)| (name.clone(), *id))
            })
    }

    /// Get the number of commits ahead/behind between two branches.
    ///
    /// Returns `(ahead, behind)` where:
    /// - `ahead` is the number of patches on `branch_a` since the LCA
    /// - `behind` is the number of patches on `branch_b` since the LCA
    ///
    /// DAG-aware: uses ancestor sets rather than first-parent chain,
    /// correctly counting patches across merge commits.
    pub fn branch_divergence(
        &self,
        branch_a: &BranchName,
        branch_b: &BranchName,
    ) -> Result<(usize, usize), DagError> {
        let target_a = self
            .get_branch(branch_a)
            .ok_or_else(|| DagError::BranchNotFound(branch_a.as_str().to_string()))?;
        let target_b = self
            .get_branch(branch_b)
            .ok_or_else(|| DagError::BranchNotFound(branch_b.as_str().to_string()))?;

        // Find the LCA (the most recent common ancestor)
        let lca_id = self
            .lca(&target_a, &target_b)
            .ok_or_else(|| DagError::Custom("no common ancestor found".to_string()))?;

        // DAG-aware: unique patches = (ancestors(tip) ∪ {tip}) - (ancestors(lca) ∪ {lca})
        let lca_ancestors = self.ancestors(&lca_id);
        let mut lca_set: std::collections::HashSet<PatchId> =
            lca_ancestors.iter().copied().collect();
        lca_set.insert(lca_id);

        let ahead = self
            .ancestors(&target_a)
            .iter()
            .chain(std::iter::once(&target_a))
            .filter(|id| !lca_set.contains(id))
            .count();

        let behind = self
            .ancestors(&target_b)
            .iter()
            .chain(std::iter::once(&target_b))
            .filter(|id| !lca_set.contains(id))
            .count();

        Ok((ahead, behind))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dag::graph::PatchDag;
    use crate::patch::types::{OperationType, Patch, TouchSet};

    fn make_patch(addr: &str) -> Patch {
        Patch::new(
            OperationType::Modify,
            TouchSet::single(addr),
            None,
            vec![],
            vec![],
            "test".to_string(),
            format!("edit {}", addr),
        )
    }

    #[test]
    fn test_branch_exists() {
        let mut dag = PatchDag::new();
        let root = make_patch("root");
        let root_id = dag.add_patch(root, vec![]).unwrap();

        let main = BranchName::new("main").unwrap();
        dag.create_branch(main.clone(), root_id).unwrap();

        assert!(dag.branch_exists(&main));
        assert!(!dag.branch_exists(&BranchName::new("nonexistent").unwrap()));
    }

    #[test]
    fn test_head() {
        let mut dag = PatchDag::new();
        let root = make_patch("root");
        let root_id = dag.add_patch(root, vec![]).unwrap();

        assert!(dag.head().is_none());

        let main = BranchName::new("main").unwrap();
        dag.create_branch(main.clone(), root_id).unwrap();

        let head = dag.head().unwrap();
        assert_eq!(head.0, "main");
        assert_eq!(head.1, root_id);
    }

    #[test]
    fn test_branch_divergence() {
        let mut dag = PatchDag::new();
        let root = make_patch("root");
        let root_id = dag.add_patch(root, vec![]).unwrap();

        // Create main branch
        let main = BranchName::new("main").unwrap();
        dag.create_branch(main.clone(), root_id).unwrap();

        // Add a commit to main
        let mc = make_patch("main_commit");
        let mc_id = dag.add_patch(mc, vec![root_id]).unwrap();
        dag.update_branch(&main, mc_id).unwrap();

        // Create feature branch from root
        let feat = BranchName::new("feature").unwrap();
        dag.create_branch(feat.clone(), root_id).unwrap();

        // Add a commit to feature
        let fc = make_patch("feat_commit");
        let fc_id = dag.add_patch(fc, vec![root_id]).unwrap();
        dag.update_branch(&feat, fc_id).unwrap();

        let (ahead, behind) = dag.branch_divergence(&main, &feat).unwrap();
        // Both branches diverge from root_id as LCA.
        // Each has 1 patch between tip and LCA.
        assert_eq!(ahead, 1, "main should be 1 ahead of feature");
        assert_eq!(behind, 1, "main should be 1 behind feature");
    }
}
