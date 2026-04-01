/-
  Suture Formal Verification: Shared Foundations
-/

import Mathlib.Data.Finset.Basic

namespace Suture

abbrev Addr := String
abbrev PatchId := String
abbrev State := Addr → Option String

structure StaticPatch where
  touchSet : Finset Addr
  write : Addr → Option String

def StaticPatch.apply (p : StaticPatch) (s : State) : State :=
  fun a => if a ∈ p.touchSet then p.write a else s a

def identityPatch : StaticPatch where
  touchSet := ∅
  write := fun _ => none

@[simp] theorem identityPatch_apply (s : State) (a : Addr) :
    identityPatch.apply s a = s a := by
  unfold StaticPatch.apply identityPatch
  split_ifs
  · contradiction
  · rfl

def actualTouchSet (p : StaticPatch) (s : State) : Finset Addr :=
  Finset.filter (fun a => p.apply s a ≠ s a) p.touchSet

theorem actualTouchSet_subset (p : StaticPatch) (s : State) :
    actualTouchSet p s ⊆ p.touchSet :=
  Finset.filter_subset _ _

def compose (p1 p2 : StaticPatch) : StaticPatch where
  touchSet := p1.touchSet ∪ p2.touchSet
  write := fun a =>
    if a ∈ p2.touchSet then p2.write a
    else if a ∈ p1.touchSet then p1.write a
    else none

structure Conflict where
  patchA : StaticPatch
  patchB : StaticPatch
  conflictAddrs : Finset Addr

structure MergeResult where
  merged : Finset Addr
  conflicts : List Conflict

end Suture
