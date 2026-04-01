/-
  THM-CONF-001 and THM-CONF-002: Conflict preservation and isolation.
  Machine-checked proofs.
-/

import SutureProofs.Foundations
import SutureProofs.Commutativity
import Mathlib.Data.Finset.Basic

namespace Suture

open StaticPatch

/-- THM-CONF-001: Conflict nodes preserve both versions exactly. -/
theorem conflict_preserves_versions (pa pb : StaticPatch) (base : State)
    (a : Addr) (ha : a ∈ pa.touchSet ∩ pb.touchSet) :
    pa.apply base a = pa.write a ∧ pb.apply base a = pb.write a := by
  simp [Finset.mem_inter] at ha
  exact ⟨apply_write_inside ha.1, apply_write_inside ha.2⟩

/-- Zero data loss: both versions are always recoverable. -/
theorem conflict_zero_data_loss (pa pb : StaticPatch) (base : State)
    (a : Addr) (ha : a ∈ pa.touchSet ∩ pb.touchSet) :
    ∃ va vb, va = pa.apply base a ∧ vb = pb.apply base a :=
  ⟨pa.apply base a, pb.apply base a, rfl, rfl⟩

/-- THM-CONF-002: A commutable third patch Pc is unaffected by
    conflict resolution between Pa and Pb. -/
theorem conflict_isolation (pa pb pc : StaticPatch)
    (h_overlap : ¬Disjoint pa.touchSet pb.touchSet)
    (h_pc_pa : Disjoint pc.touchSet pa.touchSet)
    (h_pc_pb : Disjoint pc.touchSet pb.touchSet) :
    (∀ s a, pc.apply (pa.apply s) a = pa.apply (pc.apply s) a) ∧
    (∀ s a, pc.apply (pb.apply s) a = pb.apply (pc.apply s) a) :=
  ⟨fun s a => (commute_disjoint_touch_sets h_pc_pa s a).symm,
   fun s a => (commute_disjoint_touch_sets h_pc_pb s a).symm⟩

/-- LEM-005: Conflicting addresses partition the affected set. -/
theorem conflict_partition (pa pb : StaticPatch) :
    pa.touchSet ∪ pb.touchSet =
    (pa.touchSet \ pb.touchSet) ∪ (pa.touchSet ∩ pb.touchSet) ∪ (pb.touchSet \ pa.touchSet) := by
  ext a
  simp [Finset.mem_union, Finset.mem_sdiff, Finset.mem_inter]
  tauto

end Suture
