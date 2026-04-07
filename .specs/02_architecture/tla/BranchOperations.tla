-------------------------------- MODULE BranchOperations --------------------------------
EXTENDS Naturals, FiniteSets, Sequences

CONSTANT Branch         \* Branch name type
CONSTANT CommitId       \* Commit hash type

VARIABLE branchTips     \* Map: Branch -> CommitId (where each branch points)
VARIABLE parentMap      \* Map: CommitId -> CommitId (commit graph)

Init == /\ branchTips = {"main" : 0}
        /\ parentMap = {}

CreateBranch(b) == /\ b \notin DOMAIN branchTips
                    /\ branchTips' = [branchTips EXCEPT ![b]]
                    /\ UNCHANGED <<parentMap>>

DeleteBranch(b) == /\ b \in DOMAIN branchTips
                    /\ b /= "main"   \* Cannot delete main
                    /\ branchTips' = [branchTips EXCEPT !.b]
                    /\ UNCHANGED <<parentMap>>

UpdateBranch(b, cid) == /\ b \in DOMAIN branchTips
                        /\ branchTips' = [branchTips EXCEPT ![b] @@ (b :> cid)]
                        /\ UNCHANGED <<parentMap>>

AddCommit(parent, child) == /\ parent \in RANGE branchTips
                          /\ child \notin DOMAIN parentMap
                          /\ parentMap' = [parentMap EXCEPT ![child] @@ (child :> parent)]

\* Check if target is ancestor of source (is source ahead of target?)
IsAncestor(ancestor, descendant) == 
  LET Reachable(c) == IF c = ancestor THEN TRUE
                      ELSE IF c \notin DOMAIN parentMap THEN FALSE
                      ELSE Reachable(parentMap[c])
  IN Reachable(descendant)

\* Fast-forward merge: source is ahead of target, just move target pointer
FastForwardMerge(source, target) == 
  /\ source \in DOMAIN branchTips
  /\ target \in DOMAIN branchTips
  /\ IsAncestor(branchTips[target], branchTips[source])
  /\ branchTips' = [branchTips EXCEPT ![target] @@ (target :> branchTips[source])]
  /\ UNCHANGED <<parentMap>>

TypeInvariant == /\ DOMAIN branchTips \subseteq Branch
                  /\ RANGE branchTips \subseteq CommitId
                  /\ DOMAIN parentMap \subseteq CommitId
                  /\ RANGE parentMap \subseteq CommitId

Next == CreateBranch("feature") \/ UpdateBranch("main", 1) \/ AddCommit(0, 1) \/
       UpdateBranch("feature", 1) \/ UpdateBranch("feature", 2) \/ AddCommit(1, 2) \/
       FastForwardMerge("feature", "main") \/ DeleteBranch("feature")

Spec == Init /\ [][Next]<>_typeInvariant
================================================================================
