//! Patch Application Engine — reconstructs file trees from patch chains.
//!
//! This module is the bridge between the abstract patch DAG and concrete
//! filesystem state. It provides:
//! - **FileTree**: A virtual filesystem snapshot (path → CAS blob hash)
//! - **Patch application**: Transform a FileTree by applying a patch
//! - **Chain application**: Build a FileTree from root to a given patch
//! - **Diff computation**: Compare two FileTrees to find changes
//!
//! # Correctness
//!
//! Per YP-ALGEBRA-PATCH-001:
//! - Applying a chain of patches produces a deterministic file state
//! - The order of application matters (patches are NOT reordered)
//! - Each operation type (Create/Modify/Delete/Move) has well-defined semantics

#[doc(hidden)]
pub mod apply;
#[doc(hidden)]
pub mod diff;
#[doc(hidden)]
pub mod merge;
pub mod tree;

pub use apply::{ApplyError, apply_patch, apply_patch_chain, resolve_payload_to_hash};
pub use diff::{DiffEntry, DiffType, diff_trees};
pub use merge::{MergeOutput, three_way_merge_lines};
pub use tree::FileTree;
