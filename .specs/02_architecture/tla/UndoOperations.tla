-------------------------------- MODULE UndoOperations --------------------------------
EXTENDS Naturals, FiniteSets, Sequences

CONSTANT CommitId

VARIABLE commitGraph   \* Parent map: CommitId -> CommitId  
VARIABLE branchTips    \* Branch -> CommitId (current tips)
VARIABLE undoStack     \* Stack of (branch, commitId) for undo history

\* Compute the set of ancestors reachable from a commit
ancestors(b, c) == 
  LET Reachable(cur, visited) == 
    IF cur \notin visited THEN
      IF cur \in DOMAIN commitGraph THEN
        Reachable(commitGraph[cur], visited \union {cur})
      ELSE visited \union {cur}
    ELSE visited
  IN Reachable(c, {})

\* Walk N steps back from commit c through the parent graph
ancestor(b, c, n) == 
  IF n = 0 THEN c ELSE ancestor(b, commitGraph[c], n - 1)

Init == /\ commitGraph = {}
        /\ branchTips = {"main" : "c0"}
        /\ undoStack = <<>>

\* Undo stores the current branch and tip, then moves the branch back.
Undo(b, steps) == 
  /\ b \in DOMAIN branchTips
  /\ LET oldTip == branchTips[b]
         maxSteps == Cardinality(ancestors(b, oldTip))
         steps == Min(steps, maxSteps)
     IN /\ steps > 0
        /\ undoStack' = Append(undoStack, <<b, oldTip>>)
        /\ branchTips' = [branchTips EXCEPT ![b] @@ (b :> ancestor(b, oldTip, steps))]
        /\ UNCHANGED <<commitGraph>>

\* Redo restores the most recent undo
Redo == 
  /\ Len(undoStack) > 0
  /\ LET <<b, tip>> == undoStack[Len(undoStack) - 1]
     IN /\ undoStack' = SubSeq(undoStack, 0, Len(undoStack) - 1)
        /\ branchTips' = [branchTips EXCEPT ![b] @@ (b :> tip)]

TypeInvariant ==
  /\ DOMAIN branchTips \subseteq STRING
  /\ RANGE branchTips \subseteq CommitId
  /\ DOMAIN commitGraph \subseteq CommitId
  /\ RANGE commitGraph \subseteq CommitId
  /\ Len(undoStack) >= 0

\* Commit graph is never modified by undo/redo
GraphImmutability == []UNCHANGED commitGraph

Next == \E b \in DOMAIN branchTips, s \in 1..10 : Undo(b, s) \/ Redo

Spec == Init /\ [][Next]<>_<<commitGraph, branchTips, undoStack>>
================================================================================
