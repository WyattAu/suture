//! Blob Store — the primary CAS interface for storing and retrieving blobs.
//!
//! Blobs are stored on disk using a content-addressed scheme:
//! - Hash is split into a 2-char prefix directory and 62-char filename
//! - This creates 256 buckets, avoiding any single directory having too many files
//! - Blobs are optionally Zstd-compressed
//!
//! # Thread Safety
//!
//! `BlobStore` is `Send + Sync` and can be shared across threads via `Arc`.
//! File operations are the primary bottleneck; the store itself holds no mutable
//! state beyond the root path.

use crate::cas::compressor::{self, is_zstd_compressed};
use crate::cas::hasher;
use std::fs;
use std::io;
use std::path::PathBuf;
use suture_common::Hash;
use thiserror::Error;

/// Errors that can occur during CAS operations.
#[derive(Error, Debug)]
pub enum CasError {
    #[error("blob not found: {0}")]
    BlobNotFound(String),

    #[error("hash mismatch: expected {expected}, got {actual}")]
    HashMismatch { expected: String, actual: String },

    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("compression error: {0}")]
    CompressionError(String),

    #[error("decompression error: {0}")]
    DecompressionError(String),

    #[error("decompressed data too large: {max} bytes max")]
    DecompressionTooLarge { max: usize },

    #[error("blob already exists: {0}")]
    AlreadyExists(String),

    #[error("invalid path: {0}")]
    InvalidPath(String),
}

/// The Content Addressable Storage blob store.
///
/// Stores blobs indexed by BLAKE3 hash on the local filesystem.
/// Provides deduplication, optional compression, and integrity verification.
pub struct BlobStore {
    /// Root directory containing the `objects/` subdirectory.
    root: PathBuf,
    /// Whether to compress blobs with Zstd.
    compress: bool,
    /// Zstd compression level (1-22).
    compression_level: i32,
}

impl BlobStore {
    /// Create a new BlobStore rooted at the given directory.
    ///
    /// Creates the `objects/` subdirectory if it doesn't exist.
    pub fn new(root: impl Into<PathBuf>) -> Result<Self, CasError> {
        let root = root.into();
        let objects_dir = root.join("objects");
        fs::create_dir_all(&objects_dir)?;
        Ok(Self {
            root,
            compress: true,
            compression_level: compressor::DEFAULT_COMPRESSION_LEVEL,
        })
    }

    /// Create a BlobStore with compression disabled (for testing).
    pub fn new_uncompressed(root: impl Into<PathBuf>) -> Result<Self, CasError> {
        let mut store = Self::new(root)?;
        store.compress = false;
        Ok(store)
    }

    /// Store a blob, returning its BLAKE3 hash.
    ///
    /// If a blob with the same hash already exists, this is a no-op
    /// (deduplication). Returns the hash either way.
    pub fn put_blob(&self, data: &[u8]) -> Result<Hash, CasError> {
        let hash = hasher::hash_bytes(data);
        let blob_path = self.blob_path(&hash);

        // Deduplication: if blob already exists, return immediately
        if blob_path.exists() {
            return Ok(hash);
        }

        // Ensure the prefix directory exists
        if let Some(parent) = blob_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Write blob (optionally compressed)
        if self.compress {
            let compressed = compressor::compress(data, self.compression_level)?;
            fs::write(&blob_path, &compressed)?;
        } else {
            fs::write(&blob_path, data)?;
        }

        Ok(hash)
    }

    /// Store a blob, returning an error if it already exists.
    pub fn put_blob_new(&self, data: &[u8]) -> Result<Hash, CasError> {
        let hash = hasher::hash_bytes(data);
        let blob_path = self.blob_path(&hash);

        if blob_path.exists() {
            return Err(CasError::AlreadyExists(hash.to_hex()));
        }

        if let Some(parent) = blob_path.parent() {
            fs::create_dir_all(parent)?;
        }

        if self.compress {
            let compressed = compressor::compress(data, self.compression_level)?;
            fs::write(&blob_path, &compressed)?;
        } else {
            fs::write(&blob_path, data)?;
        }

        Ok(hash)
    }

    /// Retrieve a blob by its BLAKE3 hash.
    ///
    /// Decompresses if necessary and verifies the hash of the result.
    pub fn get_blob(&self, hash: &Hash) -> Result<Vec<u8>, CasError> {
        let blob_path = self.blob_path(hash);
        let raw = fs::read(&blob_path).map_err(|e| {
            if e.kind() == io::ErrorKind::NotFound {
                CasError::BlobNotFound(hash.to_hex())
            } else {
                CasError::Io(e)
            }
        })?;

        // Decompress if necessary
        let data = if is_zstd_compressed(&raw) {
            compressor::decompress(&raw)?
        } else {
            raw
        };

        // Verify integrity
        hasher::verify_hash(&data, hash)?;

        Ok(data)
    }

    /// Check if a blob exists in the store.
    ///
    /// This does NOT verify the blob's integrity — it only checks for existence.
    pub fn has_blob(&self, hash: &Hash) -> bool {
        self.blob_path(hash).exists()
    }

    /// Delete a blob from the store.
    ///
    /// The caller is responsible for ensuring no patches reference this blob.
    pub fn delete_blob(&self, hash: &Hash) -> Result<(), CasError> {
        let blob_path = self.blob_path(hash);
        fs::remove_file(&blob_path).map_err(|e| {
            if e.kind() == io::ErrorKind::NotFound {
                CasError::BlobNotFound(hash.to_hex())
            } else {
                CasError::Io(e)
            }
        })
    }

    /// Get the total number of blobs in the store.
    pub fn blob_count(&self) -> Result<u64, CasError> {
        let objects_dir = self.root.join("objects");
        let mut count = 0u64;
        if objects_dir.exists() {
            for entry in fs::read_dir(&objects_dir)? {
                let entry = entry?;
                if entry.file_type()?.is_dir() {
                    for sub_entry in fs::read_dir(entry.path())? {
                        let sub_entry = sub_entry?;
                        if sub_entry.file_type()?.is_file() {
                            count += 1;
                        }
                    }
                }
            }
        }
        Ok(count)
    }

    /// Get the total size of all blobs in the store (compressed).
    pub fn total_size(&self) -> Result<u64, CasError> {
        let objects_dir = self.root.join("objects");
        let mut total = 0u64;
        if objects_dir.exists() {
            for entry in fs::read_dir(&objects_dir)? {
                let entry = entry?;
                if entry.file_type()?.is_dir() {
                    for sub_entry in fs::read_dir(entry.path())? {
                        let sub_entry = sub_entry?;
                        if sub_entry.file_type()?.is_file() {
                            total += sub_entry.metadata()?.len();
                        }
                    }
                }
            }
        }
        Ok(total)
    }

    /// List all blob hashes in the store.
    pub fn list_blobs(&self) -> Result<Vec<Hash>, CasError> {
        let objects_dir = self.root.join("objects");
        let mut hashes = Vec::new();
        if !objects_dir.exists() {
            return Ok(hashes);
        }
        for entry in fs::read_dir(&objects_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                let prefix = entry.file_name().to_string_lossy().to_string();
                for sub_entry in fs::read_dir(entry.path())? {
                    let sub_entry = sub_entry?;
                    if sub_entry.file_type()?.is_file() {
                        let suffix = sub_entry.file_name().to_string_lossy().to_string();
                        let hex = format!("{prefix}{suffix}");
                        if let Ok(hash) = Hash::from_hex(&hex) {
                            hashes.push(hash);
                        }
                    }
                }
            }
        }
        Ok(hashes)
    }

    /// Get the on-disk path for a given hash.
    fn blob_path(&self, hash: &Hash) -> PathBuf {
        let hex = hash.to_hex();
        let prefix = &hex[..2];
        let suffix = &hex[2..];
        self.root.join("objects").join(prefix).join(suffix)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_store() -> (TempDir, BlobStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = BlobStore::new_uncompressed(dir.path()).unwrap();
        (dir, store)
    }

    #[test]
    fn test_put_and_get_blob() {
        let (_dir, store) = make_store();
        let data = b"hello, suture!";
        let hash = store.put_blob(data).unwrap();

        let retrieved = store.get_blob(&hash).unwrap();
        assert_eq!(data.as_slice(), retrieved.as_slice());
    }

    #[test]
    fn test_deduplication() {
        let (_dir, store) = make_store();
        let data = b"deduplicate me";

        let h1 = store.put_blob(data).unwrap();
        let h2 = store.put_blob(data).unwrap();
        assert_eq!(h1, h2);

        assert_eq!(store.blob_count().unwrap(), 1, "Only one copy should exist");
    }

    #[test]
    fn test_has_blob() {
        let (_dir, store) = make_store();
        let hash = store.put_blob(b"exists").unwrap();

        assert!(store.has_blob(&hash));
        let missing = Hash::from_hex(&"f".repeat(64)).unwrap();
        assert!(!store.has_blob(&missing));
    }

    #[test]
    fn test_get_nonexistent_blob() {
        let (_dir, store) = make_store();
        let missing = Hash::from_hex(&"a".repeat(64)).unwrap();
        let result = store.get_blob(&missing);
        assert!(matches!(result, Err(CasError::BlobNotFound(_))));
    }

    #[test]
    fn test_delete_blob() {
        let (_dir, store) = make_store();
        let hash = store.put_blob(b"delete me").unwrap();
        assert!(store.has_blob(&hash));

        store.delete_blob(&hash).unwrap();
        assert!(!store.has_blob(&hash));
    }

    #[test]
    fn test_delete_nonexistent_blob() {
        let (_dir, store) = make_store();
        let missing = Hash::from_hex(&"b".repeat(64)).unwrap();
        let result = store.delete_blob(&missing);
        assert!(matches!(result, Err(CasError::BlobNotFound(_))));
    }

    #[test]
    fn test_put_blob_new_rejects_duplicate() {
        let (_dir, store) = make_store();
        let data = b"duplicate";
        store.put_blob(data).unwrap();
        let result = store.put_blob_new(data);
        assert!(matches!(result, Err(CasError::AlreadyExists(_))));
    }

    #[test]
    fn test_blob_count_and_list() {
        let (_dir, store) = make_store();
        store.put_blob(b"one").unwrap();
        store.put_blob(b"two").unwrap();
        store.put_blob(b"three").unwrap();

        assert_eq!(store.blob_count().unwrap(), 3);
        assert_eq!(store.list_blobs().unwrap().len(), 3);
    }

    #[test]
    fn test_large_blob() {
        let (_dir, store) = make_store();
        // 10 MB blob
        let data: Vec<u8> = (0..10_000_000).map(|i| (i % 256) as u8).collect();
        let hash = store.put_blob(&data).unwrap();

        let retrieved = store.get_blob(&hash).unwrap();
        assert_eq!(data.len(), retrieved.len());
        assert_eq!(data, retrieved);
    }

    #[test]
    fn test_hash_integrity() {
        let (_dir, store) = make_store();
        let data = b"integrity check";
        let hash = store.put_blob(data).unwrap();

        // Manually corrupt the stored blob
        let blob_path = store.blob_path(&hash);
        let mut corrupted = fs::read(&blob_path).unwrap();
        corrupted[0] = corrupted[0].wrapping_add(1);
        fs::write(&blob_path, &corrupted).unwrap();

        // Getting the corrupted blob should fail integrity check
        let result = store.get_blob(&hash);
        assert!(matches!(result, Err(CasError::HashMismatch { .. })));
    }

    #[test]
    fn test_compressed_store() {
        let dir = tempfile::tempdir().unwrap();
        let store = BlobStore::new(dir.path()).unwrap();

        let data = b"this will be compressed";
        let hash = store.put_blob(data).unwrap();

        // Verify the stored file is actually compressed
        let blob_path = store.blob_path(&hash);
        let raw = fs::read(&blob_path).unwrap();
        assert!(is_zstd_compressed(&raw), "Blob should be Zstd-compressed");

        // Verify round-trip
        let retrieved = store.get_blob(&hash).unwrap();
        assert_eq!(data.as_slice(), retrieved.as_slice());
    }

    mod proptests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn put_get_roundtrip(data in proptest::collection::vec(proptest::num::u8::ANY, 0..1024)) {
                let dir = tempfile::tempdir().unwrap();
                let store = BlobStore::new_uncompressed(dir.path()).unwrap();
                let hash = store.put_blob(&data).unwrap();
                let retrieved = store.get_blob(&hash).unwrap();
                prop_assert_eq!(data, retrieved);
            }

            #[test]
            fn content_addressing(
                data1 in proptest::collection::vec(proptest::num::u8::ANY, 0..512),
                data2 in proptest::collection::vec(proptest::num::u8::ANY, 0..512)
            ) {
                let dir = tempfile::tempdir().unwrap();
                let store = BlobStore::new_uncompressed(dir.path()).unwrap();

                let hash1 = store.put_blob(&data1).unwrap();
                let hash2 = store.put_blob(&data2).unwrap();

                if data1 == data2 {
                    prop_assert_eq!(hash1, hash2, "same data must produce same hash");
                } else {
                    prop_assert_ne!(hash1, hash2, "different data must produce different hashes");
                }
            }

            #[test]
            fn put_twice_idempotent(data in proptest::collection::vec(proptest::num::u8::ANY, 0..1024)) {
                let dir = tempfile::tempdir().unwrap();
                let store = BlobStore::new_uncompressed(dir.path()).unwrap();

                let hash1 = store.put_blob(&data).unwrap();
                let hash2 = store.put_blob(&data).unwrap();
                prop_assert_eq!(hash1, hash2);
                prop_assert_eq!(store.blob_count().unwrap(), 1);
            }
        }
    }
}
