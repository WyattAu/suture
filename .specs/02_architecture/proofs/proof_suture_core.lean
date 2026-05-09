/-
  Formal Verification for Suture Core Patch Algebra

  Models the core commutativity and conflict properties of Suture's
  patch-based version control system.

  Properties verified:
    1. TouchSet conflict equivalence (intersection non-empty <-> conflict)
    2. Disjoint touch sets imply commutativity
    3. Conflict relation is symmetric
    4. Identity (empty) touch set commutes with any touch set
    5. Commutativity is NOT transitive (constructive counterexample)
    6. Set difference is idempotent (partition determinism)
    7. Merge conflict soundness and completeness
-/

import Mathlib.Data.Finset.Basic

namespace Suture

/-- TouchSet model: a finite set of file paths (represented as strings). --/
abbrev TouchSet := Finset String

/-- Two touch sets conflict iff their intersection is non-empty. --/
def conflicts (ts1 ts2 : TouchSet) : Prop :=
  (ts1 ∩ ts2).Nonempty

/-- Disjoint touch sets commute: if intersection is empty, no conflict. -/
theorem disjoint_commutes (ts1 ts2 : TouchSet) (h : ts1 ∩ ts2 = ∅) :
    ¬ conflicts ts1 ts2 := by
  simp [conflicts, h]

/-- Conflict relation is symmetric. -/
theorem conflict_symmetric (ts1 ts2 : TouchSet) :
    conflicts ts1 ts2 ↔ conflicts ts2 ts1 := by
  simp only [conflicts]
  constructor
  · rintro ⟨a, ha⟩
    simp only [Finset.mem_inter] at ha
    exact ⟨a, by simp only [Finset.mem_inter]; tauto⟩
  · rintro ⟨a, ha⟩
    simp only [Finset.mem_inter] at ha
    exact ⟨a, by simp only [Finset.mem_inter]; tauto⟩

/-- Identity element: empty touch set has no conflict (left).
    Since (empty ∩ ts) = empty for all ts, the intersection is never nonempty. -/
theorem identity_commutes (ts : TouchSet) :
    ¬ conflicts ∅ ts := by
  simp [conflicts, Finset.not_nonempty_iff_eq_empty]

/-- Identity element: empty touch set has no conflict (right). -/
theorem identity_commutes_right (ts : TouchSet) :
    ¬ conflicts ts ∅ := by
  simp [conflicts, Finset.not_nonempty_iff_eq_empty]

/-- Commutativity is NOT transitive.
    Counterexample: ts1 = {"A"}, ts2 = {"B"}, ts3 = {"A", "C"}.
    ts1 and ts2 are disjoint (commute), ts2 and ts3 are disjoint (commute),
    but ts1 and ts3 overlap at "A" (conflict). -/
theorem commute_not_transitive :
    ∃ (ts1 ts2 ts3 : TouchSet),
      ¬ conflicts ts1 ts2 ∧ ¬ conflicts ts2 ts3 ∧ conflicts ts1 ts3 := by
    use {"A"}, {"B"}, {"A", "C"}
    simp only [conflicts]
    have h1 : ({"A"} : Finset String) ∩ {"B"} = ∅ := by decide
    have h2 : ({"B"} : Finset String) ∩ {"A", "C"} = ∅ := by decide
    have h3 : "A" ∈ ({"A"} : Finset String) ∩ {"A", "C"} := by decide
    refine ⟨fun h => absurd h (by rw [h1]; exact Finset.not_nonempty_empty),
            fun h => absurd h (by rw [h2]; exact Finset.not_nonempty_empty),
            ⟨"A", h3⟩⟩

/-- Set difference is idempotent: (A \ B) \ B = A \ B.
    Removing B twice is the same as removing B once.
    This models the merge partition property where excluding base changes
    from branch A is deterministic regardless of intermediate steps. -/
theorem partition_deterministic (base branch_a branch_b : Finset String) :
    ((branch_a : Finset String) \ base) \ base = (branch_a : Finset String) \ base ∧
    ((branch_b : Finset String) \ base) \ base = (branch_b : Finset String) \ base := by
  constructor <;> ext x <;> simp only [Finset.mem_sdiff] <;> tauto

/-- Soundness: if two touch sets conflict, their intersection is non-empty.
    Trivially true by the definition of `conflicts`. -/
theorem merge_conflict_soundness (ts1 ts2 : TouchSet) :
    conflicts ts1 ts2 → (ts1 ∩ ts2).Nonempty := by
  exact fun h => h

/-- Completeness: if the intersection of two touch sets is non-empty, they conflict.
    Trivially true by the definition of `conflicts`. -/
theorem merge_conflict_completeness (ts1 ts2 : TouchSet) :
    (ts1 ∩ ts2).Nonempty → conflicts ts1 ts2 := by
  exact fun h => h

end Suture
