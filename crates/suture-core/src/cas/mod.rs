//! Content Addressable Storage (CAS) — BLAKE3-indexed blob store with Zstd compression.
//!
//! The CAS is the foundational storage layer of Suture. Every piece of data —
//! file content, patch payloads, metadata — is stored as a blob indexed by its
//! BLAKE3 hash. Identical blobs are deduplicated automatically.
//!
//! # On-Disk Layout
//!
//! ```text
//! .suture/
//!   objects/
//!     ab/           # First 2 hex chars of hash (256 buckets)
//!       cdef...     # Remaining 62 hex chars = blob filename
//!   metadata.db     # SQLite database (handled by metadata module)
//! ```
//!
//! # Correctness Properties
//!
//! - **Integrity**: `get(H(data)) == data` (BLAKE3 collision resistance)
//! - **Deduplication**: Storing the same blob twice uses one copy
//! - **Lossless**: Zstd compression/decompression is lossless

#[doc(hidden)]
pub mod compressor;
#[doc(hidden)]
pub mod hasher;
#[doc(hidden)]
pub mod pack;
pub mod store;

pub use store::{BlobStore, CasError};
