# Pattern Library

Reusable design patterns from the Suture implementation.

## Pattern: Content-Addressed Storage

**Problem:** Deduplicate data and guarantee integrity without explicit indexing.

**Solution:** Store each blob at a path derived from its BLAKE3 hash. The hash
serves as both address and integrity check.

```
objects/
  ab/         # First 2 hex chars (256 buckets)
    cdef...   # Remaining 62 hex chars
```

**Consequences:**
- Automatic deduplication — identical content maps to the same path
- Integrity verification on read — recompute hash and compare
- No garbage collection needed for references (but orphan cleanup requires DAG scan)

## Pattern: Patch Commutativity via Touch Sets

**Problem:** Determine when two edits can be applied in any order without conflict.

**Solution:** Each patch declares a `TouchSet` — the set of addresses it modifies.
Two patches commute if and only if their touch sets are disjoint.

```
Patch A: touch_set = {clip/1, clip/2}
Patch B: touch_set = {clip/3, clip/4}
→ A and B commute (disjoint)

Patch A: touch_set = {clip/1}
Patch C: touch_set = {clip/1}
→ A and C do NOT commute (intersection at clip/1)
```

**Consequences:**
- Linear-time commutativity check (hash set intersection)
- Conservative — may report conflicts for semantically compatible edits
- Drivers are responsible for computing correct touch sets

## Pattern: First-Class Conflict Nodes

**Problem:** Traditional VCS either rejects merges or silently overwrites one side.

**Solution:** When patches conflict, create a `ConflictNode` that preserves both
versions explicitly. The user (or a format-specific resolver) decides how to
proceed.

```
ConflictNode {
    patch_a: Patch,   // version from branch A
    patch_b: Patch,   // version from branch B
    conflict_addresses: Vec<String>,
}
```

**Consequences:**
- No information loss — both versions are always retained
- Conflicts are addressable and queryable in the DAG
- Resolution is deferred and format-aware

## Pattern: DAG-Based History

**Problem:** Linear history cannot represent parallel development or cherry-picks.

**Solution:** Store patches as nodes in a directed acyclic graph. Merge commits
have multiple parents. Branches are named pointers to specific nodes.

**Consequences:**
- Arbitrary topologies (linear, fork, diamond, octopus)
- Lowest-common-ancestor (LCA) computed via standard DAG algorithms
- Ancestor queries support blame, log, and merge-base operations
- Acyclicity invariant enforced on every `add_patch` call
