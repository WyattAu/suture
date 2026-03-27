---
document_id: BP-CAS-001
version: 1.0.0
status: APPROVED
ieee_1016_compliant: true
component_id: COMP-CAS-001
component_type: Module
interfaces: [IF-CAS-001]
depends_on:
  yellow_papers: [YP-ALGEBRA-PATCH-002]
  blue_papers: []
  external_libs: [blake3-1.8, zstd-0.13, rusqlite-0.39]
created: 2026-03-27
---

# BP-CAS-001: Content Addressable Storage Component

## BP-1: Design Overview

The Content Addressable Storage (CAS) module is the foundational storage layer for Suture.
Every piece of data — patch payloads, serialized metadata, driver outputs — is stored as an
immutable blob indexed by its BLAKE3 cryptographic hash. The CAS provides deduplication by
content identity, lossless compression via Zstd, and integrity verification on every read.

The CAS is designed as a local, single-machine store using the host filesystem. Blobs are
sharded across a directory tree by hash prefix to avoid inode saturation in any single
directory. Metadata about each blob (compression status, size, timestamps) is tracked in the
shared SQLite metadata database (BP-METADATA-001).

The CAS supports two blob modes:
- **Physical blobs**: The full byte content is stored on disk.
- **Virtual blobs**: A pointer to an external file region (path, offset, length, hash) is
  stored. The data is materialized on demand with integrity verification.

### Design Goals

1. **Content identity**: A blob is identified solely by its hash; no separate naming scheme.
2. **Deduplication**: Identical content is stored exactly once, regardless of how many patches
   reference it.
3. **Compression transparency**: Compression/decompression is invisible to callers.
4. **Integrity by default**: Every read verifies the BLAKE3 hash of the retrieved content.
5. **Atomic writes**: A failed store operation leaves no partial or corrupt blob.

---

## BP-2: Design Decomposition

The CAS module is decomposed into three internal sub-modules:

### 2.1 Hasher (`hasher.rs`)

Responsible for computing BLAKE3-256 digests over arbitrary byte sequences. Provides:

- `hash(data: &[u8]) -> [u8; 32]`: Single-pass hash computation.
- `hash_stream(reader: impl Read) -> [u8; 32]`: Streaming hash for large inputs without
  loading the entire blob into memory.
- `Hash` newtype wrapper with `Display` (hex), `FromStr` (hex), and `AsRef<[u8]>` impls.

The hasher uses the `blake3` crate with SIMD auto-detection (AVX-512, AVX-2, SSE4.2, NEON).
No runtime configuration is required; SIMD is selected at compile time via target features.

### 2.2 Compressor (`compressor.rs`)

Responsible for Zstd compression and decompression with configurable compression level.
Provides:

- `compress(data: &[u8], level: i32) -> Vec<u8>`: Compress a byte sequence.
- `decompress(data: &[u8], expected_len: Option<usize>) -> Result<Vec<u8>, CasError>`:
  Decompress, optionally verifying the output length.
- `CompressionConfig` struct with level, workers, and threshold settings.

Compression is applied only to blobs exceeding a configurable size threshold (default: 256
bytes). Blobs smaller than this threshold are stored uncompressed, as the Zstd frame header
overhead exceeds the compression savings for tiny inputs.

### 2.3 BlobStore (`blob_store.rs`)

The primary public API for the CAS. Implements `IF-CAS-001`. Manages the on-disk blob
directory, deduplication logic, and coordinates the hasher and compressor. Provides:

- `put_blob(data: &[u8]) -> Result<Hash, CasError>`
- `get_blob(hash: &Hash) -> Result<Vec<u8>, CasError>`
- `has_blob(hash: &Hash) -> bool`
- `delete_blob(hash: &Hash) -> Result<(), CasError>`
- `list_blobs() -> Result<Vec<BlobMeta>, CasError>`
- `register_virtual_blob(path: &Path, offset: u64, length: u64, hash: &Hash) -> Result<(), CasError>`
- `materialize_virtual_blob(hash: &Hash) -> Result<Vec<u8>, CasError>`

The `BlobStore` holds an `Arc<SqlitePool>` reference to the metadata database for recording
blob metadata. Filesystem I/O is performed asynchronously via `tokio::fs`.

---

## BP-3: Design Rationale

### 3.1 BLAKE3 over SHA-256

| Criterion | BLAKE3 | SHA-256 |
|-----------|--------|---------|
| Throughput (SIMD) | >1 GB/s | ~600 MB/s |
| Parallelizable | Yes (merkle tree) | No (sequential) |
| Output size | 256-bit (configurable) | 256-bit |
| Security margin | $2^{128}$ collision resistance | $2^{128}$ collision resistance |
| SIMD support | AVX-512, AVX-2, SSE4.2, NEON | SHA-NI extensions only |

BLAKE3's parallelizable merkle-tree construction provides significantly higher throughput on
large inputs, directly satisfying REQ-CAS-006 (>1 GB/s). The security margin is equivalent to
SHA-256 for 256-bit output. REQ-CAS-008 (pluggable hash backend) ensures future FIPS
compliance is achievable via a `ContentHasher` trait.

### 3.2 Zstd over LZ4

| Criterion | Zstd (level 3) | LZ4 |
|-----------|----------------|-----|
| Compression ratio | ~2.5:1 (general) | ~2.0:1 (general) |
| Compress speed | ~500 MB/s | ~800 MB/s |
| Decompress speed | >2 GB/s | >3 GB/s |
| Configurability | 1–22 levels + dict | Fixed |

Zstd at level 3 provides superior compression ratio with acceptable speed. Since CAS reads
are latency-sensitive (REQ-PERF-002) and writes are amortized, compression ratio has a
stronger impact on total cost than peak write throughput. LZ4 is available as a future
alternative via the pluggable compressor interface.

### 3.3 Filesystem over Custom Storage

The CAS stores blobs as individual files in a sharded directory tree (`objects/ab/cdef...`).
This approach was chosen over custom binary formats (e.g., pack files) because:

1. **Simplicity**: No custom index maintenance; the filesystem is the index.
2. **Deduplication**: Hard-link-based deduplication is trivially available at the OS level.
3. **Portability**: No binary format versioning concerns for the blob store itself.
4. **Debuggability**: Each blob is inspectable with standard tools (`xxd`, `zstd -d`).

The metadata database (BP-METADATA-001) provides the structured query interface that the raw
filesystem cannot.

---

## BP-4: Traceability

This Blue Paper satisfies the following requirements from SPEC-REQ-001:

| Requirement | Satisfied By | Verification Method |
|-------------|-------------|-------------------|
| REQ-CAS-001 (BLAKE3 content addressing) | BP-1, Hasher module | Unit test: known-answer test vectors |
| REQ-CAS-002 (Zstd compression, level 3) | BP-2, Compressor module | Unit test: round-trip compression |
| REQ-CAS-003 (deduplication) | BlobStore::put_blob | Integration test: duplicate insert |
| REQ-CAS-004 (virtual blobs) | BlobStore::register_virtual_blob | Integration test: VRef lifecycle |
| REQ-CAS-005 (lossless round-trip) | Compressor, get_blob | Property test: `D(C(data)) == data` |
| REQ-CAS-006 (>1 GB/s hashing) | Hasher (BLAKE3 SIMD) | Benchmark: `criterion` throughput |
| REQ-CAS-008 (pluggable hash backend) | `ContentHasher` trait | Unit test: alternate hasher impl |
| REQ-CAS-009 (garbage collection) | BlobStore::gc | Integration test: unreachable blob cleanup |
| REQ-CAS-010 (atomic writes) | BlobStore::put_blob | Integration test: crash recovery |
| REQ-PERF-003 (SIMD BLAKE3) | Hasher module | Build verification: target features |
| REQ-PERF-005 (async I/O) | BlobStore (tokio::fs) | Integration test: concurrent writes |
| REQ-CORE-007 (tokio runtime) | BlobStore (tokio::fs) | Integration test: async context |

---

## BP-5: Interface Design

### IF-CAS-001: BlobStore Public API

```rust
pub struct BlobStore {
    store_path: PathBuf,
    meta_pool: Arc<SqlitePool>,
    compressor: CompressionConfig,
}

#[derive(Debug, Clone)]
pub struct BlobMeta {
    pub hash: Hash,
    pub size: u64,              // uncompressed size
    pub compressed_size: u64,    // size on disk (or 0 if virtual)
    pub is_compressed: bool,
    pub is_virtual: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, thiserror::Error)]
pub enum CasError {
    #[error("blob not found: {0}")]
    NotFound(Hash),
    #[error("integrity check failed: expected {expected}, got {actual}")]
    IntegrityCheckFailed { expected: Hash, actual: Hash },
    #[error("compression error: {0}")]
    CompressionError(String),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("virtual blob error: {0}")]
    VirtualBlobError(String),
}

impl BlobStore {
    pub async fn open(store_path: &Path, meta_pool: Arc<SqlitePool>) -> Result<Self, CasError>;

    /// Store a blob. Returns its BLAKE3 content address.
    /// Precondition: `data` is non-empty.
    /// Postcondition: `get_blob(result) == Ok(data.to_vec())`.
    /// Postcondition: If called twice with identical data, returns the same Hash and does not
    ///   write a second copy (deduplication).
    pub async fn put_blob(&self, data: &[u8]) -> Result<Hash, CasError>;

    /// Retrieve a blob by its content address.
    /// Precondition: `hash` is a valid 32-byte BLAKE3 digest.
    /// Postcondition: Returns the original byte sequence that was stored under this hash.
    /// Postcondition: If the blob is compressed, decompresses transparently.
    /// Postcondition: Verifies `H(result) == hash` (integrity check).
    pub async fn get_blob(&self, hash: &Hash) -> Result<Vec<u8>, CasError>;

    /// Check whether a blob exists without reading its contents.
    /// Precondition: `hash` is a valid 32-byte BLAKE3 digest.
    /// Postcondition: Returns true iff a blob with this hash is stored.
    pub async fn has_blob(&self, hash: &Hash) -> bool;

    /// Delete a blob from the store.
    /// Precondition: `hash` exists in the store.
    /// Postcondition: `has_blob(hash) == false`.
    /// Note: Deletion is logical; garbage collection reclaims disk space.
    pub async fn delete_blob(&self, hash: &Hash) -> Result<(), CasError>;

    /// List all blobs in the store with metadata.
    /// Postcondition: Returns a Vec of BlobMeta for every stored blob.
    pub async fn list_blobs(&self) -> Result<Vec<BlobMeta>, CasError>;

    /// Register a virtual blob reference to an external file.
    /// Precondition: The file at `path` exists and contains `length` bytes at `offset`.
    /// Precondition: `H(file[offset..offset+length]) == hash`.
    /// Postcondition: `has_blob(hash) == true`.
    /// Postcondition: `materialize_virtual_blob(hash)` returns the referenced byte range.
    pub async fn register_virtual_blob(
        &self,
        path: &Path,
        offset: u64,
        length: u64,
        hash: &Hash,
    ) -> Result<(), CasError>;

    /// Materialize a virtual blob by reading the referenced file region.
    /// Precondition: `hash` was previously registered as a virtual blob.
    /// Postcondition: Verifies `H(result) == hash`.
    pub async fn materialize_virtual_blob(&self, hash: &Hash) -> Result<Vec<u8>, CasError>;

    /// Remove blobs that are not reachable from any DAG node.
    /// Precondition: The DAG is loaded and all reachable patch hashes are known.
    /// Postcondition: Unreferenced blobs are removed from the store.
    pub async fn gc(&self, reachable_hashes: &HashSet<Hash>) -> Result<u64, CasError>;
}
```

### IF-CAS-001a: ContentHasher Trait (Pluggable Backend)

```rust
pub trait ContentHasher: Send + Sync {
    fn hash(&self, data: &[u8]) -> [u8; 8];
    fn hash_stream(&self, reader: &mut dyn Read) -> Result<[u8; 8], std::io::Error>;
    fn algorithm_name(&self) -> &str;
}

pub struct Blake3Hasher;
impl ContentHasher for Blake3Hasher { /* ... */ }
```

---

## BP-6: Data Design

### 6.1 On-Disk Directory Layout

```
.suture/
  objects/
    ab/                          # First byte of BLAKE3 hash (hex)
      cdef0123...                # Remaining 31 bytes as filename
      cdef4567...
      ...
    cd/
      ef012345...
      ...
  vrefs/
    ab/
      cdef0123...vref            # JSON: { "path": "...", "offset": N, "length": N }
      ...
  cas.lock                       # Advisory lock file for write serialization
```

Hash-prefix sharding uses the first byte of the hex-encoded BLAKE3 digest (256 possible
directories). Each directory contains files named by the remaining 62 hex characters. This
scales to millions of blobs without exceeding typical filesystem directory limits (ext4: ~10M
entries per directory; with 256-way sharding, effective limit is ~2.5 billion blobs).

### 6.2 Blob File Format

```
[4 bytes: magic "SCAS"]
[1 byte:  version (0x01)]
[1 byte:  flags (bit 0: compressed, bit 1: virtual)]
[variable: blob data (compressed if flag set)]
```

The 6-byte header allows future format evolution without breaking existing blobs. Virtual
blobs store a JSON VRef in place of blob data.

### 6.3 SQLite Metadata Table

```sql
CREATE TABLE cas_blobs (
    hash TEXT PRIMARY KEY,           -- BLAKE3-256 hex digest
    size INTEGER NOT NULL,           -- Uncompressed size in bytes
    compressed_size INTEGER NOT NULL,-- Size on disk
    is_compressed INTEGER NOT NULL,  -- 0 or 1
    is_virtual INTEGER NOT NULL,     -- 0 or 1
    created_at TEXT NOT NULL         -- ISO 8601 timestamp
);
```

---

## BP-7: Component Design

### 7.1 Rust Module Structure

```
suture-core/src/
  cas/
    mod.rs              -- BlobStore struct, public API, CasError
    hasher.rs           -- BLAKE3 hashing, Hash newtype
    compressor.rs       -- Zstd compression, CompressionConfig
    vref.rs             -- Virtual blob registration and materialization
    gc.rs               -- Garbage collection via DAG reachability
    content_hasher.rs   -- ContentHasher trait, Blake3Hasher impl
```

### 7.2 Concurrency Model

- **Reads**: Multiple concurrent readers via `tokio::fs` async I/O. No locking required.
- **Writes**: Serialized through a `tokio::sync::Mutex` to prevent races on dedup check +
  write. This satisfies REQ-CORE-005 (single-writer pipeline).
- **GC**: Runs with an exclusive lock, blocking all reads and writes during traversal.

### 7.3 Error Handling

All CAS operations return `Result<T, CasError>`. The `CasError` enum uses `thiserror` for
ergonomic error propagation. Integrity failures (`IntegrityCheckFailed`) are treated as
fatal and are never silently ignored.

---

## BP-8: Deployment

### 8.1 Environment

- **Platform**: Local filesystem (Linux, macOS, Windows).
- **Storage**: Any filesystem supporting POSIX file semantics (ext4, APFS, NTFS).
- **Runtime**: Tokio async runtime (REQ-CORE-007).

### 8.2 Initialization

The CAS is initialized during `suture init`:
1. Create `.suture/objects/` directory tree (256 subdirectories).
2. Create `.suture/vrefs/` directory tree.
3. Create `cas_blobs` table in the metadata database.
4. Write `cas.lock` file.

### 8.3 Configuration

| Setting | Default | Source |
|---------|---------|--------|
| Compression level | 3 | `config` table in metadata DB |
| Compression threshold | 256 bytes | `config` table in metadata DB |
| Hash algorithm | BLAKE3 | `config` table in metadata DB |
| GC auto-threshold | 1000 unreferenced blobs | `config` table in metadata DB |

---

## BP-9: Formal Verification

The CAS design is grounded in the theorems of YP-ALGEBRA-PATCH-002:

### THM-CAS-001: CAS Integrity (from YP-ALGEBRA-PATCH-002, THM-001)

> *If $H(B_{\text{stored}}) = H(B_{\text{original}})$, then $B_{\text{stored}} = B_{\text{original}}$
>   with probability $\geq 1 - 2^{-128}$.*

**Implementation obligation:** Every `get_blob` call MUST verify `H(retrieved) == requested_hash`
before returning. If verification fails, `CasError::IntegrityCheckFailed` MUST be returned.

**Test obligation:** Property-based test verifying that for all byte sequences $B$:
`get_blob(put_blob(B)) == B`.

### THM-CAS-002: Compression Round-Trip (from YP-ALGEBRA-PATCH-002, THM-002)

> *For all byte sequences: $D(C(\text{data})) = \text{data}$.*

**Implementation obligation:** The compressor module MUST use Zstd with verified
decompression. The `expected_len` parameter to `decompress` provides a secondary check.

**Test obligation:** Property-based test: `decompress(compress(data), Some(data.len())) == data`
for random byte sequences of size 0 to 10 MiB.

### THM-CAS-003: Deduplication Correctness (from YP-ALGEBRA-PATCH-002, THM-003)

> *If $H(B_1) = H(B_2)$, the CAS stores exactly one copy.*

**Implementation obligation:** `put_blob` MUST compute the hash before writing and skip
the write if the blob already exists. The dedup check and write MUST be atomic (protected
by the write mutex).

**Test obligation:** Integration test: store blob B, store identical blob B again, verify
only one file exists on disk.

---

## BP-10: Compliance

### IEC 61508 SIL-2 (Data Integrity)

The CAS is classified as a SIL-2 component within Suture's safety architecture because
corruption of stored blobs would result in irreversible data loss (the primary hazard for a
version control system). The following measures ensure SIL-2 compliance:

1. **Integrity verification on every read** (THM-CAS-001): Prevents silent data corruption.
2. **Atomic writes** (REQ-CAS-010): Write-to-temporary-then-rename pattern prevents partial
   blobs on crash.
3. **Deterministic hashing** (AX-002 of YP-ALGEBRA-PATCH-001): Eliminates non-determinism
   in content addressing.
4. **No mutable state in blobs**: Once stored, a blob's content never changes. Mutation
   requires a new hash and a new blob.

### TQA Level 4 (Formal Verification)

The CAS achieves TQA Level 4 through:
- Formal theorem proofs (THM-CAS-001, THM-CAS-002, THM-CAS-003).
- Property-based tests covering all invariants.
- Known-answer test vectors for BLAKE3 and Zstd.

---

## BP-11: Compliance Matrix

| Requirement | Section | Status | Verification |
|-------------|---------|--------|-------------|
| REQ-CAS-001 | BP-5, Hasher | Satisfied | Unit tests |
| REQ-CAS-002 | BP-5, Compressor | Satisfied | Unit tests |
| REQ-CAS-003 | BP-5, put_blob | Satisfied | Integration tests |
| REQ-CAS-004 | BP-5, VRef API | Satisfied | Integration tests |
| REQ-CAS-005 | BP-9, THM-CAS-002 | Satisfied | Property tests |
| REQ-CAS-006 | BP-3.1, Hasher | Satisfied | Benchmarks |
| REQ-CAS-007 | BP-3.2 | Satisfied | Benchmarks |
| REQ-CAS-008 | BP-5, ContentHasher | Satisfied | Unit tests |
| REQ-CAS-009 | BP-5, gc | Satisfied | Integration tests |
| REQ-CAS-010 | BP-7.2, Atomic writes | Satisfied | Crash tests |
| REQ-CORE-002 | BP-9, Determinism | Satisfied | Property tests |
| REQ-CORE-007 | BP-7.2, tokio | Satisfied | Integration tests |
| REQ-PERF-003 | BP-3.1 | Satisfied | Build flags |
| REQ-PERF-005 | BP-7.2, async I/O | Satisfied | Integration tests |

---

## BP-12: Quality Checklist

- [ ] All public API functions have preconditions and postconditions documented.
- [ ] Property-based tests verify THM-CAS-001 (integrity), THM-CAS-002 (round-trip), THM-CAS-003 (dedup).
- [ ] Known-answer test vectors for BLAKE3 (from official BLAKE3 test suite).
- [ ] Known-answer test vectors for Zstd round-trip.
- [ ] Atomic write pattern verified via crash simulation test.
- [ ] Deduplication verified: two identical puts produce one file on disk.
- [ ] Virtual blob lifecycle: register, materialize, verify integrity, detect tampering.
- [ ] Garbage collection: unreferenced blobs are cleaned; referenced blobs are preserved.
- [ ] Compression threshold: blobs < 256 bytes stored uncompressed.
- [ ] SIMD auto-detection verified: builds on x86_64 and aarch64 without manual configuration.
- [ ] `ContentHasher` trait has at least one alternate implementation in tests.
- [ ] All async functions are cancel-safe (no partial state on `tokio::select` cancellation).
- [ ] Error messages are human-readable and include the offending hash in hex.
- [ ] `cargo clippy` passes with zero warnings on the `cas` module.
- [ ] `cargo test` passes all CAS tests.

---

*End of BP-CAS-001*
