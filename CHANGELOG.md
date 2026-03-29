# Changelog

## [0.9.0] - 2026-03-29

### Added

#### `suture gc` (Path A — Git Parity)
- `gc()` — garbage collection for unreachable patches
- Walks all branch tips, collects reachable patches via ancestor traversal
- Deletes unreachable patches from metadata (patches, edges, signatures tables)
- CLI: `suture gc` — reports count of removed patches

#### `suture fsck` (Path A — Git Parity)
- `fsck()` — verify repository integrity
- Checks DAG parent consistency (all parent IDs exist)
- Checks branch target validity (all branches point to existing patches)
- Checks blob references (patch payloads resolve to CAS blobs)
- Checks HEAD consistency (current branch exists)
- CLI: `suture fsck` — reports passed checks, warnings, and errors

#### `suture bisect` (Path A — Git Parity)
- Binary search for bug-introducing commit
- Accepts good and bad refs (commit hashes, branch names, partial hashes)
- Finds midpoint in linear history, prints guidance for narrowing
- Reports first bad commit when range is narrowed to one commit
- CLI: `suture bisect <good> <bad>`

#### XML Semantic Driver (Path B — Semantic Differentiator)
- New `suture-driver-xml` crate implementing `SutureDriver`
- Element-level XML diff using `roxmltree` DOM parser
- XPath-like paths: `/root/child[index]`, `/root/child[index]@attr`, `/root/child[index]#text`
- Detects Added, Removed, Modified changes for elements, attributes, and text
- Semantic merge: recursive three-way merge with conflict detection
- 9 tests: name, extensions, modified text, added element, removed element, attribute change, format diff, merge clean, merge conflict

#### YAML Semantic Merge (Path B — Semantic Differentiator)
- `YamlDriver::merge()` — three-way merge for YAML files
- Recursive merge of `serde_yaml::Value` mappings and sequences
- Auto-merges non-overlapping changes (additions, deletions, modifications to different parts)
- Detects conflicts when same key/element modified differently by both sides
- 5 new tests: no-conflict, conflict, both-add-different, both-add-same, nested merge

#### XML/YAML Drivers Wired Into CLI (Path B — Semantic Differentiator)
- `suture diff` now uses XML driver for `.xml` files automatically
- `suture diff` now uses YAML driver for `.yaml`/`.yml` files automatically
- `suture merge` attempts XML semantic merge for conflicting `.xml` files
- `suture merge` attempts YAML semantic merge for conflicting `.yaml`/`.yml` files
- `suture drivers` lists all 5 drivers: JSON, TOML, CSV, YAML, XML

#### End-to-End Integration Tests (Path C — Hardening)
- New `suture-e2e` crate with custom test harness
- 7 integration tests: init→commit→status, branch→merge, gc, fsck, bisect, tag, stash→pop
- Tests invoke `suture-cli` binary as subprocess against real repositories
- Gracefully skips if binaries not built

#### GitHub Release Workflow (Path C — Infrastructure)
- `.github/workflows/release.yml` — triggered on `v*` tag push
- Cross-compiles static binaries for Linux, macOS, Windows
- Creates GitHub Release with attached binaries (tar.gz / zip)

#### Quality
- Test count: 274 (up from 260 in v0.8.0)
- 14 new tests (9 XML driver, 5 YAML merge) + 7 e2e integration tests
- Zero clippy warnings, zero audit findings
- 14 workspace crates (up from 12)

## [0.8.0] - 2026-03-29

### Added

#### Semantic Merge Wiring (Path B — Semantic Differentiator)
- `suture merge` now attempts semantic merge via drivers for conflicting files
- Builds `DriverRegistry` with JSON, YAML, TOML, CSV drivers after conflict detection
- Retrieves clean base/ours/theirs content from CAS via conflict blob hashes
- If a driver resolves the merge, writes result to disk and stages it
- Reports count of semantically resolved vs remaining conflicts

#### TOML Driver (Path B — Semantic Differentiator)
- New `suture-driver-toml` crate implementing `SutureDriver`
- Key-level TOML diff using `toml::Value` recursive comparison
- Semantic merge: auto-merges non-overlapping key changes, detects conflicts
- 7 tests: name, extensions, modified, added, nested, merge clean, merge conflict

#### CSV Driver (Path B — Semantic Differentiator)
- New `suture-driver-csv` crate implementing `SutureDriver`
- Row/cell-level CSV diff with row:col addressing
- Header change detection, added/removed row tracking
- 5 tests: name, extensions, cell change, added row, removed row

#### `suture shortlog` (Path A — Git Parity)
- Groups commits by author using sorted output
- Shows commit count and first message per author
- `--branch` and `-n` (limit) flags

#### `suture tag --annotate` (Path A — Git Parity)
- `suture tag -a -m "message" <name>` — creates annotated tags with stored message
- Annotations stored as `tag.<name>.message` config entries
- `suture tag list` shows `(annotated)` marker and message for annotated tags
- Tag deletion cleans up annotation

#### `suture notes` (Path A — Git Parity)
- `suture notes add <commit> [-m message]` — attach a note to a commit
- `suture notes list <commit>` — list all notes for a commit
- `suture notes remove <commit> <index>` — remove a specific note
- Notes stored as `note.<hash>.<index>` config entries
- `add_note()`, `list_notes()`, `remove_note()` core methods

#### `suture version` (Path A — Git Parity)
- Prints version from `CARGO_PKG_VERSION`

#### README Rewrite (Path C — Documentation)
- Completely rewritten to reflect actual v0.8.0 state
- Honest architecture description (CAS, Patch DAG, Hub)
- Quick start guide, CLI reference table (32 commands)
- Driver SDK section with how-to-write guidance
- Honest "not yet implemented" section (VFS, Raft, SSO, Web UI)

#### Quality
- Test count: 260 (up from 248 in v0.7.0)
- 12 new tests (7 TOML, 5 CSV) + notes/shortlog/tag-annotate core tests
- Zero clippy warnings, zero audit findings
- 12 workspace crates (up from 10)

## [0.7.0] - 2026-03-29

### Added

#### `suture rm` (Path A — Git Parity)
- `suture rm <path> [paths...]` — remove files from working tree and staging area
- `suture rm --cached <path>` — remove from staging only, keep file on disk
- `add()` now handles missing tracked files by staging them as `FileStatus::Deleted`

#### `suture mv` (Path A — Git Parity)
- `suture mv <source> <dest>` — rename/move tracked files
- Moves file on disk, stages old path as Deleted and new path as Added
- `rename_file()` core method validates paths before moving

#### `suture remote remove` (Path A — Git Parity)
- `suture remote remove <name>` — delete a configured remote
- Cleans up associated `last_pushed` state automatically
- `delete_config()` method added to `MetadataStore`

#### Semantic Merge (Path B — Semantic Differentiator)
- `SutureDriver::merge()` trait method — three-way semantic merge
- Default implementation returns `Ok(None)` (fall back to line-level)
- `JsonDriver::merge()` — key-level JSON merge: auto-merges non-overlapping changes, detects conflicts
- 6 tests: no-conflict, conflict, both-add-different, both-add-same, nested, identical

#### YAML Driver (Path B — Semantic Differentiator)
- New `suture-driver-yaml` crate implementing `SutureDriver`
- Recursive YAML comparison using `serde_yaml::Value`
- `format_diff` with YAML-specific paths
- 5 tests: modified, added, nested, and format diff scenarios

#### `suture drivers` (Path B — CLI)
- `suture drivers` — lists all registered semantic drivers with their extensions
- Shows JSON and YAML drivers by default

#### Quality
- Test count: 248 (up from 232 in v0.6.0)
- 16 new tests (5 core: rm/mv/remote, 6 JSON merge, 5 YAML driver)
- Zero clippy warnings, zero audit findings
- 10 workspace crates (up from 9)

## [0.6.0] - 2026-03-29

### Added

#### SutureDriver Trait & Registry (Path B — Semantic Differentiator)
- New `suture-driver` crate with the `SutureDriver` trait
- `SutureDriver::diff()` — produces `SemanticChange` enum (Added/Removed/Modified/Moved)
- `SutureDriver::format_diff()` — human-readable semantic diff for a file type
- `DriverRegistry` — dispatches to drivers by file extension
- `DriverError`, `VisualDiff`, `DiffHunk`, `DiffSummary` supporting types

#### JSON Semantic Driver (Path B)
- New `suture-driver-json` crate implementing `SutureDriver`
- Recursive JSON comparison using RFC 6901 JSON Pointer paths
- Detects Added, Removed, Modified changes at key level
- `format_diff` shows semantic operations: `MODIFIED /name: "Alice" → "Bob"`
- 10 tests covering nested objects, arrays, new files, identical files

#### Semantic Diff in CLI (Path B)
- `suture diff` now uses JSON driver for `.json` files automatically
- Falls through to line-level diff for unsupported formats
- Shows key-level changes instead of raw line noise for JSON files

#### `suture show <ref>` (Path A — Git Parity)
- Display commit hash, author, timestamp, message, parents, changed files
- Supports branch names, tag names, full and partial commit hashes
- `resolve_ref` helper for ref resolution across all command contexts

#### `suture reflog` (Path A — Git Parity)
- `record_reflog()` tracks HEAD movements in config as JSON entries
- `reflog_entries()` retrieves history (newest first, capped at 100)
- Reflog recorded for: commit, checkout, reset, cherry-pick, rebase
- CLI: `suture reflog` displays `short_hash entry_string` per line

#### CI/CD (Path C — Hardening)
- `.forgejo/workflows/ci.yml` — Forgejo Actions workflow (build, test, clippy, fmt, audit)
- Uses `dtolnay/rust-toolchain` action, no Nix dependency in CI
- Mirrors existing `.github/workflows/ci.yml`

#### Infrastructure (Path C — Hardening)
- `rust-toolchain.toml` — pins stable channel for non-Nix users
- `.gitignore` updated: added `.direnv/`, `suture-e2e-*/`, `alice/`

#### Quality
- Test count: 232 (up from 222 in v0.5.0)
- 10 new JSON driver tests
- Zero clippy warnings, zero audit findings
- 9 workspace crates (up from 7)

## [0.5.0] - 2026-03-29

### Added

#### `-C <path>` Global Flag
- `suture -C <path> <command>` — run any command as if started in a different directory
- Global flag applies to all subcommands (except `init` and `clone` which take their own path)

#### Cherry-Pick
- `cherry_pick(&mut self, patch_id)` — apply a specific commit onto current HEAD
- Creates a new patch with the same content but current HEAD as parent
- Skips identity, merge, and create patches (not cherry-pickable)
- CLI: `suture cherry-pick <commit-hash>`
- Bug fix: capture `old_tree` before branch update for correct working tree sync

#### Rebase
- `rebase(&mut self, target_branch)` — replay commits from current branch onto target
- Finds unique commits via LCA (Lowest Common Ancestor)
- Supports fast-forward when current branch is ancestor of target
- Returns `RebaseResult` with replay count and new tip ID
- CLI: `suture rebase <branch>`
- Bug fix: capture `old_tree` before branch update for correct working tree sync

#### Blame
- `blame(&self, path)` — per-line commit attribution for a file
- Walks patch chain tracking line-level modifications via LCS diff
- Returns `Vec<BlameEntry>` with patch_id, message, author, line content, line number
- CLI: `suture blame <file>` — displays `line_num | hash (author) content`

#### Log Filtering
- `suture log --oneline` — compact format (short hash + message)
- `suture log --author=<name>` — filter commits by author
- `suture log --grep=<pattern>` — filter commits by message substring (case-insensitive)
- Filters compose with `--graph` (graph mode falls back to filtered non-graph when filters active)

#### Quality
- Test count: 222 (up from 216 in v0.4.0)
- 6 new tests: cherry-pick (2), rebase (2), blame (2)
- Zero clippy warnings, zero audit findings
- Bug fixes in cherry-pick and rebase: working tree sync now captures old snapshot before branch update

## [0.4.0] - 2026-03-28

### Added

#### Human-Readable Diff Output
- `suture diff` now shows line-level content with `+`/`-` prefixes
- ANSI color output (green for additions, red for deletions, bold cyan for headers)
- `diff --git a/<path> b/<path>` headers and `@@ hunk @@` markers
- Added, Deleted, Modified, and Renamed files all display correctly
- Uses existing LCS-based `diff_lines` from the merge engine

#### Enhanced Status
- `suture status` now shows unstaged changes alongside staged changes
- "Unstaged changes:" section with modified, deleted, and untracked files
- Files modified after staging marked with `[staged+unstaged]`
- Walks repository directory and compares against HEAD tree

#### Clone Command
- `suture clone <url> [dir]` — bootstrap a repository from a remote Hub
- Creates target directory, initializes repo, adds "origin" remote, pulls patches
- Extracts directory name from URL when not specified

#### Fetch Command
- `suture fetch [remote]` — fetch patches from remote without updating working tree
- Updates DAG and metadata only (no working tree sync)
- Extracted shared `do_fetch`/`do_pull` helpers for code reuse

#### Reset Command
- `suture reset [--mode soft|mixed|hard] <ref>` — move HEAD to a different commit
- `--soft`: move branch pointer, keep staging and working tree
- `--mixed` (default): move branch pointer, clear staging, keep working tree
- `--hard`: move branch pointer, clear staging, restore working tree to target
- `ResetMode` enum exposed from `suture-core`

#### Quality
- Test count: 216 (up from 213 in v0.3.0)
- 3 new reset tests (soft, mixed, hard modes)
- Zero clippy warnings, zero audit findings

## [0.3.0] - 2026-03-28

### Added

#### Shell Completions
- `clap_complete` dependency for generating shell completion scripts
- CLI `completions` command: `suture completions bash|zsh|fish`

#### Log Graph
- `--graph` flag on `log` command shows ASCII branch topology
- Column-based rendering with merge lines and branch alignment
- Logical commit grouping (patches sharing message+timestamp grouped as one)
- Topological sort newest-first ordering
- Branch labels at tips with `*` marker for HEAD branch

#### Working Tree Safety
- `has_uncommitted_changes()` detects both staged and unstaged changes
- `checkout()` auto-stashes dirty working tree before switching, restores after
- Matches git behavior: dirty state is preserved across branch switches

#### Stash
- `stash_push(message)` — saves staged and unstaged changes as a stash entry
- `stash_pop()` — applies highest-index stash and removes it
- `stash_apply(index)` — applies stash without removing it
- `stash_list()` — lists all stash entries with message, branch, and HEAD
- `stash_drop(index)` — removes a stash entry
- Stash entries stored as config entries (`stash.{index}.{message,head_branch,head_id,files}`)
- CLI commands: `suture stash push [-m msg]`, `suture stash pop`, `suture stash apply <n>`, `suture stash list`, `suture stash drop <n>`

#### Quality
- Test count: 213 (up from 203 in v0.2.0)
- 9 new stash tests covering push/pop, list, drop, apply-keeps-entry, has_uncommitted_changes variants
- Zero clippy warnings, zero audit findings

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
- Checkout does not handle uncommitted working tree changes (only staged) → Fixed in v0.3.0 with auto-stash
