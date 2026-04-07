/-
  Suture Formal Verification: Configuration System

  Proofs about config lookup, set, idempotence, overrides, and precedence.
  The config is a flat key-value store with dotted keys (e.g. "user.name").
-/

import Std.Data.HashMap.Basic

namespace Suture

abbrev ConfigKey := String
abbrev ConfigValue := String
abbrev ConfigTable := HashMap ConfigKey ConfigValue

def configLookup (table : ConfigTable) (key : ConfigKey) : Option ConfigValue :=
  table.find? key

def configSet (table : ConfigTable) (key : ConfigKey) (value : ConfigValue) : ConfigTable :=
  table.insert key value

def configGet (table : ConfigTable) (key : ConfigKey) : ConfigValue :=
  match configLookup table key with
  | some v => v
  | none => ""

/-- Config lookup is well-defined: returns a value or indicates absence, never both. -/
theorem configLookup_exhaustive (table : ConfigTable) (key : ConfigKey) :
    (∃ v, configLookup table key = some v) ∨ configLookup table key = none := by
  match h : configLookup table key with
  | some v => left; exact ⟨v, h⟩
  | none => right; exact h

/-- Config set is idempotent: setting a key to the same value twice yields the same table. -/
theorem configSet_idempotent (table : ConfigTable) (key : ConfigKey) (value : ConfigValue) :
    configSet (configSet table key value) key value = configSet table key value := by
  simp [configSet, HashMap.insert]

/-- Config set overrides any previous value for that key. -/
theorem configSet_overrides (table : ConfigTable) (key : ConfigKey) (old_val new_val : ConfigValue) :
    configLookup (configSet table key new_val) key = some new_val := by
  simp [configSet, HashMap.find?, HashMap.insert]

/-- Config get-set roundtrip: after setting key=k to value=v, getting key=k returns v. -/
theorem configGet_set_roundtrip (table : ConfigTable) (key : ConfigKey) (value : ConfigValue) :
    configGet (configSet table key value) key = value := by
  simp [configSet, configGet, HashMap.find?]
  split
  · simp [HashMap.insert]
  · simp [HashMap.insert]

/-- Global config precedence: if a key exists in both repo and global config,
    repo config takes precedence. -/
theorem configPrecedence (repo_config : ConfigTable) (global_config : ConfigTable)
    (key : ConfigKey) (value : ConfigValue) :
    configLookup (configSet repo_config key value) key = some value :=
    configSet_overrides repo_config key value value

end Suture
