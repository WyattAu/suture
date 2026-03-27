# Changelog

## [0.1.0] - 2026-03-27

### Added

#### Core Engine
- Content Addressable Storage (CAS) with BLAKE3 hashing and Zstd compression
- Patch Algebra engine with commutativity detection and merge computation
- Patch DAG (Directed Acyclic Graph) with branch management and LCA computation
- SQLite metadata store with WAL mode
- Patch Application Engine: `apply_patch`, `apply_patch_chain`, `resolve_payload_to_hash`
- FileTree: in-memory file tree with insert/remove/rename operations
- Diff Engine: `diff_trees` with Added/Modified/Deleted/Renamed detection
- Full DAG reconstruction on `Repository::open()` (loads all patches from SQLite)
- `.sutureignore` support with glob-like matching (`*.o`, `build/`, exact match)
- `Repository::add_all()` — stage all files respecting ignore patterns
- `Repository::snapshot()` / `snapshot_head()` — build FileTree from patch chain
- `Repository::checkout()` — switch branches, update working tree, HEAD tracking
- `Repository::diff()` — compare two commits/branches via snapshot diff
- `Repository::revert()` — create inverse patches (Delete for Create/Modify, re-create for Delete)
- `DirtyWorkingTree` error — checkout refuses if staged changes exist

#### Ed25519 Signing
- `signing` module with `SigningKeypair` (generate, sign, verify)
- `canonical_patch_bytes()` — deterministic serialization for signing
- `verify_signature()` — verify Ed25519 signatures against public keys
- Metadata store: `public_keys` and `signatures` tables with CRUD methods

#### CLI
- `init` — initialize a new Suture repository
- `status` — show repository status, staged files, branch info
- `add` — stage files (with `--all` / `-a` flag for add-all)
- `commit` — create a commit from staged changes
- `branch` — create branches with optional target
- `log` — show commit history
- `merge` — compute merge plan (dry-run with conflict detection)
- `checkout` — switch branches, update working tree
- `diff` — show differences between commits/branches
- `revert` — revert a commit by hash

#### Testing
- 166 tests across 5 crates (0 failures)
- 21 proptest property-based test suites (10K+ randomized cases):
  - FileTree: insert/contains, remove, rename, equality, symmetry
  - Patch apply: create, modify, delete, chain commutativity
  - DAG: patch count, chain ancestry, LCA linear, ancestors subset
  - CAS: put/get roundtrip, content addressing, idempotency
  - Diff: empty-vs-full, full-vs-empty, identical, symmetry inverse

#### Benchmarks
- 6 Criterion benchmark groups (`cargo bench -p suture-bench`):
  - CAS put/get: 1KB, 10KB, 100KB blobs
  - BLAKE3 hashing: 64B, 1KB, 10KB, 100KB
  - DAG insertion: 10, 100, 1000 patch linear chains
  - DAG LCA: chains of 10, 100, 500 patches
  - Patch chain application: 10, 50, 100 create patches
  - FileTree diff: 10, 50, 100 entry trees

#### Quality Compliance
- `cargo clippy --workspace --all-targets`: zero warnings
- `cargo audit`: zero vulnerabilities (180 crate dependencies scanned)
- Zero compiler warnings on `cargo build --workspace`

### Specifications
- Yellow Papers: Patch Algebra, Serialization, Distributed Consensus
- Blue Papers: CAS, Patch Algebra, Patch DAG, Metadata, Driver SDK, CLI
- Lean 4 formal proofs (pending toolchain — proofs use `sorry` placeholders)
- Interface contracts for all components
- STRIDE threat model and security test plan
- Performance requirements and benchmark suite

### Known Limitations
- No VFS (NFSv4/ProjFS) support
- No distributed sync (Raft/gRPC)
- No interactive conflict resolution (merge is dry-run only)
- Ed25519 signing module is ready but not yet integrated into commit flow
- Lean 4 formal proofs pending toolchain installation
- Checkout does not handle uncommitted working tree changes (only staged)
