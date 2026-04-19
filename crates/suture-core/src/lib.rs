//! Suture Core — The central library for the Suture Universal Semantic Version Control System.
//!
//! This crate provides:
//! - **CAS** (Content Addressable Storage): BLAKE3-indexed blob storage with Zstd compression
//! - **Patch Algebra**: Commutativity detection, merge computation, conflict handling
//! - **Patch DAG**: Directed acyclic graph of patches with branch management
//! - **Metadata Store**: SQLite-backed persistent metadata
//! - **Repository**: High-level API combining all of the above
//!
//! # Example
//!
//! ```no_run
//! use suture_core::repository::Repository;
//!
//! // Initialize a new repository
//! let mut repo = Repository::init(
//!     std::path::Path::new("my-project"),
//!     "alice",
//! )?;
//!
//! // Create a branch
//! repo.create_branch("feature", None)?;
//!
//! // Stage and commit
//! repo.add("src/main.rs")?;
//! let patch_id = repo.commit("Initial commit")?;
//!
//! // View log
//! for patch in repo.log(None)? {
//!     println!("[{}] {}", patch.id, patch.message);
//! }
//! # Ok::<(), suture_core::repository::RepoError>(())
//! ```

pub mod cas;
pub mod dag;
pub mod diff;
pub mod engine;
pub mod file_type;
pub mod hooks;
pub mod integrity;
pub mod metadata;
pub mod patch;
pub mod repository;
pub mod signing;

// Re-export common types for convenience
pub use suture_common::{BranchName, CommonError, FileStatus, Hash, RepoPath};
