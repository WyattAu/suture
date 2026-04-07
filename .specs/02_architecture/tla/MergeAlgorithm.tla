-------------------------------- MODULE MergeAlgorithm --------------------------------
EXTENDS Naturals, FiniteSets

CONSTANT Patch          \* A patch is a record with id, touchSet, write
CONSTANT State          \* Repository state: Addr -> Data
CONSTANT Addr           \* Address space

VARIABLE repoState      \* Current repository state

\* Three-way merge partition:
\* Given base state S0, branch A state SA, branch B state SB:
\* - Common: addresses where SA[a] = SB[a] = S0[a] (unchanged by both)
\* - UniqueA: addresses where SA[a] /= S0[a] but SB[a] = S0[a] (only A changed)
\* - UniqueB: addresses where SB[a] /= S0[a] but SA[a] = S0[a] (only B changed)
\* - Conflict: addresses where SA[a] /= S0[a] and SB[a] /= S0[a] and SA[a] /= SB[a]

ThreeWayPartition(S0, SA, SB) == 
  LET Common == {a \in Addr : SA[a] = S0[a] /\ SB[a] = S0[a]}
      UniqueA == {a \in Addr : SA[a] /= S0[a] /\ SB[a] = S0[a]}
      UniqueB == {a \in Addr : SB[a] /= S0[a] /\ SA[a] = S0[a]}
      Conflict == {a \in Addr : SA[a] /= S0[a] /\ SB[a] /= S0[a] /\ SA[a] /= SB[a]}
  IN <<Common, UniqueA, UniqueB, Conflict>>

\* Merge result: common + uniqueA + uniqueB (conflicts require resolution)
MergeWithoutConflicts(S0, SA, SB) == 
  LET <<Common, UniqueA, UniqueB, Conflict>> == ThreeWayPartition(S0, SA, SB)
  IN /\ Conflict = {}  \* Precondition: no conflicts
     /\ [a \in Addr |-> 
        IF a \in Common THEN S0[a]
        ELSE IF a \in UniqueA THEN SA[a]
        ELSE IF a \in UniqueB THEN SB[a]
        ELSE S0[a]]

\* THEOREM: Partition covers all addresses
THEOREM PartitionCoversAllAddresses ==
  THEOREM S0, SA, SB :
    LET <<Common, UniqueA, UniqueB, Conflict>> == ThreeWayPartition(S0, SA, SB)
    IN Common \union UniqueA \union UniqueB \union Conflict = Addr

\* THEOREM: Partitions are disjoint
THEOREM PartitionsAreDisjoint ==
  THEOREM S0, SA, SB :
    LET <<Common, UniqueA, UniqueB, Conflict>> == ThreeWayPartition(S0, SA, SB)
    IN Common \cap UniqueA = {} /\ Common \cap UniqueB = {} /\
       Common \cap Conflict = {} /\ UniqueA \cap UniqueB = {} /\
       UniqueA \cap Conflict = {} /\ UniqueB \cap Conflict = {}

\* THEOREM: Merge is idempotent (merging a branch with itself yields no changes)
THEOREM MergeIdempotent ==
  THEOREM S :
    LET <<Common, UniqueA, UniqueB, Conflict>> == ThreeWayPartition(S, S, S)
    IN UniqueA = {} /\ UniqueB = {} /\ Conflict = {}

\* THEOREM: Merge is commutative (swapping A and B yields same result for non-conflicting merge)
THEOREM MergeCommutativeNoConflicts ==
  THEOREM S0, SA, SB :
    LET <<Common, UniqueA, UniqueB, Conflict>> == ThreeWayPartition(S0, SA, SB)
        <<Common', UniqueA', UniqueB', Conflict'>> == ThreeWayPartition(S0, SB, SA)
    IN Conflict = {} /\ Conflict' = {} =>
       Common = Common' /\ UniqueA = UniqueB' /\ UniqueB = UniqueA'

================================================================================
