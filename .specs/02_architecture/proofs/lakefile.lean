import Lake
open Lake DSL

package suture_proofs where
  leanOptions := #[⟨`autoImplicit, false⟩]

require mathlib from git
  "https://github.com/leanprover-community/mathlib4" @ "v4.29.1"

@[default_target]
lean_lib Suture where
  roots := #[`proof_suture_core]
