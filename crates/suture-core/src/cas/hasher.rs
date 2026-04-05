//! BLAKE3 hashing utilities for the Content Addressable Storage.
//!
//! All content in Suture is identified by its BLAKE3 hash. BLAKE3 was chosen for:
//! - SIMD acceleration (AVX2, SSE4.1, NEON)
//! - Parallel hashing for large files
//! - 256-bit output (2^128 collision resistance)
//! - Derivation mode for domain-separated hashing

use std::io::Read;
use suture_common::Hash;

/// Compute the BLAKE3 hash of a byte slice.
///
/// This is the primary hashing function used throughout Suture.
/// It uses the default BLAKE3 settings (no key, no context).
#[inline]
pub fn hash_bytes(data: &[u8]) -> Hash {
    Hash::from_data(data)
}

/// Compute the BLAKE3 hash of a file by streaming it in chunks.
///
/// This avoids loading the entire file into memory, which is critical
/// for multi-gigabyte media files.
#[allow(dead_code)]
pub fn hash_file(path: &std::path::Path) -> Result<Hash, std::io::Error> {
    let mut hasher = blake3::Hasher::new();
    let file = std::fs::File::open(path)?;
    let mut reader = std::io::BufReader::new(file);
    const BUFFER_SIZE: usize = 64 * 1024; // 64 KB chunks
    let mut buffer = [0u8; BUFFER_SIZE];

    loop {
        let n = reader.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }

    Ok(Hash::from(*hasher.finalize().as_bytes()))
}

/// Compute the BLAKE3 hash with a domain-separated context string.
///
/// Context strings prevent cross-domain hash collisions. For example,
/// a patch hash and a blob hash should never collide even if they
/// contain identical data. We use keyed hashing with a context-derived key.
pub fn hash_with_context(context: &str, data: &[u8]) -> Hash {
    // Derive a key from the context string
    let context_key = blake3::derive_key(
        std::str::from_utf8(context.as_bytes()).unwrap_or("default"),
        "suture-context-v1".as_bytes(),
    );
    let mut hasher = blake3::Hasher::new_keyed(&context_key);
    hasher.update(data);
    Hash::from(*hasher.finalize().as_bytes())
}

/// Verify that data matches an expected hash.
///
/// Returns `Ok(())` if `blake3(data) == expected`, `Err(CasError::HashMismatch)` otherwise.
pub fn verify_hash(data: &[u8], expected: &Hash) -> Result<(), super::CasError> {
    let actual = hash_bytes(data);
    if actual == *expected {
        Ok(())
    } else {
        Err(super::CasError::HashMismatch {
            expected: expected.to_hex(),
            actual: actual.to_hex(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_bytes_deterministic() {
        let h1 = hash_bytes(b"hello");
        let h2 = hash_bytes(b"hello");
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_hash_bytes_different() {
        let h1 = hash_bytes(b"hello");
        let h2 = hash_bytes(b"world");
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_hash_empty() {
        let h = hash_bytes(b"");
        // BLAKE3 of empty string is a known constant
        let hex = h.to_hex();
        assert!(!hex.is_empty());
        assert_eq!(hex.len(), 64);
    }

    #[test]
    fn test_hash_with_context_differs() {
        let data = b"same data";
        let h1 = hash_with_context("blob", data);
        let h2 = hash_with_context("patch", data);
        assert_ne!(h1, h2, "Different contexts must produce different hashes");
    }

    #[test]
    fn test_verify_hash_ok() {
        let data = b"test data";
        let hash = hash_bytes(data);
        assert!(verify_hash(data, &hash).is_ok());
    }

    #[test]
    fn test_verify_hash_mismatch() {
        let hash = hash_bytes(b"original");
        assert!(verify_hash(b"tampered", &hash).is_err());
    }

    #[test]
    fn test_hash_file() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, b"file content for hashing").unwrap();

        let h1 = hash_file(&file_path).unwrap();
        let h2 = hash_file(&file_path).unwrap();
        assert_eq!(h1, h2);

        let h_direct = hash_bytes(b"file content for hashing");
        assert_eq!(h1, h_direct, "File hash must match direct hash");
    }
}
