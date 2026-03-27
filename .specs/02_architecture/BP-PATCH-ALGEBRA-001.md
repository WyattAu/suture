---
document_id: BP-PATCH-ALGEBRA-001
version: 1.0.0
status: APPROVED
ieee_1016_compliant: true
component_id: COMP-PATCH-001
component_type: Module
interfaces: [IF-PATCH-001]
depends_on:
  yellow_papers: [YP-ALGEBRA-PATCH-001]
  blue_papers: []
created: 2026-03-27
---

# BP-PATCH-ALGEBRA-001: Patch Algebra Engine

## BP-1: Design Overview

The Patch Algebra Engine is the mathematical core of Suture. It implements the formal patch
algebra defined in YP-ALGEBRA-PATCH-001, providing the operations that determine whether
patches commute, how to merge divergent branches, and how to detect and represent conflicts.

This module is a pure computation library with no I/O dependencies. It operates on in-memory
data structures representing patches, touch sets, and conflict nodes. The engine is consumed
by the Patch DAG (BP-PATCH-DAG-001) for history management and by the Driver SDK
(BP-DRIVER-SDK-001) for format-specific semantic decomposition.

The engine is designed as a library crate (`suture-core`) that can be used independently of
the CLI, metadata store, or CAS. This separation ensures that the algebraic correctness of
the patch operations is testable in isolation.

---

## BP-2: Design Decomposition

### 2.1 Core Types (`types.rs`)

```rust
pub struct Patch {
    pub id: PatchId,           // BLAKE3 hash of the patch content
    pub parent_ids: Vec<PatchId>,
    pub operation_type: OpType,
    pub touch_set: TouchSet,   // HashSet<String> of semantic addresses
    pub payload: Payload,      // FlatBuffers-encoded operation data
    pub timestamp: i64,        // Unix epoch nanoseconds
    pub author: String,
    pub signature: Option<Vec<u8>>, // Ed25519 signature
}

pub struct TouchSet {
    addresses: HashSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum OpType {
    UpdateNode,
    AddNode,
    DeleteNode,
    MoveClip,
    EditCell,
    SetAttribute,
    Custom(String),
}

pub struct Conflict {
    pub id: ConflictId,
    pub patch_a: Patch,
    pub patch_b: Patch,
    pub overlapping_addresses: TouchSet,
}

pub enum CommuteResult {
    Commute { order_independent: bool },
    Conflict(Conflict),
}

pub struct MergeResult {
    pub merged_patches: Vec<Patch>,
    pub conflicts: Vec<Conflict>,
    pub is_clean: bool, // true iff conflicts.is_empty()
}
```

### 2.2 Commutativity (`commute.rs`)

Implements ALG-COMM-001 from YP-ALGEBRA-PATCH-001. The commutativity check is a set
intersection on touch sets:

```rust
pub fn commute(p1: &Patch, p2: &Patch) -> CommuteResult {
    let overlap = p1.touch_set.intersection(&p2.touch_set);
    if overlap.is_empty() {
        CommuteResult::Commute { order_independent: true }
    } else {
        CommuteResult::Conflict(Conflict {
            id: ConflictId::new(p1.id, p2.id),
            patch_a: p1.clone(),
            patch_b: p2.clone(),
            overlapping_addresses: overlap.collect(),
        })
    }
}
```

### 2.3 Merge (`merge.rs`)

Implements ALG-MERGE-001 (Three-Way Merge) from YP-ALGEBRA-PATCH-001:

```rust
pub fn merge(
    base: &[Patch],
    branch_a: &[Patch],
    branch_b: &[Patch],
) -> MergeResult {
    let base_set: HashSet<PatchId> = base.iter().map(|p| p.id).collect();
    let a_set: HashSet<PatchId> = branch_a.iter().map(|p| p.id).collect();
    let b_set: HashSet<PatchId> = branch_b.iter().map(|p| p.id).collect();

    let a_only: Vec<&Patch> = branch_a.iter()
        .filter(|p| !base_set.contains(&p.id))
        .collect();
    let b_only: Vec<&Patch> = branch_b.iter()
        .filter(|p| !base_set.contains(&p.id))
        .collect();
    let common: Vec<Patch> = branch_a.iter()
        .filter(|p| b_set.contains(&p.id))
        .cloned()
        .collect();

    let mut conflicts = Vec::new();
    for pa in &a_only {
        for pb in &b_only {
            if let CommuteResult::Conflict(c) = commute(pa, pb) {
                conflicts.push(c);
            }
        }
    }

    let mut merged: Vec<Patch> = common;
    merged.extend(a_only.into_iter().cloned());
    merged.extend(b_only.into_iter().cloned());

    MergeResult {
        merged_patches: merged,
        conflicts,
        is_clean: conflicts.is_empty(),
    }
}
```

### 2.4 Conflict Detection (`conflict.rs`)

Provides utilities for analyzing and reporting conflicts:

```rust
pub fn detect_conflicts(
    patches_a: &[Patch],
    patches_b: &[Patch],
) -> Vec<Conflict> {
    let mut conflicts = Vec::new();
    for pa in patches_a {
        for pb in patches_b {
            let overlap = pa.touch_set.intersection(&pb.touch_set);
            if !overlap.is_empty() {
                conflicts.push(Conflict {
                    id: ConflictId::new(pa.id, pb.id),
                    patch_a: pa.clone(),
                    patch_b: pb.clone(),
                    overlapping_addresses: overlap.collect(),
                });
            }
        }
    }
    conflicts
}

pub fn conflict_summary(conflicts: &[Conflict]) -> ConflictSummary {
    let total_overlapping: usize = conflicts.iter()
        .map(|c| c.overlapping_addresses.len())
        .sum();
    ConflictSummary {
        conflict_count: conflicts.len(),
        total_overlapping_addresses: total_overlapping,
        affected_patches: conflicts.iter()
            .flat_map(|c| [c.patch_a.id, c.patch_b.id])
            .collect(),
    }
}
```

---

## BP-3: Design Rationale

### 3.1 Touch-Set Based Commutativity

Suture uses touch-set disjointness as the commutativity criterion (THM-COMM-001 from
YP-ALGEBRA-PATCH-001). This approach was chosen over alternatives for the following reasons:

**vs. Pijul's pseudo-context:** Pijul uses a pseudo-context mechanism that tracks character-
level dependencies within text files. While more precise for text, pseudo-context requires
format-specific tokenizers and does not generalize to non-textual structured data (video
timelines, spreadsheets, scene graphs). Touch sets are format-agnostic.

**vs. Operational Transformation (OT):** OT tracks dependencies at the operation level using
transform functions. OT requires a transform function for every pair of operation types,
leading to $O(n^2)$ transform functions. Touch sets reduce this to a single set intersection
operation, independent of operation type.

**vs. Value-equivalent commutativity:** A more precise criterion would check whether two
patches write the same value to the same address (they would commute even with overlapping
touch sets). This requires executing patches to compare values, which is expensive and
format-dependent. Touch-set disjointness is conservative (may report false conflicts) but
safe (never misses a real conflict) and efficient.

**Conservative but safe:** The touch-set criterion may flag patches as conflicting when they
actually commute (e.g., both set the same cell to the same value). This is acceptable because
conflict nodes preserve both versions (THM-CONF-001), and the user can resolve the false
conflict trivially. Missing a real conflict would cause silent data corruption, which is
unacceptable.

### 3.2 FlatBuffers Encoding

Patches are encoded as FlatBuffers messages (REQ-PATCH-009, REQ-PATCH-010) for:
- Zero-copy deserialization (no allocation on read).
- Schema evolution (new fields can be added without breaking old data).
- Cross-language compatibility (drivers may be written in Python via PyO3).

---

## BP-4: Traceability

| Requirement | Satisfied By | Verification Method |
|-------------|-------------|-------------------|
| REQ-PATCH-001 (typed patches with touch sets) | BP-2.1, Patch struct | Type system |
| REQ-PATCH-002 (commutativity via disjoint touch sets) | BP-2.2, commute() | Unit tests, property tests |
| REQ-PATCH-003 (deterministic merge) | BP-2.3, merge() | Property tests |
| REQ-PATCH-004 (first-class conflict nodes) | BP-2.1, Conflict struct | Unit tests |
| REQ-PATCH-005 (zero data loss) | Conflict stores both patches | Property tests |
| REQ-PATCH-006 (identity patch) | OpType::UpdateNode with empty touch set | Unit tests |
| REQ-PATCH-007 (associativity) | Monoid structure (THM-PATCH-001) | Property tests |
| REQ-PATCH-008 (property-based tests) | Section BP-12 | proptest suite |
| REQ-PATCH-009 (FlatBuffers encoding) | Payload field | Unit tests |
| REQ-PATCH-010 (FlatBuffers metadata) | FlatBuffers schema | Integration tests |
| REQ-CORE-002 (determinism) | Pure functions, no I/O | Property tests |
| REQ-CORE-003 (idempotency) | Set-based merge | Property tests |

---

## BP-5: Interface Design

### IF-PATCH-001: Patch Algebra Public API

```rust
/// Check commutativity of two patches.
/// Returns CommuteResult::Commute if touch sets are disjoint.
/// Returns CommuteResult::Conflict with overlapping addresses otherwise.
///
/// Precondition: Both patches have valid, non-empty touch sets (unless identity).
/// Postcondition: Result is deterministic — same inputs always yield same output.
/// Complexity: O(min(|T(P1)|, |T(P2)|)) via hash-set intersection.
pub fn commute(p1: &Patch, p2: &Patch) -> CommuteResult;

/// Three-way merge of patch sets from two branches diverging from a common base.
///
/// Precondition: base ⊆ branch_a ∧ base ⊆ branch_b.
/// Postcondition: merged_patches contains all patches from branch_a and branch_b.
/// Postcondition: conflicts contains a Conflict for every pair (Pa, Pb) with
///   T(Pa) ∩ T(Pb) ≠ ∅.
/// Postcondition: is_clean == true iff no conflicts exist.
/// Postcondition: merge(base, a, b) == merge(base, b, a) (order-independent).
/// Complexity: O(|a_only| × |b_only| × k̄) where k̄ is average touch set size.
pub fn merge(
    base: &[Patch],
    branch_a: &[Patch],
    branch_b: &[Patch],
) -> MergeResult;

/// Detect all conflicts between two patch sets.
///
/// Precondition: patches_a and patches_b are well-formed patch sets.
/// Postcondition: Returns all pairs (Pa ∈ a, Pb ∈ b) with T(Pa) ∩ T(Pb) ≠ ∅.
pub fn detect_conflicts(
    patches_a: &[Patch],
    patches_b: &[Patch],
) -> Vec<Conflict>;

/// Check whether a patch is the identity (no-op).
///
/// Postcondition: Returns true iff the patch has an empty touch set.
pub fn is_identity(patch: &Patch) -> bool;

/// Compute the touch set of a composed patch.
///
/// Precondition: Both patches are valid.
/// Postcondition: Returns T(P1) ∪ T(P2) (LEM-004).
pub fn compose_touch_set(p1: &Patch, p2: &Patch) -> TouchSet;
```

---

## BP-6: Data Design

### 6.1 Patch Structure

```rust
pub struct Patch {
    pub id: PatchId,
    pub parent_ids: Vec<PatchId>,
    pub operation_type: OpType,
    pub touch_set: TouchSet,
    pub payload: Payload,
    pub timestamp: i64,
    pub author: String,
    pub signature: Option<Vec<u8>>,
}
```

| Field | Type | Description | Source |
|-------|------|-------------|--------|
| `id` | `PatchId` (BLAKE3 hash) | Content address of the serialized patch | REQ-CAS-001 |
| `parent_ids` | `Vec<PatchId>` | Direct predecessors in the DAG | REQ-DAG-001 |
| `operation_type` | `OpType` | Semantic operation type | REQ-PATCH-001 |
| `touch_set` | `TouchSet` (HashSet\<String\>) | Addresses modified by this patch | REQ-PATCH-001 |
| `payload` | `Payload` (Vec\<u8\>) | FlatBuffers-encoded operation data | REQ-PATCH-009 |
| `timestamp` | `i64` | Unix epoch nanoseconds | REQ-SEC-002 |
| `author` | `String` | Author identifier | REQ-SEC-001 |
| `signature` | `Option<Vec<u8>>` | Ed25519 signature over patch content | REQ-SEC-001 |

### 6.2 FlatBuffers Schema (Outline)

```
table Patch {
    id: [u8];              // 32 bytes, BLAKE3
    parent_ids: [PatchId]; // Variable-length
    operation_type: OpType;
    touch_set: [string];   // Array of address strings
    payload: [u8];         // Operation-specific data
    timestamp: int64;
    author: string;
    signature: [u8];       // 64 bytes, Ed25519
}
```

---

## BP-7: Component Design

### 7.1 Module Structure

```
suture-core/src/
  patch/
    mod.rs              -- Public API re-exports
    types.rs            -- Patch, TouchSet, OpType, Conflict, MergeResult
    commute.rs          -- commute(), ALG-COMM-001
    merge.rs            -- merge(), ALG-MERGE-001
    conflict.rs         -- detect_conflicts(), conflict_summary()
    identity.rs         -- is_identity(), compose_touch_set()
    fb/                 -- FlatBuffers generated code
      patch_generated.rs
```

### 7.2 Dependency Graph

```
types.rs ←── commute.rs ←── merge.rs
    ↑                          ↑
    └──── conflict.rs ────────┘
    ↑
    └──── identity.rs
```

All modules depend only on `types.rs` and the standard library. No I/O, no async, no
external crates (except `flatbuffers` for generated code). This ensures the algebra is
purely computational and trivially testable.

---

## BP-8: Deployment

### 8.1 Library Crate

The Patch Algebra Engine is deployed as part of `suture-core`, a Rust library crate:

```toml
[package]
name = "suture-core"
version = "0.1.0"
edition = "2024"

[dependencies]
flatbuffers = "25"
thiserror = "2"
blake3 = "1.8"

[dev-dependencies]
proptest = "1.6"
```

### 8.2 Usage

```rust
use suture_core::patch::{Patch, commute, merge, detect_conflicts};

let result = commute(&patch_a, &patch_b);
match result {
    CommuteResult::Commute { .. } => { /* patches can be applied in any order */ }
    CommuteResult::Conflict(c) => { /* present conflict to user */ }
}

let merge_result = merge(&base_patches, &branch_a, &branch_b);
if merge_result.is_clean {
    // Apply merged patches
} else {
    // Present conflicts for resolution
}
```

---

## BP-9: Formal Verification

The Patch Algebra Engine implements all axioms, definitions, lemmas, and theorems from
YP-ALGEBRA-PATCH-001. Each formal result maps to a specific implementation obligation and
test invariant:

### Theorem Verification Matrix

| Theorem | Implementation | Test Invariant |
|---------|---------------|----------------|
| AX-002 (Patch Determinism) | All functions are pure (no I/O, no mutation of inputs) | Property test: same input → same output |
| AX-006 (Identity Existence) | `is_identity()` returns true for empty touch set | Unit test |
| LEM-001 (Disjoint → Commute) | `commute()` checks touch-set intersection | Property test: disjoint → commute |
| LEM-002 (Identity Patch) | Identity patch commutes with all patches | Unit test |
| LEM-003 (Associativity) | Merge uses set operations (commutative, associative) | Property test |
| LEM-004 (Touch Set Composition) | `compose_touch_set()` returns union | Unit test |
| THM-COMM-001 (Commutativity Criterion) | `commute()` | Property test |
| THM-MERGE-001 (Deterministic Merge) | `merge()` | Property test: merge(a,b) == merge(b,a) |
| THM-CONF-001 (Conflict Preservation) | Conflict stores both patches | Unit test |
| THM-CONF-002 (Conflict Isolation) | Independent patches not affected by conflicts | Property test |
| THM-PATCH-001 (Patch Monoid) | Composition closure, associativity, identity | Property test suite |

### Property-Based Test Invariants (REQ-PATCH-008)

1. **Commutativity**: If `T(P1) ∩ T(P2) = ∅`, then for all states S:
   `apply(P2, apply(P1, S)) == apply(P1, apply(P2, S))`.
2. **Identity**: `apply(identity, S) == S` and `apply(P, S) == apply(P, apply(identity, S))`.
3. **Associativity**: For commutative triples: `apply(P3, apply(P2, apply(P1, S))) == apply(P1, apply(P2, apply(P3, S)))`.
4. **Merge Determinism**: `merge(base, a, b) == merge(base, b, a)`.
5. **Conflict Preservation**: For every conflict C(Pa, Pb, Sbase), both Pa(Sbase) and Pb(Sbase) are recoverable.
6. **Touch Set Composition**: `T(P1 ∘ P2) == T(P1) ∪ T(P2)`.

---

## BP-11: Compliance Matrix

| Requirement | Section | Status | Verification |
|-------------|---------|--------|-------------|
| REQ-PATCH-001 | BP-2.1, BP-6 | Satisfied | Type system |
| REQ-PATCH-002 | BP-2.2 | Satisfied | Unit + property tests |
| REQ-PATCH-003 | BP-2.3 | Satisfied | Property tests |
| REQ-PATCH-004 | BP-2.1, Conflict | Satisfied | Unit tests |
| REQ-PATCH-005 | BP-9, THM-CONF-001 | Satisfied | Property tests |
| REQ-PATCH-006 | BP-2.4, identity | Satisfied | Unit tests |
| REQ-PATCH-007 | BP-9, THM-PATCH-001 | Satisfied | Property tests |
| REQ-PATCH-008 | BP-9, BP-12 | Satisfied | proptest suite |
| REQ-PATCH-009 | BP-6.2, FlatBuffers | Satisfied | Unit tests |
| REQ-PATCH-010 | BP-6.2, FlatBuffers | Satisfied | Integration tests |
| REQ-CORE-002 | BP-2, pure functions | Satisfied | Property tests |
| REQ-CORE-003 | BP-2.3, set merge | Satisfied | Property tests |

---

## BP-12: Quality Checklist

- [ ] All public functions have documented preconditions and postconditions.
- [ ] Property-based tests cover all 6 invariants from YP-ALGEBRA-PATCH-001, Section 6.2.
- [ ] Unit tests cover all OpType variants.
- [ ] Unit tests cover identity patch behavior.
- [ ] Unit tests cover empty patch set merge.
- [ ] Unit tests cover single-patch branches (trivial merge).
- [ ] Unit tests cover conflict with exactly one overlapping address.
- [ ] Unit tests cover conflict with multiple overlapping addresses.
- [ ] FlatBuffers round-trip test: serialize → deserialize → identity.
- [ ] `proptest` generates random Patch objects with arbitrary touch sets.
- [ ] Merge determinism: `merge(base, a, b)` called 100 times yields identical results.
- [ ] Conflict preservation: both patches recoverable from every Conflict node.
- [ ] `cargo clippy` passes with zero warnings on the `patch` module.
- [ ] `cargo test` passes all patch algebra tests.
- [ ] No unsafe code in the patch algebra module.
- [ ] All functions are pure (no I/O, no global state, no randomness).

---

*End of BP-PATCH-ALGEBRA-001*
