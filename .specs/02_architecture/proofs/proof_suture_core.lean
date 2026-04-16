/-
  Suture VCS — Core Algorithm Formal Proofs

  Properties proven:
  1. TouchSet algebra   — conflict iff touch set intersection
  2. Commutativity      — symmetry and disjoint-implies-commute
  3. DAG acyclicity     — adding nodes preserves acyclicity
  4. Ancestor transitivity — ancestor relation is transitive
  5. LCA properties     — reflexivity and common-ancestry
  6. Merge properties   — identity, symmetry, and clean-merge conditions

  Compilation (from .specs/02_architecture/proofs/):
    lake env lean proof_suture_core.lean

  Status: 23/24 theorems sorry-free.
  THM-DAG-ACYCLIC-001 has a sorry for path-extraction reasoning
  that requires induction on path structure.
-/

import Mathlib.Data.Finset.Basic
import Mathlib.Data.Set.Basic
import Mathlib.Data.Fintype.Basic

namespace Suture

abbrev Addr := String
abbrev PatchId := String
abbrev State := Addr → Option String

-- ── 1. TouchSet Algebra ─────────────────────────────────────────

structure StaticPatch where
  touchSet : Finset Addr
  write : Addr → Option String

namespace StaticPatch

def apply (p : StaticPatch) (s : State) : State :=
  fun a => if a ∈ p.touchSet then p.write a else s a

@[simp] theorem apply_mem {p : StaticPatch} {s : State} {a : Addr}
    (h : a ∈ p.touchSet) : p.apply s a = p.write a := by
  unfold apply; simp [h]

@[simp] theorem apply_nmem {p : StaticPatch} {s : State} {a : Addr}
    (h : a ∉ p.touchSet) : p.apply s a = s a := by
  unfold apply; simp [h]

end StaticPatch

def patchesConflict (p1 p2 : StaticPatch) : Prop :=
  ¬Disjoint p1.touchSet p2.touchSet

theorem conflict_iff_touch_intersect (p1 p2 : StaticPatch) :
    patchesConflict p1 p2 ↔ p1.touchSet ∩ p2.touchSet ≠ ∅ := by
  simp only [patchesConflict, disjoint_iff, Finset.inf_eq_inter, Finset.bot_eq_empty]

theorem conflict_symmetric (p1 p2 : StaticPatch) :
    patchesConflict p1 p2 ↔ patchesConflict p2 p1 := by
  simp [patchesConflict, disjoint_comm]

theorem disjoint_no_conflict (p1 p2 : StaticPatch)
    (h : Disjoint p1.touchSet p2.touchSet) :
    ¬patchesConflict p1 p2 := by
  simp [patchesConflict, h]

-- ── 2. Commutativity ────────────────────────────────────────────

theorem disjoint_not_mem {s t : Finset Addr} {a : Addr}
    (h_disj : Disjoint s t) (h1 : a ∈ s) : a ∉ t := by
  intro h2
  have h_mem : a ∈ s ∩ t := Finset.mem_inter.mpr ⟨h1, h2⟩
  have h_empty : s ∩ t = ∅ := by
    have := Disjoint.eq_bot h_disj
    rw [Finset.inf_eq_inter, Finset.bot_eq_empty] at this
    exact this
  rw [h_empty] at h_mem
  simp [Finset.mem_def] at h_mem

theorem commute_symmetric {p1 p2 : StaticPatch}
    (h : ∀ s a, p2.apply (p1.apply s) a = p1.apply (p2.apply s) a) :
    ∀ s a, p1.apply (p2.apply s) a = p2.apply (p1.apply s) a :=
  fun s a => Eq.symm (h s a)

theorem commute_disjoint_touch_sets {p1 p2 : StaticPatch}
    (h_disj : Disjoint p1.touchSet p2.touchSet)
    (s : State) (a : Addr) :
    p2.apply (p1.apply s) a = p1.apply (p2.apply s) a := by
  by_cases h1 : a ∈ p1.touchSet
  · have h2 := disjoint_not_mem h_disj h1
    simp [StaticPatch.apply, h1, h2]
  · by_cases h2 : a ∈ p2.touchSet
    · simp [StaticPatch.apply, h1, h2]
    · simp [StaticPatch.apply, h1, h2]

theorem non_commute_implies_overlap {p1 p2 : StaticPatch}
    (h : ∃ s a, p2.apply (p1.apply s) a ≠ p1.apply (p2.apply s) a) :
    ¬Disjoint p1.touchSet p2.touchSet := by
  contrapose! h
  intro s a
  exact commute_disjoint_touch_sets h s a

def identityPatch : StaticPatch where
  touchSet := ∅
  write := fun _ => none

theorem identity_commutes (p : StaticPatch) (s : State) (a : Addr) :
    p.apply (identityPatch.apply s) a = identityPatch.apply (p.apply s) a := by rfl

theorem identity_left_unit (p : StaticPatch) (s : State) (a : Addr) :
    p.apply (identityPatch.apply s) a = p.apply s a := by rfl

theorem identity_right_unit (p : StaticPatch) (s : State) (a : Addr) :
    identityPatch.apply (p.apply s) a = p.apply s a := by rfl

-- ── 3. DAG Acyclicity ───────────────────────────────────────────

structure PatchDAG where
  nodes : Finset PatchId
  edges : Finset (PatchId × PatchId)

namespace PatchDAG

def NoSelfLoops (g : PatchDAG) : Prop :=
  ∀ a, (a, a) ∉ g.edges

def Acyclic (g : PatchDAG) : Prop :=
  ∀ (path : List PatchId),
    path.length ≥ 2 →
    (∀ i : Nat, i + 1 < path.length →
      (path.getD i default, path.getD (i + 1) default) ∈ g.edges) →
    path.getD 0 default ≠ path.getD (path.length - 1) default

inductive Reachable (g : PatchDAG) : PatchId → PatchId → Prop where
  | refl (a : PatchId) : a ∈ g.nodes → Reachable g a a
  | step {a b c : PatchId} :
      (a, b) ∈ g.edges →
      Reachable g b c →
      Reachable g a c

/-- THM-DAG-ACYCLIC-001: Adding an edge preserves acyclicity
  when the target is not reachable from the source.

  The sorry is for extracting a child→parent path from the cycle
  after removing the new edge (parent, child). This requires
  induction on path structure to decompose the cycle into
  a prefix and suffix and show the suffix forms a valid path
  from child to parent in the old graph. -/
theorem add_edge_preserves_acyclicity
    (g : PatchDAG)
    (h_acyclic : Acyclic g)
    (parent child : PatchId)
    (h_parent : parent ∈ g.nodes)
    (h_child : child ∈ g.nodes)
    (h_no_path : ¬Reachable g child parent) :
    Acyclic { nodes := g.nodes ∪ {child},
               edges := g.edges ∪ ({(parent, child)} : Finset (PatchId × PatchId)) } := by
  intro path h_len h_edges
  by_contra h_cycle
  have h_head : path.getD 0 default = path.getD (path.length - 1) default := h_cycle
  have h_all_old : ∀ i : Nat, i + 1 < path.length →
      (path.getD i default, path.getD (i + 1) default) ∈ g.edges := by
    intro i hi
    have h_edge := h_edges i hi
    by_contra h_not_old
    have h_edge2 := h_edges i hi
    simp only [Finset.mem_union] at h_edge2
    have : (path.getD i default, path.getD (i + 1) default) ∈ ({(parent, child)} : Finset (PatchId × PatchId)) := by
      match h_edge2 with
      | Or.inl h => exact absurd h h_not_old
      | Or.inr h => exact h
    have h_eq : (path.getD i default, path.getD (i + 1) default) = (parent, child) :=
      Finset.mem_singleton.mp this
    have h_reachable : Reachable g child parent := by
      have : path.getD (i + 1) default = child := by
        cases h_eq; rfl
      have : path.getD i default = parent := by
        cases h_eq; rfl
      sorry
    exact h_no_path h_reachable
  exact h_acyclic path h_len h_all_old h_head

theorem add_node_preserves_acyclicity
    (g : PatchDAG)
    (h_acyclic : Acyclic g)
    (newNode : PatchId) :
    Acyclic { nodes := g.nodes ∪ {newNode}, edges := g.edges } := by
  intro path h_len h_edges
  by_contra h_cycle
  have h_head : path.getD 0 default = path.getD (path.length - 1) default := h_cycle
  have h_edges_in_g : ∀ i : Nat, i + 1 < path.length →
      (path.getD i default, path.getD (i + 1) default) ∈ g.edges := by
    intro i hi
    exact h_edges i hi
  exact h_acyclic path h_len h_edges_in_g h_head

end PatchDAG

-- ── 4. Ancestor Transitivity ────────────────────────────────────

def ancestors (g : PatchDAG) (id : PatchId) : PatchId → Prop :=
  fun a => PatchDAG.Reachable g a id

theorem reachable_transitive
    (g : PatchDAG)
    (a b c : PatchId)
    (h1 : PatchDAG.Reachable g a b)
    (h2 : PatchDAG.Reachable g b c) :
    PatchDAG.Reachable g a c := by
  induction h1 with
  | refl _ _ => exact h2
  | step => rename_i _ _ _ he hr ih; exact PatchDAG.Reachable.step he (ih h2)

theorem ancestors_transitive
    (g : PatchDAG)
    (a b c : PatchId)
    (h1 : ancestors g b a)
    (h2 : ancestors g c b) :
    ancestors g c a := by
  unfold ancestors at h1 h2 ⊢
  exact reachable_transitive g a b c h1 h2

theorem ancestors_refl
    (g : PatchDAG)
    (a : PatchId)
    (h : a ∈ g.nodes) :
    ancestors g a a := by
  unfold ancestors
  exact PatchDAG.Reachable.refl a h

-- ── 5. LCA Properties ───────────────────────────────────────────

structure GenerationMap where
  gen : PatchId → Nat
  root : PatchId
  rootGenZero : gen root = 0

namespace PatchDAG

def commonAncestors (g : PatchDAG) (a b : PatchId) : PatchId → Prop :=
  fun x => Reachable g x a ∧ Reachable g x b

theorem lca_common_ancestors_nonempty
    (g : PatchDAG)
    (a : PatchId)
    (ha : a ∈ g.nodes) :
    ∃ x, x ∈ g.nodes ∧ commonAncestors g a a x := by
  use a
  exact ⟨ha, Reachable.refl a ha, Reachable.refl a ha⟩

theorem lca_refl
    (g : PatchDAG)
    (a : PatchId)
    (ha : a ∈ g.nodes) :
    a ∈ g.nodes ∧ commonAncestors g a a a := by
  exact ⟨ha, Reachable.refl a ha, Reachable.refl a ha⟩

theorem common_ancestor_reaches_both
    (g : PatchDAG)
    (a b l : PatchId)
    (h : commonAncestors g a b l) :
    Reachable g l a ∧ Reachable g l b := h

theorem common_ancestor_is_ancestor
    (g : PatchDAG)
    (a b l : PatchId)
    (h : commonAncestors g a b l) :
    ancestors g a l ∧ ancestors g b l := by
  unfold ancestors; exact h

theorem common_ancestors_symmetric
    (g : PatchDAG)
    (a b : PatchId) (x : PatchId) :
    commonAncestors g a b x ↔ commonAncestors g b a x := by
  unfold commonAncestors; exact And.comm

end PatchDAG

-- ── 6. Merge Properties ─────────────────────────────────────────

structure ThreeWayMerge where
  common : Finset PatchId
  oursOnly : Finset PatchId
  theirsOnly : Finset PatchId

def threeWayMerge (base ours theirs : Finset PatchId) : ThreeWayMerge where
  common := ours ∩ theirs
  oursOnly := ours \ base
  theirsOnly := theirs \ base

theorem merge_base_equals_theirs (base ours : Finset PatchId) :
    (threeWayMerge base ours base).common ∪
    (threeWayMerge base ours base).oursOnly ∪
    (threeWayMerge base ours base).theirsOnly =
    ours := by
  simp only [threeWayMerge]
  ext a
  simp only [Finset.mem_union, Finset.mem_inter, Finset.mem_sdiff]
  constructor
  · intro h
    match h with
    | Or.inl (Or.inl h) => exact h.1
    | Or.inl (Or.inr h) => exact h.1
    | Or.inr h => exact False.elim (h.2 h.1)
  · intro h
    by_cases hb : a ∈ base
    · exact Or.inl (Or.inl ⟨h, hb⟩)
    · exact Or.inl (Or.inr ⟨h, hb⟩)

theorem merge_base_equals_ours (base theirs : Finset PatchId) :
    (threeWayMerge base base theirs).common ∪
    (threeWayMerge base base theirs).oursOnly ∪
    (threeWayMerge base base theirs).theirsOnly =
    theirs := by
  simp only [threeWayMerge]
  ext a
  simp only [Finset.mem_union, Finset.mem_inter, Finset.mem_sdiff]
  constructor
  · intro h
    match h with
    | Or.inl (Or.inl h) => exact h.2
    | Or.inl (Or.inr h) => exact False.elim (h.2 h.1)
    | Or.inr h => exact h.1
  · intro h
    by_cases hb : a ∈ base
    · exact Or.inl (Or.inl ⟨hb, h⟩)
    · exact Or.inr ⟨h, hb⟩

theorem merge_both_same (base x : Finset PatchId) :
    (threeWayMerge base x x).common ∪
    (threeWayMerge base x x).oursOnly ∪
    (threeWayMerge base x x).theirsOnly =
    x := by
  simp only [threeWayMerge]
  ext a
  simp only [Finset.mem_union, Finset.mem_inter, Finset.mem_sdiff]
  constructor
  · intro h
    match h with
    | Or.inl (Or.inl h) => exact h.1
    | Or.inl (Or.inr h) => exact h.1
    | Or.inr h => exact h.1
  · intro h
    by_cases hb : a ∈ base
    · exact Or.inl (Or.inl ⟨h, h⟩)
    · exact Or.inl (Or.inr ⟨h, hb⟩)

theorem merge_symmetric (base ours theirs : Finset PatchId) :
    (threeWayMerge base ours theirs).common ∪
    (threeWayMerge base ours theirs).oursOnly ∪
    (threeWayMerge base ours theirs).theirsOnly =
    (threeWayMerge base theirs ours).common ∪
    (threeWayMerge base theirs ours).oursOnly ∪
    (threeWayMerge base theirs ours).theirsOnly := by
  simp only [threeWayMerge]
  ext a
  simp only [Finset.mem_union, Finset.mem_inter, Finset.mem_sdiff]
  constructor
  · intro h
    match h with
    | Or.inl (Or.inl h) => exact Or.inl (Or.inl ⟨h.2, h.1⟩)
    | Or.inl (Or.inr h) => exact Or.inr ⟨h.1, h.2⟩
    | Or.inr h => exact Or.inl (Or.inr ⟨h.1, h.2⟩)
  · intro h
    match h with
    | Or.inl (Or.inl h) => exact Or.inl (Or.inl ⟨h.2, h.1⟩)
    | Or.inl (Or.inr h) => exact Or.inr ⟨h.1, h.2⟩
    | Or.inr h => exact Or.inl (Or.inr ⟨h.1, h.2⟩)

theorem merge_no_spurious (base ours theirs : Finset PatchId) (p : PatchId)
    (hp : p ∈ (threeWayMerge base ours theirs).common ∪
             (threeWayMerge base ours theirs).oursOnly ∪
             (threeWayMerge base ours theirs).theirsOnly) :
    p ∈ ours ∨ p ∈ theirs := by
  simp only [threeWayMerge, Finset.mem_union, Finset.mem_inter, Finset.mem_sdiff] at hp
  match hp with
  | Or.inl (Or.inl h) => exact Or.inl h.1
  | Or.inl (Or.inr h) => exact Or.inl h.1
  | Or.inr h => exact Or.inr h.1

theorem merge_content_subset (base ours theirs : Finset PatchId) :
    (threeWayMerge base ours theirs).oursOnly ⊆ ours ∧
    (threeWayMerge base ours theirs).theirsOnly ⊆ theirs := by
  simp only [threeWayMerge]
  exact ⟨Finset.sdiff_subset, Finset.sdiff_subset⟩

structure PatchWithTouch where
  id : PatchId
  touchSet : Finset Addr
  deriving DecidableEq

def mergeIsClean
    (base ours theirs : Finset PatchWithTouch) : Prop :=
  ∀ p₁ ∈ ours, ∀ p₂ ∈ theirs,
    p₁ ∉ base → p₂ ∉ base →
    Disjoint p₁.touchSet p₂.touchSet

theorem merge_clean_symmetric
    (base ours theirs : Finset PatchWithTouch) :
    mergeIsClean base ours theirs ↔ mergeIsClean base theirs ours := by
  constructor
  · intro h p₁ hp₁ p₂ hp₂ hnb₁ hnb₂
    exact Disjoint.symm (h p₂ hp₂ p₁ hp₁ hnb₂ hnb₁)
  · intro h p₁ hp₁ p₂ hp₂ hnb₁ hnb₂
    exact Disjoint.symm (h p₂ hp₂ p₁ hp₁ hnb₂ hnb₁)

theorem merge_clean_ours_subset_base
    (base ours theirs : Finset PatchWithTouch)
    (h_sub : ours ⊆ base) :
    mergeIsClean base ours theirs := by
  unfold mergeIsClean
  intro p₁ hp₁ p₂ hp₂ hnb₁ hnb₂
  exact absurd hnb₁ (fun h => h (h_sub hp₁))

theorem merge_clean_theirs_subset_base
    (base ours theirs : Finset PatchWithTouch)
    (h_sub : theirs ⊆ base) :
    mergeIsClean base ours theirs := by
  unfold mergeIsClean
  intro p₁ hp₁ p₂ hp₂ hnb₁ hnb₂
  exact absurd hnb₂ (fun h => h (h_sub hp₂))

theorem disjoint_touch_sets_clean_merge
    (base ours theirs : Finset PatchWithTouch)
    (h : ∀ p₁ ∈ ours, ∀ p₂ ∈ theirs,
      p₁ ∉ base → p₂ ∉ base →
      Disjoint p₁.touchSet p₂.touchSet) :
    mergeIsClean base ours theirs := h

end Suture
