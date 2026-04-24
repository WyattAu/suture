//! Suture Common — Shared types, errors, and utilities used across all crates.
//!
//! This crate defines the foundational data structures that every other crate
//! depends on: hashes, patch IDs, branch names, error types, and serialization
//! helpers.

use blake3::Hash as Blake3Hash;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;
use thiserror::Error;

// =============================================================================
// Core Identifier Types
// =============================================================================

/// A BLAKE3 content hash (32 bytes / 256 bits).
///
/// Used as the canonical identifier for blobs in the Content Addressable Storage
/// and for patch identifiers. BLAKE3 provides SIMD-accelerated hashing with a
/// 2^128 collision resistance bound.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Hash(pub [u8; 32]);

impl Hash {
    /// Compute the BLAKE3 hash of arbitrary data.
    pub fn from_data(data: &[u8]) -> Self {
        Self(*blake3::hash(data).as_bytes())
    }

    /// Parse a hash from a 64-character hex string.
    pub fn from_hex(hex: &str) -> Result<Self, CommonError> {
        if hex.len() != 64 {
            return Err(CommonError::InvalidHashLength(hex.len()));
        }
        let mut bytes = [0u8; 32];
        hex.as_bytes()
            .chunks_exact(2)
            .zip(bytes.iter_mut())
            .try_for_each(|(chunk, byte)| {
                *byte = u8::from_str_radix(
                    std::str::from_utf8(chunk).map_err(|_| CommonError::InvalidHex)?,
                    16,
                )
                .map_err(|_| CommonError::InvalidHex)?;
                Ok::<_, CommonError>(())
            })?;
        Ok(Self(bytes))
    }

    /// Convert to a 64-character lowercase hex string.
    pub fn to_hex(&self) -> String {
        const HEX_CHARS: &[u8; 16] = b"0123456789abcdef";
        let mut s = String::with_capacity(64);
        for byte in &self.0 {
            s.push(HEX_CHARS[(byte >> 4) as usize] as char);
            s.push(HEX_CHARS[(byte & 0x0f) as usize] as char);
        }
        s
    }

    /// The zero hash (all zeros). Used as a sentinel value.
    pub const ZERO: Self = Self([0u8; 32]);

    /// Convert to a blake3::Hash reference.
    pub fn as_blake3(&self) -> &Blake3Hash {
        // SAFETY: blake3::Hash is a newtype around [u8; 32] with repr(transparent),
        // so the pointer cast is sound. The layout is verified at compile time
        // by the repr(transparent) attribute.
        unsafe { &*(&self.0 as *const [u8; 32] as *const Blake3Hash) }
    }
}

impl fmt::Debug for Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Hash({})", self.to_hex())
    }
}

impl fmt::Display for Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Display short form: first 12 hex chars
        let hex = self.to_hex();
        write!(f, "{}…", &hex[..12])
    }
}

impl From<Blake3Hash> for Hash {
    fn from(h: Blake3Hash) -> Self {
        Self(*h.as_bytes())
    }
}

impl From<[u8; 32]> for Hash {
    fn from(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

/// A patch identifier — currently identical to a BLAKE3 hash of the patch content.
pub type PatchId = Hash;

/// A branch name. Must be non-empty and contain only valid UTF-8.
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BranchName(pub String);

impl BranchName {
    pub fn new(name: impl Into<String>) -> Result<Self, CommonError> {
        let s = name.into();
        if s.is_empty() {
            return Err(CommonError::EmptyBranchName);
        }
        // Validate: no null bytes
        if s.contains('\0') {
            return Err(CommonError::InvalidBranchName(s));
        }
        Ok(Self(s))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for BranchName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Branch({})", self.0)
    }
}

impl fmt::Display for BranchName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for BranchName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

// =============================================================================
// Error Types
// =============================================================================

/// Top-level error type for the suture-common crate.
#[derive(Error, Debug)]
pub enum CommonError {
    #[error("invalid hash length: expected 64 hex chars, got {0}")]
    InvalidHashLength(usize),

    #[error("invalid hexadecimal string")]
    InvalidHex,

    #[error("branch name must not be empty")]
    EmptyBranchName,

    #[error("invalid branch name: {0}")]
    InvalidBranchName(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Custom(String),
}

// =============================================================================
// Repository Path Types
// =============================================================================

/// The path to a file within a Suture repository (relative to repo root).
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RepoPath(pub String);

impl RepoPath {
    pub fn new(path: impl Into<String>) -> Result<Self, CommonError> {
        let s = path.into();
        if s.is_empty() {
            return Err(CommonError::Custom("repo path must not be empty".into()));
        }
        Ok(Self(s))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn to_path_buf(&self) -> PathBuf {
        PathBuf::from(&self.0)
    }
}

impl fmt::Debug for RepoPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RepoPath({})", self.0)
    }
}

impl fmt::Display for RepoPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// =============================================================================
// Status Type for Working Set Files
// =============================================================================

/// Status of a file in the working set.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum FileStatus {
    /// File is added but not yet committed.
    Added,
    /// File is modified relative to the last commit.
    Modified,
    /// File is deleted from the working tree.
    Deleted,
    /// File is unmodified (tracked but clean).
    Clean,
    /// File is not tracked by Suture.
    Untracked,
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_from_data_deterministic() {
        let data = b"hello, suture";
        let h1 = Hash::from_data(data);
        let h2 = Hash::from_data(data);
        assert_eq!(h1, h2, "Hash must be deterministic");
    }

    #[test]
    fn test_hash_different_data() {
        let h1 = Hash::from_data(b"hello");
        let h2 = Hash::from_data(b"world");
        assert_ne!(h1, h2, "Different data must produce different hashes");
    }

    #[test]
    fn test_hash_hex_roundtrip() {
        let data = b"test data for hex roundtrip";
        let hash = Hash::from_data(data);
        let hex = hash.to_hex();
        assert_eq!(hex.len(), 64, "Hex string must be 64 characters");

        let parsed = Hash::from_hex(&hex).expect("Valid hex must parse");
        assert_eq!(hash, parsed, "Hex roundtrip must preserve hash");
    }

    #[test]
    fn test_hash_from_hex_invalid() {
        assert!(Hash::from_hex("too short").is_err());
        assert!(Hash::from_hex("not hex!!characters!!64!!").is_err());
    }

    #[test]
    fn test_hash_zero() {
        let zero = Hash::ZERO;
        assert_eq!(zero.to_hex(), "0".repeat(64));
    }

    #[test]
    fn test_branch_name_valid() {
        assert!(BranchName::new("main").is_ok());
        assert!(BranchName::new("feature/my-feature").is_ok());
        assert!(BranchName::new("fix-123").is_ok());
    }

    #[test]
    fn test_branch_name_invalid() {
        assert!(BranchName::new("").is_err());
        assert!(BranchName::new("has\0null").is_err());
    }

    #[test]
    fn test_repo_path() {
        let path = RepoPath::new("src/main.rs").unwrap();
        assert_eq!(path.as_str(), "src/main.rs");
    }
}
