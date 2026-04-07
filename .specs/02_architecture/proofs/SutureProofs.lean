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
   
   Sorry-free theorems: 11/13 (THM-DAG-001 acyclicity preservation and
   batch commit theorems have sorry due to inductive reasoning complexity).
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
