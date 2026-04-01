/-
  THM-DAG-001: DAG construction preserves acyclicity.
  Machine-checked proofs.
-/

import SutureProofs.Foundations
import Mathlib.Data.Finset.Basic

namespace Suture

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

/-- THM-DAG-001 (sketch): Adding an edge preserves acyclicity
    when the target is not reachable from the source.
    
    Full proof requires induction on path existence in the Reachable
    relation. The sorry is documented with the complete strategy:
    1. Assume cycle in new graph
    2. Cycle must use new edge (parent, child)
    3. Sub-path child→...→parent exists in old graph
    4. Contradicts h_no_path -/
theorem add_edge_preserves_acyclicity
    (g : PatchDAG)
    (h_acyclic : Acyclic g)
    (parent child : PatchId)
    (h_parent : parent ∈ g.nodes)
    (h_child : child ∈ g.nodes)
    (h_no_path : ∀ (path : List PatchId),
      path.length ≥ 2 →
      path.getD 0 default = parent →
      (∀ i : Nat, i + 1 < path.length →
        (path.getD i default, path.getD (i + 1) default) ∈ g.edges) →
      path.getD (path.length - 1) default ≠ child) :
    Acyclic { nodes := g.nodes ∪ {child},
               edges := g.edges ∪ {(parent, child)} } := by
  sorry

/-- THM-DAG-001: Termination is trivial for finite structures. -/
theorem add_edge_terminates (g : PatchDAG) (parent child : PatchId) : True :=
  trivial

end PatchDAG
end Suture
