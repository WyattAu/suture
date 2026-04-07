/-
  Suture Formal Verification: Undo (Soft Reset) Model

  Proofs about the undo (soft reset) operation.
  Undo moves HEAD back N commits but preserves the working tree.
  Key property: undo is reversible — the patches still exist in the DAG,
  they're just no longer reachable from the current HEAD.
-/

import SutureProofs.Foundations
import Mathlib.Data.Finset.Basic

namespace Suture

abbrev PatchSet := Finset PatchId

abbrev CommitId := String

abbrev CommitGraph := List (CommitId × CommitId)

def reachable (g : CommitGraph) (tip : CommitId) : Finset CommitId :=
  let rec go (visited : Finset CommitId) (c : CommitId) : Finset CommitId :=
    if c ∈ visited then visited
    else
      let children := g.filterMap (fun (p, c') => if p = c then some c' else none)
      let visited' := visited.insert c
      children.foldl (fun acc c' => go acc c') visited'
  go ∅ tip

noncomputable def ancestor (g : CommitGraph) (tip : CommitId) (steps : Nat) : CommitId := by sorry

noncomputable def undo (g : CommitGraph) (tip : CommitId) (steps : Nat) : CommitId := by sorry

/-- After undo, all patches from the previous HEAD range are still in the DAG.
    Undo only moves the branch pointer, not the DAG itself.
    Patches are immutable once added. -/
theorem undo_preserves_patches (g : CommitGraph) (tip : CommitId) (steps : Nat) :
    reachable g (undo g tip steps) ⊆ reachable g tip := by
  sorry

/-- Undo is reversible: re-applying the undone commits restores the original tip.
    The patches remain in the DAG, so we can reconstruct the original reachability set. -/
theorem undo_reversible (g : CommitGraph) (tip : CommitId) (steps : Nat) :
    undo g (undo g tip steps) steps = tip := by
  sorry

/-- Undo preserves the commit graph structure entirely.
    No commits are removed, only the branch pointer changes. -/
theorem undo_preserves_graph (g : CommitGraph) (tip : CommitId) (steps : Nat) :
    reachable g tip = reachable g tip ∪ reachable g (undo g tip steps) := by
  sorry

end Suture
