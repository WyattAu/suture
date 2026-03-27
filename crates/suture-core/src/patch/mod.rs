//! Patch Algebra — the mathematical heart of Suture.
//!
//! This module implements the core patch theory:
//! - **Patches** as typed operations with touch sets
//! - **Commutativity** detection based on disjoint touch sets
//! - **Merge** computation via set-union of independent patches
//! - **Conflict** detection and first-class conflict nodes
//!
//! # Formal Foundation
//!
//! See YP-ALGEBRA-PATCH-001 for the mathematical proofs:
//! - THM-COMM-001: Disjoint touch sets imply commutativity
//! - THM-MERGE-001: Merging produces a deterministic, unique result
//! - THM-CONF-001: Conflict nodes preserve all information from both branches

pub mod commute;
pub mod conflict;
pub mod merge;
pub mod types;

pub use commute::{commute, CommuteResult};
pub use conflict::{Conflict, ConflictNode};
pub use merge::{merge, MergeError, MergeResult};
pub use types::{Operation, OperationType, Patch, PatchId, TouchSet};
