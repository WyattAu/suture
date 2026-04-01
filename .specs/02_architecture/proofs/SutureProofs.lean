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
  
  Sorry-free theorems: 6/7 (THM-DAG-001 acyclicity preservation has a sorry
  due to the complexity of inductive reasoning over Reachable paths).
-/

import SutureProofs.Foundations
import SutureProofs.Commutativity
import SutureProofs.Composition
import SutureProofs.Merge
import SutureProofs.Conflict
import SutureProofs.DAG
import SutureProofs.CAS
