/-
  Suture Formal Verification — Root Module
  
  This module imports all proof modules for the Suture VCS patch algebra.
  Building this file with `lake build` verifies all theorems.
  
  Proof Summary (matching Yellow Paper YP-ALGEBRA-PATCH-001):
  
  | Theorem          | File                | Status    |
  |------------------|---------------------|-----------|
  | THM-COMM-001     | Commutativity.lean  | ✓ Proved  |
  | THM-COMPOSE-001  | Composition.lean    | ✓ Proved  |
  | THM-MERGE-001    | Merge.lean          | ✓ Proved  |
  | THM-CONF-001     | Conflict.lean       | ✓ Proved  |
  | THM-CONF-002     | Conflict.lean       | ✓ Proved  |
  | THM-DAG-001      | DAG.lean            | ⚠ Sketched|
  | CAS Integrity    | CAS.lean            | ✓ Proved  |
  | LEM-001 to LEM-005| Various             | ✓ Proved  |
   | THM-PATCH-001    | Composition.lean    | ✓ Proved  |
   | Config idempotent| Config.lean         | ✓ Proved  |
   | Config overrides | Config.lean         | ✓ Proved  |
   | Config roundtrip | Config.lean         | ✓ Proved  |
   | Config precedence| Config.lean         | ✓ Proved  |
   | Merge dry-run    | MergeDryRun.lean    | ✓ Proved  |
   | Merge deterministic| MergeDryRun.lean  | ✓ Proved  |
   | Batch apply equiv| BatchCommit.lean    | ⚠ Sketched|
   | Batch touch union| BatchCommit.lean    | ⚠ Sketched|
   | Undo preserves    | Undo.lean           | ⚠ Sketched|
   | Undo reversible   | Undo.lean           | ⚠ Sketched|
   | Undo graph preserv| Undo.lean           | ⚠ Sketched|
   | Staging monotonic | InteractiveStaging.lean | ✓ Proved|
   | Staged ⊆ changed  | InteractiveStaging.lean | ✓ Proved|
   | Empty staging val | InteractiveStaging.lean | ✓ Proved|
   | Unstage in changed| InteractiveStaging.lean | ✓ Proved|
   
   Sorry-free theorems: 15/21 (THM-DAG-001 acyclicity preservation,
   batch commit theorems, and undo theorems have sorry due to
   inductive reasoning complexity).
-/

import SutureProofs.Foundations
import SutureProofs.Commutativity
import SutureProofs.Composition
import SutureProofs.Merge
import SutureProofs.Conflict
import SutureProofs.DAG
import SutureProofs.CAS
import SutureProofs.Config
import SutureProofs.MergeDryRun
import SutureProofs.BatchCommit
import SutureProofs.Undo
import SutureProofs.InteractiveStaging
