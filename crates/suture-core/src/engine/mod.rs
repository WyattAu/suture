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

pub mod apply;
pub mod diff;
pub mod tree;

pub use apply::{apply_patch, apply_patch_chain, ApplyError};
pub use diff::{diff_trees, DiffEntry, DiffType};
pub use tree::FileTree;
