# Changelog

## [0.2.0] - 2026-03-27

### Added

#### Incremental Push
- `patches_since(since_id)` — walks DAG from branch tips, returns only new patches
- Topological sort (Kahn's algorithm) ensures parents-before-children ordering
- CLI `push` tracks `remote.<name>.last_pushed` config for incremental sync
- Push now shows patch count: "Push successful (3 patch(es))"

#### Author Identity
- `Repository::init()` defaults to `"unknown"` author (no longer hardcoded `"alice"`)
- `open()` reads `user.name` config first, falls back to `author`, then `"unknown"`
- `get_config` / `set_config` / `list_config` exposed as public API
- CLI `config` command: list all, get single key, set key=value
- Internal keys (`head_branch`, `pending_merge_parents`) hidden from `config` listing
- Init prints hint: "run `suture config user.name \"Your Name\"` to set your identity"

#### Tag Support
- `create_tag(name, target)` / `delete_tag(name)` / `list_tags()` / `resolve_tag(name)`
- Tags stored as `tag.<name>` config entries mapping to patch IDs
- CLI `tag` command: list all, create at HEAD or `--target <ref>`, `--delete`

#### Branch Delete
- `delete_branch(name)` with current-branch protection
- Removes branch from both DAG and metadata
- CLI `branch --delete <name>`

#### Conflict Resolution Persistence
- `pending_merge_parents` persisted to config as JSON on merge
- Restored on `Repository::open()`
- Cleared on commit (conflict resolved)

#### Ed25519 Signing Wired Into Push
- CLI `key generate [name]` — generates Ed25519 keypair
- Private key saved to `.suture/keys/<name>.ed25519`
- Public key stored in config as `key.public.<name>`
- CLI `key list` / `key public [name]`
- `signing.key` config auto-set to `"default"` on key generation
- `cmd_push()` reads private key, signs canonical push bytes, attaches signature
- Hub only verifies signatures when authorized keys are configured
- Hub accepts signed pushes even without auth (backward compatible)
- `canonical_push_bytes()` aligned between CLI and hub (includes operation_type)

#### Line-Level Diff3 Merge
- `engine::merge` module with LCS-based line diff algorithm
- `three_way_merge_lines()` — performs three-way merge with conflict markers
- `MergeOutput` struct with `lines`, `is_clean`, `auto_merged`, `conflicts` fields
- Conflict markers use configurable labels (e.g., `<<<<<<< ours (HEAD)`)
- Handles: trivial cases, one-side changes, non-overlapping regions, same-change detection, empty base, trailing insertions
- 13 unit tests covering all merge scenarios

#### Hub SQLite Persistence
- `HubStorage` module — SQLite-backed storage replacing in-memory HashMaps
- Persistent repos, patches, branches, and blobs across hub restarts
- `--db <path>` CLI flag for `suture-hub` to specify database file
- In-memory mode still available (default, for testing)
- 4 storage-specific tests including cross-reopen persistence

#### Hub Ed25519 Authentication
- `authorized_keys` table in hub storage for registering public keys
- Push request signature verification using canonical bytes
- Auth is optional — only enforced when authorized keys are configured
- `signature` field in `PushRequest` (optional, 64-byte Ed25519 sig)
- 3 auth tests: required-when-keys-exist, valid-signature-succeeds, no-auth-when-unconfigured

#### Quality
- Test count: 203 (up from 180 in v0.1.0)
- Zero clippy warnings, zero audit findings
- End-to-end verified: signed push with hub auth, pull by unauthorized client

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

#### Distributed Sync
- Hub daemon (`suture-hub`) with axum-based HTTP API
- `POST /push` — push patches, branches, and blobs to hub
- `POST /pull` — pull new patches based on client's known branches
- `GET /repos` — list all repositories
- `GET /repo/{id}` — get repo info (patch count, branches)
- Topological sort for patch delivery order
- CLI: `remote add`, `remote list`, `push`, `pull` commands
- End-to-end verified: repo A pushes → Hub stores → repo B pulls → files appear on disk

#### Merge Execution
- `execute_merge()` — creates two-parent merge commits for clean merges
- Fast-forward detection (single-parent merge when possible)
- Diff3 conflict markers with `<<<<<<< ours` / `=======` / `>>>>>>> theirs` labels
- `pending_merge_parents` for multi-parent merge commits
- Conflict resolution via `resolve_merge_conflict()`

#### Snapshot Caching
- RefCell-based snapshot cache in Repository
- `invalidate_head_cache()` on commit/revert/merge
- O(1) `snapshot_head()` for repeated calls (was O(n) iterating all patches)
- Self-host test: 101 files committed in 251ms, status/log in ~60ms

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
- `merge` — merge a branch into HEAD (with conflict detection)
- `checkout` — switch branches, update working tree
- `diff` — show differences between commits/branches
- `revert` — revert a commit by hash
- `remote add` / `remote list` — manage remote hubs
- `push` / `pull` — distributed sync with hub

#### Testing
- 180 tests across 6 crates (0 failures)
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
- `cargo audit`: zero vulnerabilities (300 crate dependencies scanned)
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
- No Raft/gRPC-based distributed consensus
- Lean 4 formal proofs pending toolchain installation
- Checkout does not handle uncommitted working tree changes (only staged)
