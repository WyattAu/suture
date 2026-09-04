// SPDX-License-Identifier: MIT OR Apache-2.0

//! Content Addressable Storage (CAS) — BLAKE3-indexed blob store with Zstd compression.
//!
//! The CAS is the foundational storage layer of Suture. Every piece of data —
//! file content, patch payloads, metadata — is stored as a blob indexed by its
//! BLAKE3 hash. Identical blobs are deduplicated automatically.
//!
//! # Implementation
//!
//! The storage implementation is delegated to the [`cas_kit`] crate; this
//! module is a compatibility facade preserving the historical
//! `suture_core::cas` API (in particular, everything is expressed in terms of
//! [`suture_common::Hash`]). Conversions between `suture_common::Hash` and
//! `cas_kit::Hash` happen at this boundary via their identical `pub [u8; 32]`
//! representation.
//!
//! # On-Disk Layout
//!
//! ```text
//! .suture/
//!   objects/
//!     ab/           # First 2 hex chars of hash (256 buckets)
//!       cdef...     # Remaining 62 hex chars = blob filename
//!     pack/         # Packfiles bundling many small blobs
//!   metadata.db     # SQLite database (handled by metadata module)
//! ```
//!
//! # Correctness Properties
//!
//! - **Integrity**: `get(H(data)) == data` (BLAKE3 collision resistance)
//! - **Deduplication**: Storing the same blob twice uses one copy
//! - **Lossless**: Zstd compression/decompression is lossless

/// Blob store and error types, delegating to `cas_kit::store`.
pub mod store {
    use std::path::PathBuf;
    use suture_common::Hash;

    /// Errors that can occur during CAS operations.
    pub type CasError = cas_kit::CasError;

    /// The Content Addressable Storage blob store.
    ///
    /// Stores blobs indexed by BLAKE3 hash on the local filesystem.
    /// Provides deduplication, optional compression, and integrity verification.
    ///
    /// # Thread Safety
    ///
    /// `BlobStore` is `Send + Sync` and can be shared across threads via `Arc`.
    pub struct BlobStore {
        inner: cas_kit::BlobStore,
    }

    impl BlobStore {
        /// Create a new BlobStore rooted at the given directory.
        ///
        /// Creates the `objects/` subdirectory if it doesn't exist.
        pub fn new(root: impl Into<PathBuf>) -> Result<Self, CasError> {
            Ok(Self {
                inner: cas_kit::BlobStore::new(root)?,
            })
        }

        /// Create a new uncompressed BlobStore rooted at the given directory.
        pub fn new_uncompressed(root: impl Into<PathBuf>) -> Result<Self, CasError> {
            Ok(Self {
                inner: cas_kit::BlobStore::new_uncompressed(root)?,
            })
        }

        /// Enable or disable hash verification on every read.
        pub fn set_verify_on_read(&mut self, verify: bool) {
            self.inner.set_verify_on_read(verify);
        }

        /// Whether reads verify the blob hash against its address.
        pub fn verify_on_read(&self) -> bool {
            self.inner.verify_on_read()
        }

        /// Store a blob, returning its content hash. Deduplicates.
        pub fn put_blob(&self, data: &[u8]) -> Result<Hash, CasError> {
            self.inner.put_blob(data).map(|h| Hash(h.0))
        }

        /// Store a blob, failing if it already exists.
        pub fn put_blob_new(&self, data: &[u8]) -> Result<Hash, CasError> {
            self.inner.put_blob_new(data).map(|h| Hash(h.0))
        }

        /// Store a blob only if its content hashes to `expected_hash`.
        pub fn put_blob_with_hash(
            &self,
            data: &[u8],
            expected_hash: &Hash,
        ) -> Result<(), CasError> {
            self.inner
                .put_blob_with_hash(data, &cas_kit::Hash(expected_hash.0))
        }

        /// Read a blob by hash.
        pub fn get_blob(&self, hash: &Hash) -> Result<Vec<u8>, CasError> {
            self.inner.get_blob(&cas_kit::Hash(hash.0))
        }

        /// Whether the blob exists (loose or packed).
        pub fn has_blob(&self, hash: &Hash) -> bool {
            self.inner.has_blob(&cas_kit::Hash(hash.0))
        }

        /// Delete a loose blob by hash.
        pub fn delete_blob(&self, hash: &Hash) -> Result<(), CasError> {
            self.inner.delete_blob(&cas_kit::Hash(hash.0))
        }

        /// Number of loose blobs in the store.
        pub fn blob_count(&self) -> Result<u64, CasError> {
            self.inner.blob_count()
        }

        /// Total size in bytes of all loose blobs.
        pub fn total_size(&self) -> Result<u64, CasError> {
            self.inner.total_size()
        }

        /// List the hashes of all loose blobs.
        pub fn list_blobs(&self) -> Result<Vec<Hash>, CasError> {
            self.inner
                .list_blobs()
                .map(|hs| hs.into_iter().map(|h| Hash(h.0)).collect())
        }

        /// Path of the `objects/` directory.
        pub fn objects_dir(&self) -> PathBuf {
            self.inner.objects_dir()
        }

        /// Path of the `objects/pack/` directory.
        pub fn pack_dir(&self) -> PathBuf {
            self.inner.pack_dir()
        }

        /// Drop the cached pack indices (e.g. after external pack mutation).
        pub fn invalidate_pack_cache(&self) {
            self.inner.invalidate_pack_cache();
        }

        /// Read a blob, consulting packfiles when it is not loose.
        pub fn get_blob_packed(&self, hash: &Hash) -> Result<Vec<u8>, CasError> {
            self.inner.get_blob_packed(&cas_kit::Hash(hash.0))
        }

        /// Whether the blob exists loose or inside any packfile.
        pub fn has_blob_packed(&self, hash: &Hash) -> bool {
            self.inner.has_blob_packed(&cas_kit::Hash(hash.0))
        }

        /// List the hashes of all blobs found in packfiles.
        pub fn list_blobs_packed(&self) -> Result<Vec<Hash>, CasError> {
            self.inner
                .list_blobs_packed()
                .map(|hs| hs.into_iter().map(|h| Hash(h.0)).collect())
        }

        /// Bundle loose blobs into packfiles once the loose count exceeds
        /// `threshold`; returns the number of blobs packed.
        pub fn repack(&self, threshold: usize) -> Result<usize, CasError> {
            self.inner.repack(threshold)
        }
    }
}

/// BLAKE3 hashing helpers, delegating to `cas_kit::hasher`.
#[doc(hidden)]
pub mod hasher {
    use super::store::CasError;
    use suture_common::Hash;

    /// Compute the BLAKE3 hash of arbitrary data.
    #[must_use]
    pub fn hash_bytes(data: &[u8]) -> Hash {
        Hash(cas_kit::hash_bytes(data).0)
    }

    /// Compute the BLAKE3 hash of a file's contents.
    pub fn hash_file(path: &std::path::Path) -> Result<Hash, std::io::Error> {
        cas_kit::hash_file(path).map(|h| Hash(h.0))
    }

    /// Compute a BLAKE3 hash keyed by a domain-separation context string.
    #[must_use]
    pub fn hash_with_context(context: &str, data: &[u8]) -> Hash {
        Hash(cas_kit::hash_with_context(context, data).0)
    }

    /// Verify that `data` hashes to `expected`.
    pub fn verify_hash(data: &[u8], expected: &Hash) -> Result<(), CasError> {
        cas_kit::verify_hash(data, &cas_kit::Hash(expected.0))
    }
}

/// Packfile primitives, delegating to `cas_kit::pack`.
#[doc(hidden)]
pub mod pack {
    use std::path::{Path, PathBuf};
    use suture_common::Hash;

    pub use cas_kit::pack::PackError;

    /// Creates deduplicated packfile/index pairs.
    pub struct PackFile;

    impl PackFile {
        /// Write a new pack file (and its index) containing `objects`.
        ///
        /// The pack name is derived from the BLAKE3 hash of the index, so
        /// identical packs are naturally deduplicated on disk.
        pub fn create(
            pack_dir: &Path,
            objects: &[(Hash, Vec<u8>)],
        ) -> Result<(PathBuf, PathBuf), PackError> {
            let converted: Vec<(cas_kit::Hash, Vec<u8>)> = objects
                .iter()
                .map(|(h, data)| (cas_kit::Hash(h.0), data.clone()))
                .collect();
            cas_kit::PackFile::create(pack_dir, &converted)
        }
    }
}

pub use store::{BlobStore, CasError};

#[cfg(test)]
mod tests {
    use super::{hasher, store::BlobStore};
    use suture_common::Hash;

    #[test]
    fn put_get_roundtrip_dedups() {
        let dir = tempfile::tempdir().unwrap();
        let store = BlobStore::new(dir.path()).unwrap();
        let h = store.put_blob(b"hello, suture").unwrap();
        assert_eq!(store.get_blob(&h).unwrap(), b"hello, suture".to_vec());
        let h2 = store.put_blob(b"hello, suture").unwrap();
        assert_eq!(h, h2);
        assert_eq!(store.blob_count().unwrap(), 1);
    }

    #[test]
    fn hasher_agrees_with_hash_from_data() {
        let data = b"determinism check";
        assert_eq!(hasher::hash_bytes(data), Hash::from_data(data));
        assert_eq!(
            hasher::hash_with_context("ctx", data).0,
            cas_kit::hash_with_context("ctx", data).0
        );
    }
}
