---
document_id: BP-PATCH-DAG-001
version: 1.0.0
status: APPROVED
ieee_1016_compliant: true
component_id: COMP-DAG-001
component_type: Module
interfaces: [IF-DAG-001]
depends_on:
  yellow_papers: [YP-ALGEBRA-PATCH-001]
  blue_papers: [BP-PATCH-ALGEBRA-001]
created: 2026-03-27
---

# BP-PATCH-DAG-001: Patch DAG Data Structure

## BP-1: Design Overview

The Patch DAG is the authoritative version history for a Suture repository. It is a directed
acyclic graph (DAG) where each node is a patch (from BP-PATCH-ALGEBRA-001) and each edge
represents the "applied-after" relation. Branches are named, movable pointers to specific
DAG nodes.

The DAG supports the following operations:
- Adding patches (linear or merge commits with multiple parents).
- Creating, listing, renaming, and deleting named branches.
- Computing the Lowest Common Ancestor (LCA) for merge base identification.
- Computing the transitive ancestor set for any node.
- Generating merge plans that identify conflicting patches.

The DAG topology is persisted in the SQLite metadata database (BP-METADATA-001) via the
`patches` and `edges` tables. In-memory operations use an adjacency list representation
with `RwLock`-protected concurrent access (REQ-CORE-004).

---

## BP-2: Design Decomposition

### 2.1 Core Types

```rust
pub struct PatchDag {
    nodes: HashMap<PatchId, DagNode>,
    edges: HashMap<PatchId, Vec<PatchId>>,    // parent → children
    reverse_edges: HashMap<PatchId, Vec<PatchId>>, // child → parents
    branches: HashMap<BranchName, Branch>,
    head: Option<PatchId>,                    // Current HEAD (may be detached)
}

pub struct DagNode {
    pub patch: Patch,
    pub depth: u64,        // Longest path from root to this node
    pub children: Vec<PatchId>,
    pub parents: Vec<PatchId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BranchName(String);

pub struct Branch {
    pub name: BranchName,
    pub target: PatchId,
    pub created_at: i64,
}

pub struct MergePlan {
    pub base: PatchId,                    // LCA of the two branches
    pub patches_from_a: Vec<Patch>,       // Unique patches from branch A
    pub patches_from_b: Vec<Patch>,       // Unique patches from branch B
    pub merge_result: MergeResult,        // From BP-PATCH-ALGEBRA-001
}
```

### 2.2 Graph Module (`graph.rs`)

Manages the DAG topology: adding nodes, edges, cycle detection, and graph queries.

### 2.3 Branch Module (`branch.rs`)

Manages named branches: create, delete, rename, list, move pointer.

### 2.4 Merge Module (`merge.rs`)

Computes merge plans using LCA and delegates conflict detection to the Patch Algebra Engine.

---

## BP-3: Design Rationale

### 3.1 Adjacency List with HashMap Index

The DAG uses two HashMap-based adjacency lists:
- `edges`: maps each parent to its children (forward traversal).
- `reverse_edges`: maps each child to its parents (backward traversal).

This representation provides:
- $O(1)$ lookup of a node's parents or children.
- $O(|V| + |E|)$ ancestor computation via BFS.
- $O(|V|)$ memory overhead for the reverse index (acceptable for typical repo sizes).

**Alternative considered:** Adjacency matrix ($O(|V|^2)$ memory) — rejected because DAGs
are typically sparse (each node has 1–2 parents).

### 3.2 RwLock for Concurrent Access

The DAG uses `RwLock<PatchDag>` for concurrency:
- **Read operations** (ancestor queries, branch listing, LCA) acquire a read lock,
  allowing multiple concurrent readers (REQ-CORE-004).
- **Write operations** (add_patch, create_branch, merge) acquire an exclusive write lock.

This satisfies REQ-CORE-005 (single-writer pipeline) while enabling concurrent reads.

**Alternative considered:** Lock-free data structures — rejected because DAG mutations
are rare relative to reads, and the complexity of lock-free graph structures is not
justified for the expected access pattern.

### 3.3 In-Memory + SQLite Hybrid

The DAG is loaded from SQLite into memory on repository open. All queries operate on the
in-memory representation. Mutations are persisted to SQLite after each write operation.
This provides:
- Sub-millisecond query latency (REQ-PERF-002).
- ACID persistence via SQLite WAL mode (REQ-META-001).
- Simple recovery from crashes (reload from SQLite).

---

## BP-4: Traceability

| Requirement | Satisfied By | Verification Method |
|-------------|-------------|-------------------|
| REQ-DAG-001 (patch DAG) | BP-2.1, PatchDag | Unit tests |
| REQ-DAG-002 (acyclicity guarantee) | BP-5, add_patch cycle detection | Property tests |
| REQ-DAG-003 (named branches) | BP-2.3, Branch | Unit tests |
| REQ-DAG-004 (LCA computation) | BP-5, lca() | Unit tests |
| REQ-DAG-005 (transitive ancestors) | BP-5, ancestors() | Unit tests |
| REQ-DAG-006 (persistent SQLite) | BP-6, SQL tables | Integration tests |
| REQ-DAG-007 (branch CRUD) | BP-5, branch operations | Unit tests |
| REQ-DAG-008 (detached HEAD) | BP-2.1, head: Option | Unit tests |
| REQ-DAG-009 (topological ordering) | BP-5, topo_order() | Unit tests |
| REQ-CORE-004 (concurrent reads) | BP-3.2, RwLock | Integration tests |
| REQ-CORE-005 (single-writer) | BP-3.2, RwLock | Integration tests |
| REQ-PERF-001 (merge < 10ms for 10k patches) | In-memory operations | Benchmarks |
| REQ-PERF-002 (sub-ms DAG queries) | In-memory HashMap | Benchmarks |

---

## BP-5: Interface Design

### IF-DAG-001: PatchDag Public API

```rust
impl PatchDag {
    /// Load a DAG from the metadata database.
    pub fn load(meta: &MetadataStore) -> Result<Self, DagError>;

    /// Persist the current DAG state to the metadata database.
    pub fn save(&self, meta: &MetadataStore) -> Result<(), DagError>;

    /// Add a patch to the DAG with one or more parents.
    ///
    /// Precondition: patch.id ∉ nodes (no duplicates).
    /// Precondition: All elements of parents ∈ nodes (parents must exist).
    /// Postcondition: patch is a node in the DAG.
    /// Postcondition: Edges from each parent to patch exist.
    /// Postcondition: DAG remains acyclic (returns DagError::CycleDetected otherwise).
    /// Complexity: O(|V| + |E|) for cycle detection.
    pub fn add_patch(
        &mut self,
        patch: Patch,
        parents: Vec<PatchId>,
    ) -> Result<PatchId, DagError>;

    /// Create a named branch pointing to a specific patch.
    ///
    /// Precondition: target ∈ nodes.
    /// Precondition: name is not already used.
    /// Postcondition: branches[name].target == target.
    pub fn create_branch(
        &mut self,
        name: BranchName,
        target: PatchId,
    ) -> Result<(), DagError>;

    /// Delete a named branch.
    ///
    /// Precondition: name exists in branches.
    /// Postcondition: name is removed from branches.
    /// Note: Cannot delete the branch that HEAD points to.
    pub fn delete_branch(&mut self, name: &BranchName) -> Result<(), DagError>;

    /// Rename a branch.
    ///
    /// Precondition: old_name exists, new_name does not exist.
    pub fn rename_branch(
        &mut self,
        old_name: &BranchName,
        new_name: BranchName,
    ) -> Result<(), DagError>;

    /// List all branch names.
    pub fn list_branches(&self) -> Vec<&BranchName>;

    /// Move a branch pointer to a new target.
    ///
    /// Precondition: name exists, target ∈ nodes.
    pub fn move_branch(
        &mut self,
        name: &BranchName,
        target: PatchId,
    ) -> Result<(), DagError>;

    /// Compute a merge plan for two branches.
    ///
    /// Precondition: Both branch names exist.
    /// Postcondition: MergePlan contains the LCA, unique patches from each branch,
    ///   and the merge result (including any conflicts).
    pub fn merge_branches(
        &self,
        a: &BranchName,
        b: &BranchName,
    ) -> Result<MergePlan, DagError>;

    /// Compute the transitive ancestor set of a node.
    ///
    /// Precondition: patch_id ∈ nodes.
    /// Postcondition: Returns all nodes reachable via reverse edges from patch_id.
    /// Complexity: O(|V| + |E|) via BFS.
    pub fn ancestors(&self, patch_id: PatchId) -> HashSet<PatchId>;

    /// Compute the Lowest Common Ancestor of two nodes.
    ///
    /// Precondition: a, b ∈ nodes.
    /// Postcondition: Returns the deepest common ancestor, or None if no common
    ///   ancestor exists.
    /// Complexity: O(|V| + |E|).
    pub fn lca(&self, a: PatchId, b: PatchId) -> Option<PatchId>;

    /// Return all nodes in topological order.
    ///
    /// Postcondition: For every edge (u, v), u appears before v in the output.
    pub fn topo_order(&self) -> Vec<PatchId>;

    /// Set HEAD to a specific node (detached HEAD mode).
    ///
    /// Precondition: target ∈ nodes.
    pub fn detach_head(&mut self, target: PatchId) -> Result<(), DagError>;

    /// Set HEAD to point to a branch.
    pub fn set_head_branch(&mut self, name: &BranchName) -> Result<(), DagError>;
}

#[derive(Debug, thiserror::Error)]
pub enum DagError {
    #[error("patch not found: {0}")]
    PatchNotFound(PatchId),
    #[error("branch not found: {0}")]
    BranchNotFound(String),
    #[error("branch already exists: {0}")]
    BranchAlreadyExists(String),
    #[error("cycle detected: adding patch {0} would create a cycle")]
    CycleDetected(PatchId),
    #[error("no common ancestor between {0} and {1}")]
    NoCommonAncestor(PatchId, PatchId),
    #[error("cannot delete branch '{0}': HEAD is attached to it")]
    CannotDeleteCurrentBranch(String),
}
```

---

## BP-6: Data Design

### 6.1 SQLite Tables

```sql
CREATE TABLE patches (
    id TEXT PRIMARY KEY,
    parent_ids TEXT NOT NULL,       -- JSON array of parent patch IDs
    depth INTEGER NOT NULL,         -- Longest path from root
    operation_type TEXT NOT NULL,
    touch_set TEXT NOT NULL,        -- JSON array of addresses
    payload BLOB,
    timestamp TEXT NOT NULL,
    author TEXT NOT NULL,
    signature BLOB
);

CREATE TABLE edges (
    parent_id TEXT NOT NULL REFERENCES patches(id),
    child_id TEXT NOT NULL REFERENCES patches(id),
    PRIMARY KEY (parent_id, child_id)
);

CREATE INDEX idx_edges_child ON edges(child_id);
CREATE INDEX idx_edges_parent ON edges(parent_id);

CREATE TABLE branches (
    name TEXT PRIMARY KEY,
    target_patch_id TEXT NOT NULL REFERENCES patches(id),
    created_at TEXT NOT NULL
);
```

### 6.2 In-Memory Layout

```
PatchDag {
    nodes: HashMap<PatchId, DagNode>,          // O(1) lookup by ID
    edges: HashMap<PatchId, Vec<PatchId>>,      // parent → [children]
    reverse_edges: HashMap<PatchId, Vec<PatchId>>, // child → [parents]
    branches: HashMap<BranchName, Branch>,      // name → branch pointer
    head: Option<PatchId>,                      // detached or branch-attached
}
```

Total memory: $O(|V| + |E| + |B|)$ where $|B|$ is the number of branches.

---

## BP-7: Component Design

### 7.1 Module Structure

```
suture-core/src/
  dag/
    mod.rs              -- PatchDag struct, public API, DagError
    graph.rs            -- add_patch, cycle detection, topo_order, ancestors
    branch.rs           -- create_branch, delete_branch, rename_branch, move_branch
    merge.rs            -- merge_branches, lca computation
    persistence.rs      -- load/save from SQLite
```

### 7.2 Dependency Graph

```
graph.rs ←── merge.rs ←── Patch Algebra (BP-PATCH-ALGEBRA-001)
    ↑                         ↑
    └──── branch.rs ─────────┘
    ↑
    └──── persistence.rs ←── MetadataStore (BP-METADATA-001)
```

---

## BP-8: Deployment

### 8.1 Library Crate

Deployed as part of `suture-core`. The DAG module depends on the patch algebra module
(BP-PATCH-ALGEBRA-001) for merge conflict detection and on the metadata store
(BP-METADATA-001) for persistence.

### 8.2 Concurrency

```rust
pub struct DagHandle {
    inner: Arc<RwLock<PatchDag>>,
}

impl DagHandle {
    pub async fn read<F, T>(&self, f: F) -> T
    where
        F: FnOnce(&PatchDag) -> T,
    {
        let guard = self.inner.read().await;
        f(&guard)
    }

    pub async fn write<F, T>(&self, f: F) -> T
    where
        F: FnOnce(&mut PatchDag) -> T,
    {
        let mut guard = self.inner.write().await;
        f(&mut guard)
    }
}
```

---

## BP-9: Formal Verification

### THM-DAG-001: Acyclicity and Termination (from YP-ALGEBRA-PATCH-001)

> *The algorithm for adding a new patch P to the Patch-DAG terminates in O(|V| + |E|) time,
>   and the DAG remains acyclic after every successful add_patch operation.*

**Implementation obligation:**
1. `add_patch` MUST perform cycle detection after inserting the new node and edges.
2. Cycle detection MUST use DFS with a recursion stack (white/gray/black coloring).
3. If a cycle is detected, the insertion MUST be rolled back and `DagError::CycleDetected`
   MUST be returned.
4. The DAG invariant (no cycles) MUST hold at all times between write operations.

**Test obligations:**
- Property test: for any sequence of `add_patch` operations, the resulting graph has no
  cycles (verified by topological sort).
- Unit test: adding a patch that creates a cycle returns `DagError::CycleDetected`.
- Unit test: the DAG root (identity patch) is always reachable from every node.

### THM-DAG-002: LCA Uniqueness (from YP-ALGEBRA-PATCH-001)

> *For any two nodes a, b in a well-formed Patch-DAG, the LCA is unique.*

**Implementation obligation:**
- `lca()` MUST compute ancestor sets for both nodes and return the deepest common ancestor.
- If multiple LCAs exist (criss-cross merge), the implementation MUST detect this and
  return a single representative LCA (the deepest one, or synthesize a virtual merge base).

**Test obligations:**
- Unit test: linear history — LCA of any two nodes is the earlier one.
- Unit test: single merge — LCA of two branch tips is the merge base.
- Unit test: criss-cross merge — LCA returns the deepest common ancestor.

---

## BP-12: Quality Checklist

- [ ] All public API functions have documented preconditions and postconditions.
- [ ] Cycle detection is tested: adding a back-edge returns CycleDetected.
- [ ] LCA computation tested: linear, single-merge, and criss-cross scenarios.
- [ ] Ancestor computation tested: BFS from any node reaches the root.
- [ ] Topological ordering tested: every edge (u, v) has u before v.
- [ ] Branch CRUD tested: create, delete, rename, move, list.
- [ ] Detached HEAD tested: set, unset, branch attachment.
- [ ] Persistence tested: save → load produces identical in-memory state.
- [ ] Concurrent access tested: multiple readers + single writer via DagHandle.
- [ ] Property test: any sequence of add_patch operations produces an acyclic graph.
- [ ] Property test: merge result is deterministic (merge(a, b) == merge(b, a)).
- [ ] `cargo clippy` passes with zero warnings on the `dag` module.
- [ ] `cargo test` passes all DAG tests.

---

*End of BP-PATCH-DAG-001*
