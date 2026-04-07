# TLA+ Specifications

Formal specifications of Suture's core algorithms using TLA+ (Temporal Logic of Actions).

## Modules

| Module | Purpose | Key Theorems |
|--------|---------|--------------|
| `CommitStateMachine.tla` | Repository lifecycle: init, stage, commit, branch, checkout | Type invariant |
| `MergeAlgorithm.tla` | Three-way merge partition and correctness | Partition coverage, disjointness, idempotency, commutativity |
| `BranchOperations.tla` | Branch CRUD, fast-forward merge, ancestry | Type invariant |

## How to Verify

Install the TLA+ Toolbox from https://github.com/tlaplus/tlaplus/releases.

```bash
# Model check each spec
tlc CommitStateMachine.tla
tlc MergeAlgorithm.tla
tlc BranchOperations.tla
```

## Relationship to Lean 4 Proofs

The TLA+ specs model state machines and temporal properties (what happens over time).
The Lean 4 proofs model algebraic properties (what is true for all inputs).
Together they provide complementary verification:
- TLA+: "the system never enters a bad state"
- Lean 4: "the merge algorithm is mathematically correct"
