// SPDX-License-Identifier: MIT OR Apache-2.0

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

pub mod audit;
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

#[cfg(test)]
mod hash_proptests {
    use super::Hash;
    use proptest::prelude::*;
    use proptest::test_runner::TestRunner;

    #[test]
    fn hash_from_data_deterministic() {
        let mut runner = TestRunner::default();
        let strategy = proptest::collection::vec(proptest::num::u8::ANY, 0..1024usize);
        runner
            .run(&strategy, |data: Vec<u8>| {
                let h1 = Hash::from_data(&data);
                let h2 = Hash::from_data(&data);
                prop_assert_eq!(h1, h2);
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn hash_hex_roundtrip() {
        let mut runner = TestRunner::default();
        let strategy = proptest::collection::vec(proptest::num::u8::ANY, 0..1024usize);
        runner
            .run(&strategy, |data: Vec<u8>| {
                let hash = Hash::from_data(&data);
                let hex = hash.to_hex();
                prop_assert_eq!(hex.len(), 64);
                let parsed = Hash::from_hex(&hex).unwrap();
                prop_assert_eq!(hash, parsed);
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn hash_different_data_produces_different_hash() {
        let mut runner = TestRunner::default();
        let strategy = (
            proptest::collection::vec(proptest::num::u8::ANY, 1..512usize),
            0u8..255u8,
        );
        runner
            .run(&strategy, |(mut data1, extra): (Vec<u8>, u8)| {
                let mut data2 = data1.clone();
                let last = data2.last_mut().unwrap();
                *last = last.wrapping_add(1);
                data2.push(extra);
                let h1 = Hash::from_data(&data1);
                let h2 = Hash::from_data(&data2);
                prop_assert_ne!(h1, h2);
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn hash_zero_constant() {
        assert_eq!(Hash::ZERO.to_hex(), "0".repeat(64));
    }
}
