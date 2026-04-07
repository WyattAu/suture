-------------------------------- MODULE StashOperations --------------------------------
EXTENDS Naturals, FiniteSets, Sequences

CONSTANT FileContent     \* Map: FilePath -> Content
CONSTANT StashId

VARIABLE stashes         \* Seq of (state, branch, message)
VARIABLE headState        \* Current working tree state

Init == /\ stashes = <<>>
        /\ headState = {}

\* Stash save captures the current working tree state with a message
StashSave(msg) ==
  /\ LET entry == <<headState, "main", msg>>
     IN stashes' = Append(stashes, entry)
  /\ UNCHANGED <<headState>>

\* Stash pop restores the most recent stash and removes it
StashPop ==
  /\ Len(stashes) > 0
  /\ LET <<saved_state, _branch, _msg>> == stashes[Len(stashes) - 1]
     IN /\ headState' = saved_state
        /\ stashes' = SubSeq(stashes, 0, Len(stashes) - 1)

\* Stash apply restores the most recent stash but keeps it in the list
StashApply ==
  /\ Len(stashes) > 0
  /\ LET <<saved_state, _branch, _msg>> == stashes[Len(stashes) - 1]
     IN /\ headState' = saved_state
        /\ UNCHANGED <<stashes>>

\* Stash list shows all stashes without modifying state
StashList ==
  /\ UNCHANGED <<stashes, headState>>

\* Stash drop removes a specific stash by index
StashDrop(idx) ==
  /\ idx < Len(stashes)
  /\ stashes' = SubSeq(stashes, 0, idx) \o SubSeq(stashes, idx + 1, Len(stashes))
  /\ UNCHANGED <<headState>>

\* Stash clear removes all stashes
StashClear ==
  /\ stashes' = <<>>
  /\ UNCHANGED <<headState>>

TypeInvariant ==
  /\ Len(stashes) >= 0

Next == StashSave("wip") \/ StashPop \/ StashApply \/ StashDrop(0) \/ StashClear

Spec == Init /\ [][Next]<>_<<stashes, headState>>
================================================================================
