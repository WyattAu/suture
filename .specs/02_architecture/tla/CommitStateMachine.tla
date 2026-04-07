-------------------------------- MODULE CommitStateMachine --------------------------------
EXTENDS Naturals, Sequences, FiniteSets
CONSTANT Repo          \* The repository state
CONSTANT Files         \* Set of file paths in the repo

VARIABLE state         \* Current file tree state: Files -> Data
VARIABLE branches      \* Set of branch names
VARIABLE head          \* Current branch name
VARIABLE log           \* Sequence of commits: each commit is [branch, parent, files_changed]
VARIABLE staged        \* Set of staged file paths

Init == /\ state = [f \in Files |-> ""]
        /\ branches = {"main"}
        /\ head = "main"
        /\ log = << >>
        /\ staged = {}

TypeInvariant == /\ state \in [Files -> STRING]
                  /\ branches \subseteq STRING
                  /\ head \in branches
                  /\ log \in Seq(STRING \X STRING \X SUBSET(Files))

StageFile(f) == /\ f \in Files
                /\ UNCHANGED <<state, branches, head, log, staged>>
                /\ staged' = staged \union {f}

Commit(msg) == /\ staged \subseteq DOMAIN state
               /\ LET newLog == Append(log, <<head, Len(log), staged>>)
                  IN /\ UNCHANGED <<state, branches, head>>
                     /\ log' = newLog
                     /\ staged' = {}

CreateBranch(b) == /\ b \notin branches
                     /\ UNCHANGED <<state, log, staged>>
                     /\ branches' = branches \union {b}

Checkout(b) == /\ b \in branches
               /\ UNCHANGED <<state, branches, log, staged>>
               /\ head' = b

Next == StageFile("f1") \/ StageFile("f2") \/ Commit("msg") 
       \/ CreateBranch("feature") \/ Checkout("feature") \/ Checkout("main")

Spec == Init /\ [][Next]<>_typeInvariant
================================================================================
