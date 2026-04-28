//! # Patch DAG
//!
//! An in-memory directed acyclic graph (DAG) of patches, serving as the
//! core history data structure for a Suture repository.
//!
//! The DAG stores [`Patch`] nodes connected by parent-child edges.
//! Each branch name maps to a tip patch. Ancestor queries and
//! Lowest Common Ancestor (LCA) computation are supported for
//! merge-base detection and rebasing.
//!
//! Supports:
//! - Adding patches with parent edges
//! - Ancestor queries
//! - Lowest Common Ancestor (LCA) computation
//! - Acyclicity enforcement
//! - Branch creation, deletion, and lookup

use crate::patch::types::{Patch, PatchId};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::rc::Rc;
use suture_common::BranchName;
use thiserror::Error;

/// Errors that can occur during DAG operations.
#[derive(Error, Debug)]
pub enum DagError {
    #[error("patch already exists: {0}")]
    DuplicatePatch(String),

    #[error("patch not found: {0}")]
    PatchNotFound(String),

    #[error("parent patch not found: {0}")]
    ParentNotFound(String),

    #[error("would create a cycle: {from} -> {to}")]
    CycleDetected { from: String, to: String },

    #[error("branch not found: {0}")]
    BranchNotFound(String),

    #[error("branch already exists: {0}")]
    BranchAlreadyExists(String),

    #[error("empty branch name")]
    EmptyBranchName,

    #[error("invalid branch name: {0}")]
    InvalidBranchName(String),

    #[error("cannot create root patch with parents")]
    RootWithParents,

    #[error("no common ancestor found")]
    NoCommonAncestor,

    #[error("DAG merge error: {0}")]
    MergeFailed(String),


}

/// A node in the Patch-DAG.
#[derive(Clone, Debug)]
pub struct DagNode {
    /// The patch data.
    pub(crate) patch: Patch,
    /// Parent patch IDs.
    pub(crate) parent_ids: Vec<PatchId>,
    /// Child patch IDs.
    pub(crate) child_ids: Vec<PatchId>,
    /// Generation number: max(parent generations) + 1, or 0 for root.
    /// Used for O(1) depth comparisons in LCA computation.
    pub(crate) generation: u64,
}

impl DagNode {
    #[inline]
    pub fn id(&self) -> &PatchId {
        &self.patch.id
    }
}

/// The Patch-DAG — a directed acyclic graph of patches.
///
/// Internally stores:
/// - A HashMap of PatchId -> DagNode
/// - A HashMap of branch name -> target PatchId
/// - An ancestor cache (lazy, stable across mutations since adding new nodes
///   never changes existing nodes' ancestor sets)
pub struct PatchDag {
    /// All nodes in the DAG, indexed by patch ID.
    pub(crate) nodes: HashMap<PatchId, DagNode>,
    /// Named branch pointers.
    pub(crate) branches: HashMap<String, PatchId>,
    /// Cache of ancestor sets, keyed by patch ID.
    /// Populated lazily by `ancestors()`; stable because `add_patch()` only
    /// creates new nodes (existing nodes' ancestor sets never change).
    ancestor_cache: RefCell<HashMap<PatchId, Rc<HashSet<PatchId>>>>,
}

impl Default for PatchDag {
    fn default() -> Self {
        Self::new()
    }
}

impl PatchDag {
    /// Create a new empty DAG.
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            branches: HashMap::new(),
            ancestor_cache: RefCell::new(HashMap::new()),
        }
    }

    /// Add a patch to the DAG.
    ///
    /// # Arguments
    ///
    /// * `patch` - The patch to add
    /// * `parent_ids` - Parent patch IDs (empty for root commit)
    ///
    /// # Errors
    ///
    /// - `DuplicatePatch` if a patch with the same ID already exists
    /// - `ParentNotFound` if a parent doesn't exist (unless it's a root commit)
    /// - `CycleDetected` if adding this edge would create a cycle
    pub fn add_patch(
        &mut self,
        patch: Patch,
        parent_ids: Vec<PatchId>,
    ) -> Result<PatchId, DagError> {
        let id = patch.id;

        // Check for duplicates
        if self.nodes.contains_key(&id) {
            return Err(DagError::DuplicatePatch(id.to_hex()));
        }

        // Validate parents exist (unless this is a root commit)
        if !parent_ids.is_empty() {
            for parent_id in &parent_ids {
                if !self.nodes.contains_key(parent_id) {
                    return Err(DagError::ParentNotFound(parent_id.to_hex()));
                }
            }

            // Check for cycles: ensure this patch is not an ancestor of any parent
            // (Since this patch is new, it can't be an ancestor yet, so this check
            // is trivially satisfied for new patches. The cycle check is more relevant
            // for branch operations.)
        }

        // Compute generation number: max(parent generations) + 1, or 0 for root
        let generation = if parent_ids.is_empty() {
            0
        } else {
            parent_ids
                .iter()
                .map(|pid| self.nodes.get(pid).map(|n| n.generation).unwrap_or(0))
                .max()
                .unwrap_or(0)
                + 1
        };

        // Create the node
        let node = DagNode {
            patch,
            parent_ids: parent_ids.clone(),
            child_ids: Vec::new(),
            generation,
        };

        // Add edges from parents to this node
        for parent_id in &parent_ids {
            if let Some(parent_node) = self.nodes.get_mut(parent_id) {
                parent_node.child_ids.push(id);
            }
        }

        self.nodes.insert(id, node);
        Ok(id)
    }

    /// Get a patch by ID.
    pub fn get_patch(&self, id: &PatchId) -> Option<&Patch> {
        self.nodes.get(id).map(|node| &node.patch)
    }

    /// Get a node by ID.
    pub fn get_node(&self, id: &PatchId) -> Option<&DagNode> {
        self.nodes.get(id)
    }

    /// Check if a patch exists.
    pub fn has_patch(&self, id: &PatchId) -> bool {
        self.nodes.contains_key(id)
    }

    /// Get all transitive ancestors of a patch (excluding the patch itself).
    ///
    /// Uses BFS traversal. Results are cached: the first call for a given patch
    /// ID computes the ancestor set via BFS; subsequent calls return the cached
    /// result in O(1). The cache is safe without invalidation because
    /// `add_patch()` only creates new nodes — existing nodes' ancestor sets
    /// never change.
    pub fn ancestors(&self, id: &PatchId) -> Rc<HashSet<PatchId>> {
        if let Some(cached) = self.ancestor_cache.borrow().get(id) {
            return Rc::clone(cached);
        }

        let mut ancestors = HashSet::new();
        let mut queue: VecDeque<PatchId> = VecDeque::new();

        if let Some(node) = self.nodes.get(id) {
            for parent_id in &node.parent_ids {
                if !ancestors.contains(parent_id) {
                    ancestors.insert(*parent_id);
                    queue.push_back(*parent_id);
                }
            }
        }

        while let Some(current) = queue.pop_front() {
            if let Some(node) = self.nodes.get(&current) {
                for parent_id in &node.parent_ids {
                    if ancestors.insert(*parent_id) {
                        queue.push_back(*parent_id);
                    }
                }
            }
        }

        let result = Rc::new(ancestors);
        self.ancestor_cache
            .borrow_mut()
            .insert(*id, Rc::clone(&result));
        result
    }

    /// Find the Lowest Common Ancestor (LCA) of two patches.
    ///
    /// The LCA is the most recent patch that is an ancestor of both.
    /// Returns `None` if no common ancestor exists.
    ///
    /// Uses generation numbers for O(1) depth comparison instead of BFS-based
    /// `ancestor_depth()`, reducing complexity from O(n²) to O(n).
    pub fn lca(&self, a: &PatchId, b: &PatchId) -> Option<PatchId> {
        if a == b {
            return Some(*a);
        }

        let ancestors_a = self.ancestors(a);
        let ancestors_b = self.ancestors(b);

        // Quick check: if a is an ancestor of b, then a is the LCA
        if ancestors_b.contains(a) {
            return Some(*a);
        }
        // If b is an ancestor of a, then b is the LCA
        if ancestors_a.contains(b) {
            return Some(*b);
        }

        // Find common ancestors
        let common: Vec<PatchId> = ancestors_a.intersection(&ancestors_b).copied().collect();

        if common.is_empty() {
            return None;
        }

        // Find the most recent common ancestor (highest generation number).
        // Uses precomputed generation field — O(1) per lookup instead of BFS.
        let mut best: Option<PatchId> = None;
        let mut best_gen: u64 = 0;

        for candidate in &common {
            let candidate_gen = self.nodes.get(candidate).map(|n| n.generation).unwrap_or(0);
            if candidate_gen > best_gen
                || (candidate_gen == best_gen
                    && (best.is_none() || candidate < best.as_ref().unwrap()))
            {
                best_gen = candidate_gen;
                best = Some(*candidate);
            }
        }

        best
    }

    /// Compute the "depth" of a patch using its precomputed generation number.
    ///
    /// For a linear chain, this equals the number of ancestors.
    /// For DAGs with merges, the generation is the length of the longest
    /// path from root to this node.
    #[allow(dead_code)]
    fn ancestor_depth(&self, id: &PatchId) -> usize {
        self.nodes
            .get(id)
            .map(|n| n.generation as usize)
            .unwrap_or(0)
    }

    /// Create a new branch pointing to a patch.
    pub fn create_branch(&mut self, name: BranchName, target: PatchId) -> Result<(), DagError> {
        let name_str = name.as_str().to_string();

        if name_str.is_empty() {
            return Err(DagError::EmptyBranchName);
        }

        if self.branches.contains_key(&name_str) {
            return Err(DagError::BranchAlreadyExists(name_str));
        }

        if !self.nodes.contains_key(&target) {
            return Err(DagError::PatchNotFound(target.to_hex()));
        }

        self.branches.insert(name_str, target);
        Ok(())
    }

    /// Get the target patch ID of a branch.
    pub fn get_branch(&self, name: &BranchName) -> Option<PatchId> {
        self.branches.get(name.as_str()).copied()
    }

    /// Update a branch to point to a new patch.
    pub fn update_branch(&mut self, name: &BranchName, target: PatchId) -> Result<(), DagError> {
        if !self.branches.contains_key(name.as_str()) {
            return Err(DagError::BranchNotFound(name.as_str().to_string()));
        }
        if !self.nodes.contains_key(&target) {
            return Err(DagError::PatchNotFound(target.to_hex()));
        }
        self.branches.insert(name.as_str().to_string(), target);
        Ok(())
    }

    /// Delete a branch.
    pub fn delete_branch(&mut self, name: &BranchName) -> Result<(), DagError> {
        if self.branches.remove(name.as_str()).is_none() {
            return Err(DagError::BranchNotFound(name.as_str().to_string()));
        }
        Ok(())
    }

    /// List all branches.
    pub fn list_branches(&self) -> Vec<(String, PatchId)> {
        let mut branches: Vec<_> = self
            .branches
            .iter()
            .map(|(name, id)| (name.clone(), *id))
            .collect();
        branches.sort_by_key(|a| a.0.clone());
        branches
    }

    #[inline]
    pub fn patch_count(&self) -> usize {
        self.nodes.len()
    }

    /// Get all patch IDs in the DAG.
    pub fn patch_ids(&self) -> Vec<PatchId> {
        self.nodes.keys().copied().collect()
    }

    /// Get patches from a specific node back to root (inclusive), following
    /// **all** parent edges (not just the first). This is required for
    /// computing snapshots of merge commits, where both parent lineages
    /// contribute to the final file tree.
    ///
    /// Returns patches in oldest-first (root → tip) order suitable for
    /// sequential replay via `apply_patch_chain`.
    pub fn patch_chain_full(&self, id: &PatchId) -> Vec<PatchId> {
        // Collect self + all ancestors
        let all: HashSet<PatchId> = {
            let anc = self.ancestors(id);
            let mut set = HashSet::with_capacity(anc.len() + 1);
            set.insert(*id);
            for a in anc.iter() {
                set.insert(*a);
            }
            set
        };

        // Topological sort by generation number (oldest first).
        // Generation is assigned in add_patch as max(parent generations) + 1,
        // so parents always have strictly lower generation than children.
        let mut sorted: Vec<PatchId> = all.iter().copied().collect();
        sorted.sort_by_key(|pid| self.nodes.get(pid).map(|n| n.generation).unwrap_or(0));

        sorted
    }

    /// Get patches from a specific node back to root (inclusive).
    /// Follows **only the first parent** — use `patch_chain_full` for
    /// DAG-aware traversal that includes merge parents.
    pub fn patch_chain(&self, id: &PatchId) -> Vec<PatchId> {
        let mut chain = Vec::new();
        let mut seen = HashSet::new();
        let mut current = Some(*id);

        while let Some(curr_id) = current {
            if !seen.insert(curr_id) {
                break;
            }
            chain.push(curr_id);
            current = self
                .nodes
                .get(&curr_id)
                .and_then(|n| n.parent_ids.first().copied());
        }

        chain
    }

    /// Collect all patches reachable from a given patch ID (inclusive).
    pub fn reachable_patches(&self, id: &PatchId) -> Vec<Patch> {
        let ancestors = self.ancestors(id);
        let mut patches = Vec::with_capacity(ancestors.len() + 1);

        if let Some(node) = self.nodes.get(id) {
            patches.push(node.patch.clone());
        }

        for ancestor_id in ancestors.iter() {
            if let Some(node) = self.nodes.get(ancestor_id) {
                patches.push(node.patch.clone());
            }
        }

        patches
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patch::types::{OperationType, Patch, TouchSet};
    use suture_common::Hash;

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
    fn test_add_root_patch() {
        let mut dag = PatchDag::new();
        let root = make_patch("root");
        let id = dag.add_patch(root, vec![]).unwrap();
        assert_eq!(dag.patch_count(), 1);
        assert!(dag.has_patch(&id));
    }

    #[test]
    fn test_add_patch_with_parent() {
        let mut dag = PatchDag::new();
        let root = make_patch("root");
        let root_id = dag.add_patch(root, vec![]).unwrap();

        let child = make_patch("child");
        let child_id = dag.add_patch(child, vec![root_id]).unwrap();
        assert_eq!(dag.patch_count(), 2);

        let ancestors = dag.ancestors(&child_id);
        assert_eq!(ancestors.len(), 1);
        assert!(ancestors.contains(&root_id));
    }

    #[test]
    fn test_duplicate_patch_rejected() {
        let mut dag = PatchDag::new();
        let p = make_patch("dup");
        let _id = dag.add_patch(p.clone(), vec![]).unwrap();
        let result = dag.add_patch(p, vec![]);
        assert!(matches!(result, Err(DagError::DuplicatePatch(_))));
    }

    #[test]
    fn test_parent_not_found() {
        let mut dag = PatchDag::new();
        let child = make_patch("child");
        let fake_parent = Hash::from_hex(&"f".repeat(64)).unwrap();
        let result = dag.add_patch(child, vec![fake_parent]);
        assert!(matches!(result, Err(DagError::ParentNotFound(_))));
    }

    #[test]
    fn test_ancestors_linear_chain() {
        let mut dag = PatchDag::new();
        let p0 = make_patch("p0");
        let id0 = dag.add_patch(p0, vec![]).unwrap();

        let p1 = make_patch("p1");
        let id1 = dag.add_patch(p1, vec![id0]).unwrap();

        let p2 = make_patch("p2");
        let id2 = dag.add_patch(p2, vec![id1]).unwrap();

        let ancestors = dag.ancestors(&id2);
        assert_eq!(ancestors.len(), 2);
        assert!(ancestors.contains(&id0));
        assert!(ancestors.contains(&id1));
    }

    #[test]
    fn test_ancestors_diamond() {
        let mut dag = PatchDag::new();
        let root = make_patch("root");
        let root_id = dag.add_patch(root, vec![]).unwrap();

        let left = make_patch("left");
        let left_id = dag.add_patch(left, vec![root_id]).unwrap();

        let right = make_patch("right");
        let right_id = dag.add_patch(right, vec![root_id]).unwrap();

        let merge = make_patch("merge");
        let merge_id = dag.add_patch(merge, vec![left_id, right_id]).unwrap();

        let ancestors = dag.ancestors(&merge_id);
        assert_eq!(ancestors.len(), 3); // root, left, right
    }

    #[test]
    fn test_lca_linear() {
        let mut dag = PatchDag::new();
        let p0 = make_patch("p0");
        let id0 = dag.add_patch(p0, vec![]).unwrap();

        let p1 = make_patch("p1");
        let id1 = dag.add_patch(p1, vec![id0]).unwrap();

        let p2 = make_patch("p2");
        let id2 = dag.add_patch(p2, vec![id1]).unwrap();

        assert_eq!(dag.lca(&id1, &id2), Some(id1));
        assert_eq!(dag.lca(&id0, &id2), Some(id0));
    }

    #[test]
    fn test_hashset_contains() {
        use std::collections::HashSet;
        let h1 = suture_common::Hash::from_data(b"test1");
        let h2 = suture_common::Hash::from_data(b"test2");
        let mut set: HashSet<suture_common::Hash> = HashSet::new();
        set.insert(h1);
        set.insert(h2);
        assert!(set.contains(&h1));
        assert!(set.contains(&h2));
        let h3 = suture_common::Hash::from_data(b"test1");
        assert!(set.contains(&h3), "same-value hash should be in set");
    }

    #[test]
    fn test_ancestors_with_hashset() {
        let mut dag = PatchDag::new();
        let root = make_patch("root");
        let root_id = dag.add_patch(root, vec![]).unwrap();
        let child = make_patch("child");
        let child_id = dag.add_patch(child, vec![root_id]).unwrap();

        let ancestors = dag.ancestors(&child_id);
        assert_eq!(ancestors.len(), 1, "child should have 1 ancestor");
        assert!(
            ancestors.contains(&root_id),
            "root should be ancestor of child"
        );
    }

    #[test]
    fn test_lca_diamond() {
        let mut dag = PatchDag::new();
        let root = make_patch("root");
        let root_id = dag.add_patch(root, vec![]).unwrap();

        let left = make_patch("left");
        let left_id = dag.add_patch(left, vec![root_id]).unwrap();

        let right = make_patch("right");
        let right_id = dag.add_patch(right, vec![root_id]).unwrap();

        let merge = make_patch("merge");
        let merge_id = dag.add_patch(merge, vec![left_id, right_id]).unwrap();

        // Debug: verify ancestors work
        let anc_left = dag.ancestors(&left_id);
        let anc_right = dag.ancestors(&right_id);
        let anc_merge = dag.ancestors(&merge_id);
        assert!(
            anc_left.contains(&root_id),
            "root_id should be ancestor of left_id"
        );
        assert!(
            anc_right.contains(&root_id),
            "root_id should be ancestor of right_id"
        );
        assert!(
            anc_merge.contains(&left_id),
            "left_id should be ancestor of merge_id"
        );
        assert!(
            anc_merge.contains(&root_id),
            "root_id should be ancestor of merge_id"
        );
        assert_eq!(
            anc_left.len(),
            1,
            "left_id should have exactly 1 ancestor (root_id)"
        );
        assert_eq!(
            anc_merge.len(),
            3,
            "merge_id should have 3 ancestors (left, right, root)"
        );

        let lca_result = dag.lca(&merge_id, &left_id);
        assert_eq!(
            lca_result,
            Some(left_id),
            "LCA of merge and left should be left"
        );
    }

    #[test]
    fn test_branch_operations() {
        let mut dag = PatchDag::new();
        let root = make_patch("root");
        let root_id = dag.add_patch(root, vec![]).unwrap();

        let main = BranchName::new("main").unwrap();
        dag.create_branch(main.clone(), root_id).unwrap();

        assert_eq!(dag.get_branch(&main), Some(root_id));
        assert_eq!(dag.list_branches().len(), 1);

        let child = make_patch("child");
        let child_id = dag.add_patch(child, vec![root_id]).unwrap();
        dag.update_branch(&main, child_id).unwrap();
        assert_eq!(dag.get_branch(&main), Some(child_id));

        let feat = BranchName::new("feature").unwrap();
        dag.create_branch(feat.clone(), root_id).unwrap();
        assert_eq!(dag.list_branches().len(), 2);

        dag.delete_branch(&feat).unwrap();
        assert_eq!(dag.list_branches().len(), 1);
    }

    #[test]
    fn test_branch_duplicate_rejected() {
        let mut dag = PatchDag::new();
        let root = make_patch("root");
        let root_id = dag.add_patch(root, vec![]).unwrap();

        let main = BranchName::new("main").unwrap();
        dag.create_branch(main.clone(), root_id).unwrap();
        let result = dag.create_branch(main, root_id);
        assert!(matches!(result, Err(DagError::BranchAlreadyExists(_))));
    }

    #[test]
    fn test_patch_chain() {
        let mut dag = PatchDag::new();
        let p0 = make_patch("p0");
        let id0 = dag.add_patch(p0, vec![]).unwrap();

        let p1 = make_patch("p1");
        let id1 = dag.add_patch(p1, vec![id0]).unwrap();

        let p2 = make_patch("p2");
        let id2 = dag.add_patch(p2, vec![id1]).unwrap();

        let chain = dag.patch_chain(&id2);
        assert_eq!(chain.len(), 3);
        assert_eq!(chain[0], id2); // Most recent first
        assert_eq!(chain[1], id1);
        assert_eq!(chain[2], id0);
    }

    #[test]
    fn test_generation_numbers_linear() {
        let mut dag = PatchDag::new();
        let p0 = make_patch("p0");
        let id0 = dag.add_patch(p0, vec![]).unwrap();
        assert_eq!(dag.get_node(&id0).unwrap().generation, 0);

        let p1 = make_patch("p1");
        let id1 = dag.add_patch(p1, vec![id0]).unwrap();
        assert_eq!(dag.get_node(&id1).unwrap().generation, 1);

        let p2 = make_patch("p2");
        let id2 = dag.add_patch(p2, vec![id1]).unwrap();
        assert_eq!(dag.get_node(&id2).unwrap().generation, 2);
    }

    #[test]
    fn test_generation_numbers_diamond() {
        let mut dag = PatchDag::new();
        let root = make_patch("root");
        let root_id = dag.add_patch(root, vec![]).unwrap();

        let left = make_patch("left");
        let left_id = dag.add_patch(left, vec![root_id]).unwrap();

        let right = make_patch("right");
        let right_id = dag.add_patch(right, vec![root_id]).unwrap();

        let merge = make_patch("merge");
        let merge_id = dag.add_patch(merge, vec![left_id, right_id]).unwrap();

        assert_eq!(dag.get_node(&root_id).unwrap().generation, 0);
        assert_eq!(dag.get_node(&left_id).unwrap().generation, 1);
        assert_eq!(dag.get_node(&right_id).unwrap().generation, 1);
        // Merge's generation = max(left.gen, right.gen) + 1 = 2
        assert_eq!(dag.get_node(&merge_id).unwrap().generation, 2);
    }

    #[test]
    fn test_generation_numbers_uneven_branches() {
        let mut dag = PatchDag::new();
        let root = make_patch("root");
        let root_id = dag.add_patch(root, vec![]).unwrap();

        // Short branch: root -> a
        let a = make_patch("a");
        let a_id = dag.add_patch(a, vec![root_id]).unwrap();

        // Long branch: root -> b -> c -> d
        let b = make_patch("b");
        let b_id = dag.add_patch(b, vec![root_id]).unwrap();
        let c = make_patch("c");
        let c_id = dag.add_patch(c, vec![b_id]).unwrap();
        let d = make_patch("d");
        let d_id = dag.add_patch(d, vec![c_id]).unwrap();

        // Merge short and long branches
        let merge = make_patch("merge");
        let merge_id = dag.add_patch(merge, vec![a_id, d_id]).unwrap();

        assert_eq!(dag.get_node(&a_id).unwrap().generation, 1);
        assert_eq!(dag.get_node(&d_id).unwrap().generation, 3);
        // Merge gen = max(1, 3) + 1 = 4
        assert_eq!(dag.get_node(&merge_id).unwrap().generation, 4);
    }

    #[test]
    fn test_ancestor_cache() {
        let mut dag = PatchDag::new();
        let p0 = make_patch("p0");
        let id0 = dag.add_patch(p0, vec![]).unwrap();
        let p1 = make_patch("p1");
        let id1 = dag.add_patch(p1, vec![id0]).unwrap();
        let p2 = make_patch("p2");
        let id2 = dag.add_patch(p2, vec![id1]).unwrap();

        // First call: computes via BFS, caches result
        let anc1 = dag.ancestors(&id2);
        assert_eq!(anc1.len(), 2);

        // Second call: should return cached result (same values)
        let anc2 = dag.ancestors(&id2);
        assert_eq!(anc2.len(), 2);
        assert_eq!(anc1, anc2);

        // Cache should have entries for id2
        assert!(dag.ancestor_cache.borrow().contains_key(&id2));
    }

    #[test]
    fn test_lca_uneven_branches() {
        let mut dag = PatchDag::new();
        let root = make_patch("root");
        let root_id = dag.add_patch(root, vec![]).unwrap();

        // Branch A: root -> a1 -> a2
        let a1 = make_patch("a1");
        let a1_id = dag.add_patch(a1, vec![root_id]).unwrap();
        let a2 = make_patch("a2");
        let a2_id = dag.add_patch(a2, vec![a1_id]).unwrap();

        // Branch B: root -> b1
        let b1 = make_patch("b1");
        let b1_id = dag.add_patch(b1, vec![root_id]).unwrap();

        // LCA(a2, b1) should be root
        assert_eq!(dag.lca(&a2_id, &b1_id), Some(root_id));
        // LCA(a1, b1) should be root
        assert_eq!(dag.lca(&a1_id, &b1_id), Some(root_id));
    }

    #[test]
    fn test_lca_no_common_ancestor() {
        let mut dag = PatchDag::new();
        // Two disconnected trees
        let r1 = make_patch("root1");
        let r1_id = dag.add_patch(r1, vec![]).unwrap();
        let r2 = make_patch("root2");
        let r2_id = dag.add_patch(r2, vec![]).unwrap();

        // No common ancestor
        assert_eq!(dag.lca(&r1_id, &r2_id), None);
    }

    mod proptests {
        use super::*;
        use crate::patch::types::{OperationType, Patch, TouchSet};
        use proptest::prelude::*;

        fn make_unique_patch(idx: usize) -> Patch {
            let addr = format!("proptest_addr_{}", idx);
            Patch::new(
                OperationType::Modify,
                TouchSet::single(&addr),
                Some(format!("file_{}", addr)),
                addr.clone().into_bytes(),
                vec![],
                "proptest".to_string(),
                format!("patch {}", idx),
            )
        }

        proptest! {
            #[test]
            fn add_patch_increases_count(n in 0usize..20) {
                let mut dag = PatchDag::new();
                let mut last_id = None;
                for i in 0..n {
                    let patch = make_unique_patch(i);
                    let parents = last_id.map(|id| vec![id]).unwrap_or_default();
                    let id = dag.add_patch(patch, parents).unwrap();
                    last_id = Some(id);
                }
                prop_assert_eq!(dag.patch_count(), n);
            }

            #[test]
            fn patch_chain_ancestry(n in 0usize..20) {
                prop_assume!(n > 0);
                let mut dag = PatchDag::new();
                let mut last_id = None;
                for i in 0..n {
                    let patch = make_unique_patch(i);
                    let parents = last_id.map(|id| vec![id]).unwrap_or_default();
                    let id = dag.add_patch(patch, parents).unwrap();
                    last_id = Some(id);
                }
                let tip = last_id.unwrap();
                let chain = dag.patch_chain(&tip);
                prop_assert_eq!(chain.len(), n);
            }

            #[test]
            fn lca_linear_chain(n in 2usize..15) {
                let mut dag = PatchDag::new();
                let mut ids = Vec::new();
                for i in 0..n {
                    let patch = make_unique_patch(i);
                    let parents = ids.last().map(|id| vec![*id]).unwrap_or_default();
                    let id = dag.add_patch(patch, parents).unwrap();
                    ids.push(id);
                }
                // LCA(first, last) == first
                prop_assert_eq!(dag.lca(&ids[0], &ids[n - 1]), Some(ids[0]));
                // LCA(second, last) == second
                if n >= 3 {
                    prop_assert_eq!(dag.lca(&ids[1], &ids[n - 1]), Some(ids[1]));
                }
                // LCA(last, last) == last
                prop_assert_eq!(dag.lca(&ids[n - 1], &ids[n - 1]), Some(ids[n - 1]));
            }

            #[test]
            fn ancestors_subset(n in 1usize..20) {
                let mut dag = PatchDag::new();
                let mut ids = Vec::new();
                for i in 0..n {
                    let patch = make_unique_patch(i);
                    let parents = ids.last().map(|id| vec![*id]).unwrap_or_default();
                    let id = dag.add_patch(patch, parents).unwrap();
                    ids.push(id);
                }
                let tip = ids.last().unwrap();
                let ancestors = dag.ancestors(tip);
                // All predecessors should be in ancestors
                for (i, id) in ids.iter().enumerate().take(n - 1) {
                    prop_assert!(ancestors.contains(id),
                        "predecessor {} should be ancestor of tip", i);
                }
                // Tip itself should NOT be in ancestors
                prop_assert!(!ancestors.contains(tip));
                // Should have exactly n-1 ancestors
                prop_assert_eq!(ancestors.len(), n - 1);
            }
        }
    }
}
