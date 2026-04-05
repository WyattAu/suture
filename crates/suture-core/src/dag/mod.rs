//! Patch DAG — Directed Acyclic Graph of patches with branch management.
//!
//! The Patch-DAG is the central data structure that tracks the history of
//! a Suture repository. Every commit adds a new node to the DAG. Branches
//! are named pointers to specific nodes.
//!
//! # Invariants
//!
//! - THM-DAG-001: The DAG is always acyclic
//! - Every non-root node has at least one parent
//! - Branch names are unique

#[doc(hidden)]
pub(crate) mod branch;
pub mod graph;
#[doc(hidden)]
pub(crate) mod merge;

pub use graph::{DagError, DagNode, PatchDag};
