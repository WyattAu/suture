/-
  THM-COMPOSE-001 and THM-PATCH-001: Composition and Monoid structure.
  Machine-checked proofs.
-/

import SutureProofs.Foundations
import SutureProofs.Commutativity
import Mathlib.Data.Finset.Basic

namespace Suture

open StaticPatch

@[simp] theorem compose_touchSet (p1 p2 : StaticPatch) :
    (compose p1 p2).touchSet = p1.touchSet ∪ p2.touchSet := rfl

/-- Compose two patches: (P2 ∘ P1).apply = P2.apply ∘ P1.apply. -/
theorem compose_correctness (p1 p2 : StaticPatch) (s : State) (a : Addr) :
    (compose p1 p2).apply s a = p2.apply (p1.apply s) a := by
  show (fun b => if b ∈ p1.touchSet ∪ p2.touchSet then
        (fun a' => if a' ∈ p2.touchSet then p2.write a'
        else if a' ∈ p1.touchSet then p1.write a' else none) b
        else s b) a =
        (fun b => if b ∈ p2.touchSet then p2.write b else
        (fun b' => if b' ∈ p1.touchSet then p1.write b' else s b') b) a
  by_cases h1 : a ∈ p1.touchSet <;> by_cases h2 : a ∈ p2.touchSet
  · simp [h1, h2]
  · simp [h1, h2]
  · simp [h1, h2]
  · simp [h1, h2]

/-- Composition is associative. -/
theorem compose_associative (p1 p2 p3 : StaticPatch) (s : State) (a : Addr) :
    (compose (compose p1 p2) p3).apply s a =
    (compose p1 (compose p2 p3)).apply s a := by
  by_cases h3 : a ∈ p3.touchSet
  · simp [compose, StaticPatch.apply, h3]
  · by_cases h2 : a ∈ p2.touchSet
    · simp [compose, StaticPatch.apply, h3, h2]
    · by_cases h1 : a ∈ p1.touchSet
      · simp [compose, StaticPatch.apply, h3, h2, h1]
      · simp [compose, StaticPatch.apply, h3, h2, h1]

/-- Identity is left unit for composition. -/
theorem compose_identity_left (p : StaticPatch) (s : State) (a : Addr) :
    (compose identityPatch p).apply s a = p.apply s a := by
  simp [identityPatch, compose, StaticPatch.apply]
  split_ifs <;> rfl

/-- Identity is right unit for composition. -/
theorem compose_identity_right (p : StaticPatch) (s : State) (a : Addr) :
    (compose p identityPatch).apply s a = p.apply s a := by
  simp [identityPatch, compose, StaticPatch.apply]
  split_ifs <;> rfl

/-- Composition order is irrelevant when touch sets are disjoint. -/
theorem compose_order_independent {p1 p2 : StaticPatch}
    (h : Disjoint p1.touchSet p2.touchSet)
    (s : State) (a : Addr) :
    (compose p1 p2).apply s a = (compose p2 p1).apply s a := by
  by_cases h1 : a ∈ p1.touchSet <;> by_cases h2 : a ∈ p2.touchSet
  · -- Both: contradiction with disjoint
    exfalso
    exact disjoint_not_mem h h1 h2
  · -- P1 only: LHS = P1.write, RHS needs P2 first (no effect), then P1
    show (fun b => if b ∈ p1.touchSet ∪ p2.touchSet then
          (fun a' => if a' ∈ p2.touchSet then p2.write a'
          else if a' ∈ p1.touchSet then p1.write a' else none) b
          else s b) a =
          (fun b => if b ∈ p2.touchSet ∪ p1.touchSet then
          (fun a' => if a' ∈ p1.touchSet then p1.write a'
          else if a' ∈ p2.touchSet then p2.write a' else none) b
          else s b) a
    simp [h1, h2]
  · -- P2 only
    show (fun b => if b ∈ p1.touchSet ∪ p2.touchSet then
          (fun a' => if a' ∈ p2.touchSet then p2.write a'
          else if a' ∈ p1.touchSet then p1.write a' else none) b
          else s b) a =
          (fun b => if b ∈ p2.touchSet ∪ p1.touchSet then
          (fun a' => if a' ∈ p1.touchSet then p1.write a'
          else if a' ∈ p2.touchSet then p2.write a' else none) b
          else s b) a
    simp [h1, h2]
  · -- Neither: both = s a
    show (fun b => if b ∈ p1.touchSet ∪ p2.touchSet then
          (fun a' => if a' ∈ p2.touchSet then p2.write a'
          else if a' ∈ p1.touchSet then p1.write a' else none) b
          else s b) a =
          (fun b => if b ∈ p2.touchSet ∪ p1.touchSet then
          (fun a' => if a' ∈ p1.touchSet then p1.write a'
          else if a' ∈ p2.touchSet then p2.write a' else none) b
          else s b) a
    simp [h1, h2]

end Suture
