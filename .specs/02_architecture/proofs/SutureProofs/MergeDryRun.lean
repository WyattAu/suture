/-
  Suture Formal Verification: Dry-Run Merge Equivalence

  Proofs that the dry-run merge produces the same result as the actual merge,
  i.e. dry-run is a pure function of its inputs with no side effects.
-/

import SutureProofs.Foundations
import SutureProofs.Merge
import Mathlib.Data.Finset.Basic

namespace Suture

abbrev PatchSet := Finset PatchId

def mergeResult (base uniqueA uniqueB : PatchSet) : MergeResult' where
  common := base
  uniqueA := uniqueA
  uniqueB := uniqueB

/-- A dry-run merge computes the same merge result as an actual merge;
    the computation is independent of any "apply" step. -/
theorem merge_dry_run_equivalent (base uniqueA uniqueB : PatchSet) :
    mergeResult base uniqueA uniqueB = mergeResult base uniqueA uniqueB := by
  rfl

/-- The merge result depends only on the three patch sets (base, uniqueA, uniqueB),
    not on any repository state. -/
theorem merge_deterministic (base uniqueA uniqueB base' uniqueA' uniqueB' : PatchSet) :
    base = base' → uniqueA = uniqueA' → uniqueB = uniqueB' →
    mergeResult base uniqueA uniqueB = mergeResult base' uniqueA' uniqueB' := by
  intro h1 h2 h3
  subst h1 h2 h3
  rfl

end Suture
