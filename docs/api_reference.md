# API Reference

## Core Types

### `Hash`

A BLAKE3 content hash (32 bytes / 256 bits). Used as the canonical identifier
for blobs and patches.

```rust
use suture_common::Hash;

// Compute hash from data
let hash = Hash::from_data(b"hello");

// Parse from hex string
let hash = Hash::from_hex("abc123...")?;

// Display
println!("{}", hash);          // abc123def456… (short form)
println!("{}", hash.to_hex()); // full 64-char hex

// Sentinel value
let zero = Hash::ZERO;
```

### `PatchId`

Alias for `Hash`. Identifies a patch uniquely by its BLAKE3 content hash.

### `BranchName`

A validated branch name (non-empty, no null bytes).

```rust
use suture_common::BranchName;
let name = BranchName::new("feature/cut")?;
println!("{}", name.as_str());
```

### `Patch`

The fundamental unit of change. Contains operation type, touch set, payload,
parent IDs, author, message, and timestamp.

```rust
use suture_core::patch::{Patch, TouchSet, OperationType};

let patch = Patch::new(
    OperationType::Modify,
    TouchSet::from_addrs(["clip/intro", "clip/outro"]),
    Some("timeline.otio".to_string()),
    vec![/* payload bytes */],
    vec![/* parent PatchIds */],
    "alice".to_string(),
    "Trim intro and outro".to_string(),
);
```

### `TouchSet`

A set of addresses/resources modified by a patch. Disjoint touch sets imply
commutativity (THM-COMM-001).

```rust
use suture_core::patch::TouchSet;

let ts = TouchSet::from_addrs(["clip/A1", "clip/B1"]);
ts.is_empty();             // false
ts.contains("clip/A1");    // true
ts.intersects(&other_ts);  // true if any address in common
```

## CAS API (`suture_core::cas`)

### `BlobStore::new(root)`

Creates a new content-addressable blob store at `root/objects/`.

```rust
use suture_core::cas::BlobStore;
let store = BlobStore::new(".suture")?;
```

### `store.put_blob(data) -> Result<Hash, CasError>`

Stores a blob, returning its BLAKE3 hash. Deduplicates automatically.

### `store.get_blob(hash) -> Result<Vec<u8>, CasError>`

Retrieves a blob by hash. Decompresses if needed and verifies integrity.

### `store.has_blob(hash) -> bool`

Checks existence without reading or verifying.

## Patch Algebra API (`suture_core::patch`)

### `commute(patch_a, patch_b) -> CommuteResult`

Determines if two patches commute. Returns `CommuteResult::Commutes` (with
reordered pair) or `CommuteResult::DoesNotCommute`.

### `merge(patch_a, patch_b) -> Result<MergeResult, MergeError>`

Merges two patches. If touch sets are disjoint, returns a clean merge.
If they overlap, returns a `MergeResult` containing conflict nodes.

### `detect_conflicts(patches) -> Vec<Conflict>`

Scans a list of patches and returns all pairwise conflicts based on
overlapping touch sets.

## DAG API (`suture_core::dag`)

### `PatchDag::new()`

Creates an empty patch DAG.

### `dag.add_patch(patch) -> Result<(), DagError>`

Adds a patch node to the DAG. Validates acyclicity (THM-DAG-001).

### `dag.create_branch(name, target_patch_id) -> Result<(), DagError>`

Creates a named branch pointing at the given patch (or root if `None`).

### `dag.ancestors(patch_id) -> Vec<PatchId>`

Returns all ancestors of a patch in topological order.

### `dag.lca(patch_a, patch_b) -> Option<PatchId>`

Computes the lowest common ancestor of two patches.

## Repository API (`suture_core::repository`)

### `Repository::init(path, author) -> Result<Repository, RepoError>`

Initializes a new Suture repository at the given path.

### `Repository::open(path) -> Result<Repository, RepoError>`

Opens an existing repository.

### `repo.add(file_path) -> Result<(), RepoError>`

Stages a file for the next commit.

### `repo.commit(message) -> Result<PatchId, RepoError>`

Creates a patch from the staged changes and adds it to the DAG.

### `repo.status() -> Result<RepoStatus, RepoError>`

Returns the current repository status (HEAD, branch, staged files, counts).

### `repo.log(branch) -> Result<Vec<LogEntry>, RepoError>`

Returns the commit history for a branch (or HEAD if `None`).

### `repo.create_branch(name, target) -> Result<(), RepoError>`

Creates a new branch at the given target (or HEAD if `None`).

### `repo.merge_plan(branch_a, branch_b) -> Result<MergePlan, RepoError>`

Computes a merge plan between two branches. Reports conflicts if touch sets
overlap.
