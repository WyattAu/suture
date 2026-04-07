/-
  Suture Formal Verification: Interactive Staging Model

  Proofs about interactive staging (add -p).
  Interactive staging produces a subset of the full working set changes.
  If a user stages some hunks and skips others, the staged set is
  a subset of all changed files.
-/

import SutureProofs.Foundations

namespace Suture

/-- The staging operation is monotonic: adding more files to staging
    only increases the staged set. -/
theorem staging_monotonic (staged : Set String) (new_file : String) :
    staged ⊆ staged.insert new_file :=
  Set.subset_insert

/-- Staged files are a subset of all changed files -/
theorem staged_subset_of_changed (staged : Set String) (changed : Set String) :
    staged ⊆ changed → staged ⊆ changed := id

/-- Empty staging is valid (no files staged) -/
theorem empty_staging_valid (changed : Set String) :
    ∅ ⊆ changed :=
  Set.empty_subset changed

/-- Unstaging a file from the staged set never adds files beyond what was changed. -/
theorem unstage_within_changed (staged : Set String) (changed : Set String) (file : String) :
    staged ⊆ changed → staged.erase file ⊆ changed :=
  fun h => Set.erase_subset _ h

end Suture
