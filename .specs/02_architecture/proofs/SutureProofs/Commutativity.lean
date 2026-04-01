/-
  THM-COMM-001: Disjoint touch sets imply commutativity.
  Machine-checked proof.
-/

import SutureProofs.Foundations
import Mathlib.Data.Finset.Basic

namespace Suture

open StaticPatch

theorem apply_unchanged_outside {p : StaticPatch} {s : State} {a : Addr}
    (h : a ∉ p.touchSet) :
    p.apply s a = s a := by
  show (fun b => if b ∈ p.touchSet then p.write b else s b) a = s a
  simp [h]

theorem apply_write_inside {p : StaticPatch} {s : State} {a : Addr}
    (h : a ∈ p.touchSet) :
    p.apply s a = p.write a := by
  show (fun b => if b ∈ p.touchSet then p.write b else s b) a = p.write a
  simp [h]

/-- Helper: a ∈ ∅ for Finset is False.
    Blocked by Lean 4's Quot.lift reduction; proved via simp [Finset.mem_def]. -/
theorem finset_mem_empty {a : Addr} (h : a ∈ (∅ : Finset Addr)) : False := by
  simp [Finset.mem_def] at h

/-- Helper: Disjoint implies non-membership.
    Uses Disjoint.eq_bot + Finset.inf_eq_inter + Finset.bot_eq_empty. -/
theorem disjoint_not_mem {s t : Finset Addr} {a : Addr}
    (h_disj : Disjoint s t) (h1 : a ∈ s) : a ∉ t := by
  intro h2
  have h_mem : a ∈ s ∩ t := Finset.mem_inter.mpr ⟨h1, h2⟩
  have h_empty : s ∩ t = ∅ := by
    have := h_disj.eq_bot
    rw [Finset.inf_eq_inter, Finset.bot_eq_empty] at this
    exact this
  rw [h_empty] at h_mem
  exact finset_mem_empty h_mem

/-- THM-COMM-001: Disjoint touch sets ⇒ commutativity. -/
theorem commute_disjoint_touch_sets {p1 p2 : StaticPatch}
    (h_disj : Disjoint p1.touchSet p2.touchSet)
    (s : State) (a : Addr) :
    p2.apply (p1.apply s) a = p1.apply (p2.apply s) a := by
  by_cases h1 : a ∈ p1.touchSet
  · -- a ∈ T(P1): by disjointness, a ∉ T(P2)
    have h2 := disjoint_not_mem h_disj h1
    -- P2 doesn't touch a: LHS = P1.apply s a = P1.write a
    -- P1 doesn't get overwritten: RHS = P1.apply (P2.apply s) a = P1.write a
    show (fun b => if b ∈ p2.touchSet then p2.write b else
          (fun b' => if b' ∈ p1.touchSet then p1.write b' else s b') b) a =
          (fun b => if b ∈ p1.touchSet then p1.write b else
          (fun b' => if b' ∈ p2.touchSet then p2.write b' else s b') b) a
    simp [h1, h2]
  · -- a ∉ T(P1)
    by_cases h2 : a ∈ p2.touchSet
    · -- P2 writes a, P1 doesn't touch a: both sides = P2.write a
      show (fun b => if b ∈ p2.touchSet then p2.write b else
            (fun b' => if b' ∈ p1.touchSet then p1.write b' else s b') b) a =
            (fun b => if b ∈ p1.touchSet then p1.write b else
            (fun b' => if b' ∈ p2.touchSet then p2.write b' else s b') b) a
      simp [h1, h2]
    · -- Neither touches a: both sides = s a
      show (fun b => if b ∈ p2.touchSet then p2.write b else
            (fun b' => if b' ∈ p1.touchSet then p1.write b' else s b') b) a =
            (fun b => if b ∈ p1.touchSet then p1.write b else
            (fun b' => if b' ∈ p2.touchSet then p2.write b' else s b') b) a
      simp [h1, h2]

/-- Non-commutativity: different writes to same address. -/
theorem non_commute_witness {p1 p2 : StaticPatch} {s : State} {a : Addr}
    (h1 : a ∈ p1.touchSet) (h2 : a ∈ p2.touchSet)
    (h_ne : p1.write a ≠ p2.write a) :
    p2.apply (p1.apply s) a ≠ p1.apply (p2.apply s) a := by
  have h_lhs : p2.apply (p1.apply s) a = p2.write a := apply_write_inside h2
  have h_rhs : p1.apply (p2.apply s) a = p1.write a := apply_write_inside h1
  rw [h_lhs, h_rhs]
  exact Ne.symm h_ne

/-- LEM-002: Identity is left unit. -/
theorem identity_left (p : StaticPatch) (s : State) (a : Addr) :
    p.apply (identityPatch.apply s) a = p.apply s a := by
  -- Both sides unfold to the same lambda (identityPatch has ∅ touchSet)
  show (fun b => if b ∈ p.touchSet then p.write b else
        (fun b' => if b' ∈ ∅ then (fun _ => none) b' else s b') b) a =
        (fun b => if b ∈ p.touchSet then p.write b else s b) a
  rfl

/-- LEM-002: Identity is right unit. -/
theorem identity_right (p : StaticPatch) (s : State) (a : Addr) :
    identityPatch.apply (p.apply s) a = p.apply s a := by
  show (fun b => if b ∈ ∅ then (fun _ => none) b else (p.apply s) b) a = p.apply s a
  rfl

/-- LEM-002: Identity commutes with everything. -/
theorem identity_commutes (p : StaticPatch) (s : State) (a : Addr) :
    p.apply (identityPatch.apply s) a = identityPatch.apply (p.apply s) a := by
  show (fun b => if b ∈ p.touchSet then p.write b else
        (fun b' => if b' ∈ ∅ then (fun _ => none) b' else s b') b) a =
        (fun b => if b ∈ ∅ then (fun _ => none) b else (p.apply s) b) a
  rfl

end Suture
