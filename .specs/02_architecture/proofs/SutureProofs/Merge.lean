/-
  THM-MERGE-001: Merge is deterministic.
  Machine-checked proofs.
-/

import SutureProofs.Foundations
import Mathlib.Data.Finset.Basic

namespace Suture

structure MergeResult' where
  common : Finset PatchId
  uniqueA : Finset PatchId
  uniqueB : Finset PatchId

def threeWayMerge (base branchA branchB : Finset PatchId) : MergeResult' where
  common := branchA ∩ branchB
  uniqueA := branchA \ base
  uniqueB := branchB \ base

/-- Merge partition is unique by construction. -/
theorem merge_partition_unique (base branchA branchB : Finset PatchId) :
    (threeWayMerge base branchA branchB).common = branchA ∩ branchB ∧
    (threeWayMerge base branchA branchB).uniqueA = branchA \ base ∧
    (threeWayMerge base branchA branchB).uniqueB = branchB \ base := by
  constructor
  · rfl
  · constructor
    · rfl
    · rfl

/-- Merge is symmetric: swapping A and B yields the same combined set. -/
theorem merge_symmetric (base branchA branchB : Finset PatchId) :
    (threeWayMerge base branchA branchB).common ∪
    (threeWayMerge base branchA branchB).uniqueA ∪
    (threeWayMerge base branchA branchB).uniqueB =
    (threeWayMerge base branchB branchA).common ∪
    (threeWayMerge base branchB branchA).uniqueA ∪
    (threeWayMerge base branchB branchA).uniqueB := by
  simp only [threeWayMerge]
  -- After simp: branchA ∩ branchB ∪ branchA \ base ∪ branchB \ base = branchB ∩ branchA ∪ branchB \ base ∪ branchA \ base
  have h_inter : branchA ∩ branchB = branchB ∩ branchA := Finset.inter_comm _ _
  rw [h_inter]
  have h_assoc : ∀ a b c : Finset PatchId, a ∪ b ∪ c = a ∪ (b ∪ c) := fun a b c => Finset.union_assoc a b c
  rw [h_assoc, h_assoc]
  congr 1
  rw [Finset.union_comm]

/-- Merge content is a subset of the inputs. -/
theorem merge_content_subset (base branchA branchB : Finset PatchId) :
    (threeWayMerge base branchA branchB).uniqueA ⊆ branchA ∧
    (threeWayMerge base branchA branchB).uniqueB ⊆ branchB := by
  simp only [threeWayMerge]
  constructor
  · intro _ h
    exact Finset.mem_sdiff.mp h |>.1
  · intro _ h
    exact Finset.mem_sdiff.mp h |>.1

/-- No spurious patches: everything in the merge result came from A or B. -/
theorem merge_no_spurious (base branchA branchB : Finset PatchId) (p : PatchId)
    (hp : p ∈ (threeWayMerge base branchA branchB).common ∪
             (threeWayMerge base branchA branchB).uniqueA ∪
             (threeWayMerge base branchA branchB).uniqueB) :
    p ∈ branchA ∨ p ∈ branchB := by
  simp only [threeWayMerge, Finset.mem_union, Finset.mem_inter, Finset.mem_sdiff] at hp
  -- hp : ((p ∈ branchA ∧ p ∈ branchB) ∨ (p ∈ branchA ∧ p ∉ base)) ∨ (p ∈ branchB ∧ p ∉ base)
  exact Or.elim hp
    (fun h : (p ∈ branchA ∧ p ∈ branchB) ∨ (p ∈ branchA ∧ p ∉ base) =>
      Or.elim h
        (fun h2 : p ∈ branchA ∧ p ∈ branchB => Or.inl h2.1)
        (fun h2 : p ∈ branchA ∧ p ∉ base => Or.inl h2.1))
    (fun h : p ∈ branchB ∧ p ∉ base => Or.inr h.1)

end Suture
