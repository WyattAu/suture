//! Repository — high-level API that combines CAS, DAG, and Metadata.
//!
//! The `Repository` is the primary interface for working with a Suture
//! repository. It coordinates between the BlobStore (CAS), PatchDag,
//! and MetadataStore to provide a unified API.

pub mod repo_impl;
pub use repo_impl::{RepoError, RepoStatus, Repository};
