# Suture Architecture v5.3.1

## System Overview

Suture is a patch-based version control system with semantic merge capabilities. It understands the *structure* of files (JSON, YAML, CSV, XML, DOCX, XLSX, PPTX, OTIO, SQL, PDF, images, and more) and merges them intelligently using format-aware drivers rather than treating them as opaque text.

### High-Level Component Diagram

```
                           User Interface Layer
    ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐
    │suture-cli│  │suture-tui│  │suture-lsp│  │ VS Code  │  │ Tauri    │
    │(clap, 58 │  │(ratatui, │  │(tower-   │  │Extension │  │Desktop   │
    │ commands)│  │ 7 tabs)  │  │ lsp)     │  │(TS)      │  │(HTML UI) │
    └────┬─────┘  └────┬─────┘  └────┬─────┘  └────┬─────┘  └────┬─────┘
         │              │              │              │              │
         └──────────────┴──────┬───────┴──────────────┘              │
                               │                                     │
              ┌────────────────┼────────────────┐                     │
              │                │                │                     │
       ┌──────▼──────┐ ┌──────▼──────┐ ┌───────▼───────┐             │
       │ suture-core │ │suture-driver │ │suture-protocol│             │
       │ CAS, DAG,   │ │ trait +      │ │ wire format,  │             │
       │ patches,    │ │ registry     │ │ compression   │             │
       │ repo engine │         │ (18 drivers) │ │ delta encoding│             │
       └──────┬──────┘ └──────┬──────┘ └───────┬───────┘             │
              │               │                │                     │
       ┌──────▼──────┐       │                │                     │
       │suture-common│       │                │                     │
       │Hash, types  │       │                │                     │
       └─────────────┘       │                │                     │
              ┌──────────────┼────────────────┘                     │
              │              │                                      │
    ┌─────────▼─────────┐   │         ┌─────────────────────┐      │
    │  suture-platform  │   │         │    suture-hub       │◄─────┘
    │  Web UI, auth,    │   │         │ HTTP/gRPC server,  │
    │  billing, merge   │   │         │ SQLite, auth,       │
    │  API, orgs        │   │         │ webhooks, mirrors   │
    └─────────┬─────────┘   │         └──────────┬──────────┘
              │              │                    │
    ┌─────────▼─────────┐   │         ┌──────────▼──────────┐
    │  Stripe           │   │         │  suture-raft        │
    │  billing          │   │         │  consensus, election│
    │  webhooks         │   │         │  log replication    │
    └───────────────────┘   │         └──────────┬──────────┘
                            │                    │
         ┌──────────────────┼────────────────────┘
         │                  │
  ┌──────▼──────┐   ┌──────▼──────┐
  │ suture-s3   │   │suture-daemon│
  │ blob store  │   │ file watch, │
  │ AWS SigV4   │   │ auto-sync,  │
  │ MinIO compat│   │ SHM, FUSE   │
  └─────────────┘   └──────┬──────┘
                           │
                    ┌──────▼──────┐
                    │ suture-vfs  │
                    │ FUSE3 mount │
                    │ WebDAV      │
                    └─────────────┘
```

### Data Flow: Merge Request Through the System

```
 1. User edits files          2. suture add / commit
 ┌─────────────┐              ┌──────────────────┐
 │ Working     │──────────────▶│ Stage changes    │
 │ Tree        │              │ Create patch     │
 └─────────────┘              │ (BLAKE3 hash)    │
                              └────────┬─────────┘
                                       │
 3. Patch stored in CAS               ▼
 ┌──────────────────┐       ┌──────────────────┐
 │ BlobStore        │◀──────│ PatchDag         │
 │ .suture/objects/ │       │ Add node + edges │
 │ (content-addrs)  │       │ Acyclicity check │
 └──────────────────┘       └────────┬─────────┘
                                      │
 4. Push to remote (optional)        ▼
 ┌──────────────────┐       ┌──────────────────┐
 │ suture-hub       │◀──────│ suture-protocol  │
 │ HTTP/gRPC        │       │ Zstd compression │
 │ V2 handshake     │       │ Delta encoding   │
 └──────────────────┘       └──────────────────┘

 5. Merge
 ┌──────────────────┐
 │ suture merge     │
 │ feature-branch   │
 └────────┬─────────┘
          │
          ▼
 ┌──────────────────┐    ┌────────────────────┐
 │ Compute LCA      │───▶│ PatchDag::lca()    │
 │ (merge base)     │    │ Lowest Common      │
 │                  │    │ Ancestor            │
 └────────┬─────────┘    └────────────────────┘
          │
          ▼
 ┌──────────────────┐    ┌────────────────────┐
 │ Per-file merge   │───▶│ DriverRegistry     │
 │                  │    │ lookup by extension │
 └────────┬─────────┘    └────────────────────┘
          │
     ┌────┴────┐
     │         │
     ▼         ▼
 ┌───────┐ ┌───────┐
 │Driver │ │Text   │
 │exists?│ │merge  │
 │       │ │fallback│
 └───┬───┘ └───┬───┘
     │         │
     ▼         ▼
 ┌──────────────────┐
 │ Three-way merge  │
 │ driver.merge()   │
 │ or line-based    │
 └────────┬─────────┘
          │
     ┌────┴────┐
     │         │
     ▼         ▼
 ┌───────┐ ┌───────┐
 │Clean  │ │Conflict│
 │Merge  │ │Markers │
 └───────┘ └───────┘
```

### Deployment Topologies

#### Standalone (CLI Only)

```
┌──────────────────────────────┐
│ User Machine                 │
│  ┌────────┐  ┌────────────┐  │
│  │suture  │──│ .suture/   │  │
│  │cli     │  │ objects/   │  │
│  │        │  │ metadata.db│  │
│  └────────┘  └────────────┘  │
└──────────────────────────────┘
```

No server required. All operations are local. 17 semantic drivers available.

#### Hub + Clients

```
┌──────────────┐     HTTP/gRPC      ┌──────────────────┐
│ Client A     │◀──────────────────▶│ suture-hub       │
│ (CLI/TUI)    │                    │ SQLite + Ed25519 │
└──────────────┘                    │ auth, webhooks   │
                                    └────────┬─────────┘
┌──────────────┐     HTTP/gRPC               │
│ Client B     │◀────────────────────────────┘
│ (CLI/TUI)    │
└──────────────┘

Optional: suture-s3 for blob storage (replaces SQLite blobs)
Optional: suture-raft for HA clustering (3+ nodes)
```

#### Full Platform (SaaS)

```
┌──────────────┐     HTTPS          ┌──────────────────┐
│ Client       │◀──────────────────▶│ suture-platform  │
│ (Web UI /    │                    │ Axum + SQLite    │
│  REST API)   │                    │ JWT auth, Stripe │
└──────────────┘                    │ billing, orgs    │
                                    └────────┬─────────┘
┌──────────────┐     HTTPS                  │
│ VS Code /    │◀───────────────────────────┘
│ JetBrains /  │
│ Neovim       │
└──────────────┘

Billing tiers: Free (100 merges/mo), Pro ($9/mo), Enterprise ($29/mo)
Analytics available on Pro+
WASM plugin uploads on Enterprise
```

## Core Components

### suture-common

**Path:** `crates/suture-common/`
**Tests:** 8

Foundation types shared across all crates. Zero dependencies.

| Type | Description |
|------|-------------|
| `Hash` | BLAKE3 256-bit content hash (32 bytes, 64 hex chars) |
| `PatchId` | Alias for `Hash` — identifies patches by content |
| `BranchName` | Validated branch name (non-empty, no null bytes) |
| `RepoPath` | Validated repository path (rejects `..`, absolute paths, null bytes) |

Key operations:
- `Hash::from_data(data)` — compute BLAKE3 hash
- `Hash::from_hex(str)` / `to_hex()` — hex encoding/decoding
- `Hash::ZERO` — sentinel value (all zeros)

---

### suture-core

**Path:** `crates/suture-core/`
**Tests:** 298
**Depends on:** suture-common

The core engine coordinating four subsystems:

| Subsystem | File | Description |
|-----------|------|-------------|
| **BlobStore** | `src/cas/` | Content-addressable storage on disk. 2-char prefix directory scheme (256 buckets). Optional Zstd compression (level 3). Deduplication by hash. |
| **PatchDag** | `src/dag/graph.rs` | In-memory DAG of patch history. Ancestor/descendant queries, LCA computation, branch management. Reconstructed from SQLite on `Repository::open()`. |
| **MetadataStore** | `src/metadata/` | SQLite-backed persistent metadata. Stores patches, branches, configuration. |
| **Patch Engine** | `src/repository/repo_impl.rs` | Orchestrates CAS, DAG, metadata. Handles commit, merge, stash, reset, cherry-pick, rebase, blame, reflog, notes, GC, fsck, squash. |

Additional capabilities:
- **Audit Log** (`src/audit.rs`): Append-only JSONL with BLAKE3 hash chaining. Each entry contains hash of previous entry. Optional Ed25519 signatures.
- **Signing** (`src/signing/`): Ed25519 key generation, signing, verification for push operations.
- **File-type Detection**: 14 file types with `auto_detect_repo_type()`.
- **Semantic Diff Formatter**: File-type-aware diff output with driver labels.
- **Supply Chain Integrity**: Shannon entropy analysis, 13 risk indicators, XZ-style attack detection.
- **Conflict Classification**: Categorize merge conflicts by type.
- **Stash/RM/MV/Notes/GC/fsck/Squash/Patch composition**: Full VCS lifecycle.

---

### suture-driver

**Path:** `crates/suture-driver/`
**Tests:** 8
**Depends on:** suture-core, suture-common

Defines the `SutureDriver` trait and driver registry:

```rust
pub trait SutureDriver: Send + Sync {
    fn name(&self) -> &str;
    fn supported_extensions(&self) -> &[&str];
    fn diff(&self, base: Option<&str>, new: &str) -> Result<Vec<SemanticChange>, DriverError>;
    fn format_diff(&self, base: Option<&str>, new: &str) -> Result<String, DriverError>;
    fn merge(&self, base: &str, ours: &str, theirs: &str) -> Result<Option<String>, DriverError>;
    fn diff_raw(&self, base: &[u8], new: &[u8]) -> Result<Vec<SemanticChange>, DriverError> { ... }
    fn merge_raw(&self, base: &[u8], ours: &[u8], theirs: &[u8]) -> Result<Option<Vec<u8>>, DriverError> { ... }
}
```

- `diff()` / `diff_raw()` produce `SemanticChange` entries (`Added`, `Removed`, `Modified`, `Moved`)
- `merge()` / `merge_raw()` return `Some(merged)` for clean merges, `None` for conflicts
- `format_diff()` produces human-readable diff output

**DriverRegistry** (`src/registry.rs`): Dispatches to the correct driver by file extension. The CLI builds a `builtin_registry()` registering all 17 drivers.

**SemanticChange types:**

```rust
pub enum SemanticChange {
    Added { path: String, value: String },
    Removed { path: String, old_value: String },
    Modified { path: String, old_value: String, new_value: String },
    Moved { old_path: String, new_path: String, value: String },
}
```

#### Builtin Drivers (17)

| Driver | Crate | Extensions | Tests |
|--------|-------|------------|-------|
| JSON | `suture-driver-json` | `.json` | 47 |
| YAML | `suture-driver-yaml` | `.yaml`, `.yml` | 30 |
| TOML | `suture-driver-toml` | `.toml` | 30 |
| CSV | `suture-driver-csv` | `.csv` | 27 |
| XML | `suture-driver-xml` | `.xml` | 31 |
| Markdown | `suture-driver-markdown` | `.md`, `.markdown`, `.mdown`, `.mkd` | 41 |
| SQL | `suture-driver-sql` | `.sql` | 18 |
| DOCX | `suture-driver-docx` | `.docx` | 13 |
| XLSX | `suture-driver-xlsx` | `.xlsx` | 13 |
| PPTX | `suture-driver-pptx` | `.pptx` | 19 |
| OTIO | `suture-driver-otio` | `.otio` | 21 |
| PDF | `suture-driver-pdf` | `.pdf` | 12 |
| Image | `suture-driver-image` | `.png`, `.jpg`, `.jpeg`, `.gif`, `.bmp`, `.webp`, `.tiff`, `.ico`, `.avif` | 12 |
| HTML | `suture-driver-html` | `.html`, `.htm` | — |
| SVG | `suture-driver-svg` | `.svg` | — |
| Feed | `suture-driver-feed` | `.rss`, `.atom` | — |
| iCal | `suture-driver-ical` | `.ics`, `.ifb` | — |

Supporting crate: **suture-ooxml** — shared OOXML infrastructure (ZIP parsing, part navigation, per-part relationship resolution).

---

### suture-hub

**Path:** `crates/suture-hub/`
**Tests:** 61 (with features)
**Depends on:** suture-common, suture-core, suture-protocol

Central server for hosting Suture repositories.

**Transport layers:**

| Layer | Technology | Description |
|-------|-----------|-------------|
| HTTP | Axum | REST API, middleware, auth, web UI |
| gRPC | Tonic + Prost | Bidirectional streaming (14 RPCs) |
| Storage | SQLite | Repository metadata and blob storage |

**HTTP Routes (key endpoints):**

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/healthz` | No | Health check |
| GET/POST | `/handshake` | No | V1 handshake |
| GET/POST | `/v2/handshake` | No | V2 with capability negotiation |
| POST | `/v2/push` | No | Push patches + blobs |
| POST | `/v2/pull` | No | Pull patches + blobs |
| GET | `/repos` | No | List repositories |
| POST | `/repos` | Auth | Create repository |
| GET/DELETE | `/repo/{repo_id}` | Auth | Get/delete repo |
| POST | `/auth/register` | No | Register user |
| POST | `/auth/login` | No | Login, get Ed25519 token |
| POST | `/auth/token` | Auth | Create auth token |
| POST | `/auth/verify` | No | Verify auth token |
| GET | `/search` | No | Cross-repo search |
| POST | `/lfs/batch` | Auth | LFS batch upload/download |
| POST | `/webhooks` | Auth | Create webhook |
| POST/GET | `/mirror/setup|sync|status` | Auth | Mirror management |
| POST/GET | `/replication/peers` | Auth | Raft peer management |

**gRPC Service (`SutureHub`):**

Defined in `crates/suture-hub/proto/suture.proto`. 14 RPCs:

`Handshake`, `ListRepos`, `GetRepoInfo`, `CreateRepo`, `DeleteRepo`, `ListBranches`, `CreateBranch`, `DeleteBranch`, `ListPatches`, `GetBlob`, `Push`, `Pull`, `GetTree`, `Search`

**Features:**

- **Auth**: Ed25519 key-based authentication
- **Webhooks**: Event-driven notifications (push/branch events) with HMAC-SHA256 signing
- **S3 Backend** (feature `s3-backend`): Blob storage via `suture-s3` (AWS SigV4, MinIO compatible)
- **Raft Clustering** (feature `raft-cluster`): Consensus via `suture-raft` for HA deployments
- **Mirrors**: Repository mirroring support
- **Pagination**: Cursor-based pagination for large result sets
- **LFS**: Large file storage via batch API

---

### suture-platform

**Path:** `crates/suture-platform/`
**Depends on:** suture-driver (all 17), suture-wasm-plugin

SaaS platform with web UI, authentication, billing, organizations, and merge API.

**Architecture:**

- **Axum** web server with tower-http tracing
- **SQLite** (WAL mode) via `PlatformDb` with `Mutex<Connection>`
- **JWT** authentication (HS256, 7-day expiry, revocation support)
- **Rate limiting**: In-memory sliding window per user/IP, tier-based limits
- **CORS**: Permissive (configurable for production)
- **Stripe**: Checkout sessions, portal, webhook handling with HMAC-SHA256 verification
- **OAuth**: Google and GitHub (CSRF state tokens, one-time use)
- **WASM plugins**: Runtime plugin loading and merge execution

See `docs/api-reference.md` for full endpoint documentation.

---

### suture-raft

**Path:** `crates/suture-raft/`
**Tests:** 30

Raft consensus protocol implementation for hub clustering.

**Key components:**

| Component | Description |
|-----------|-------------|
| `RaftNode` | Raft node state machine (leader, follower, candidate) |
| `RaftTcpTransport` | TCP transport with 4-byte BE length + JSON wire format |
| `RaftRuntime` | Background tick loop, propose/apply channels, leader tracking |
| `SqliteRaftLog` | Persisted Raft log (gated on `persist` feature) |
| `HubCommand` | Command enum for state machine replication |

**Protocol details:**

- Randomized election timeouts (Raft paper section 5.2) to prevent split-vote livelock
- Leader election, log replication, commit
- Multi-node 3-cluster integration test over real TCP
- 3 Raft integration tests + 12 hub-Raft tests

---

### suture-vfs

**Path:** `crates/suture-vfs/`
**Tests:** 28 (2 ignored — require root)

Exposes a Suture repository as a filesystem.

| Feature | Technology | Description |
|---------|-----------|-------------|
| FUSE3 mount | `fuse3` + Tokio | Read/write access to repo as filesystem |
| WebDAV server | Axum | HTTP-based file access (macOS Finder, Windows Explorer) |
| Path translation | Custom | Maps between filesystem paths and repository paths |
| Inode allocation | Custom | Stable inode assignment for FUSE operations |

Integration tests (require root, `#[ignore]`d): mount read/write/modify/delete/stat, WebDAV serve.

---

### suture-wasm-plugin

**Path:** `crates/suture-wasm-plugin/`

WASM plugin runtime for loading custom merge drivers at runtime.

**ABI:**

- Version 1 ABI with version checking
- `PluginManager`: Load, validate, list, and execute WASM plugins
- `validate_plugin()`: Pre-load validation of Wasm modules
- Plugin descriptor files (`.suture-plugin`) for discovery
- `discover_plugins(dir)`: Scan a directory for plugin descriptors

**Integration with platform:**

- Plugins loaded at startup from `plugins/` directory
- Enterprise-tier users can upload plugins via REST API
- `merge_with_plugin()` endpoint dispatches to loaded WASM drivers

---

### suture-cli

**Path:** `crates/suture-cli/`
**Tests:** 32

Command-line interface using [clap](https://docs.rs/clap) with derive macros.

**58 subcommands** organized in `src/cmd/` (one file per command).

Key commands:

| Command | Description |
|---------|-------------|
| `init` | Initialize repository (with `--type` and `--template` support) |
| `add` | Stage files (recursive directory expansion) |
| `commit` | Create patch from staged changes |
| `merge` | Merge branches with semantic drivers (`-s ours|theirs|manual|semantic`, `--dry-run`) |
| `diff` | Show semantic or line diffs (`--integrity`, `--classification`, `--summary`, `--name-only`) |
| `log` | View patch history (`--stat`, `--diff`, `--audit`, `--graph`) |
| `push` / `pull` | Remote sync via HTTP protocol |
| `stash` | Stash and restore changes |
| `rebase` | Rebase patches onto new base |
| `bisect` | Binary search for bug-introducing commit |
| `tui` | Launch terminal UI |
| `sync` | Auto-commit, pull, push (Google Drive replacement) |
| `doctor` | Health checks (`--fix` auto-remediation) |
| `export` | Export clean snapshot |
| `undo` | Reflog-aware undo |
| `verify` | Ed25519 signature verification |
| `blame` | Per-line blame with range filtering (`-L`) |
| `grep` | Search tracked content with regex |
| `switch` / `restore` | Modern alternatives to checkout |
| `archive` | Export as tar.gz/tar/zip |
| `ls-remote` | List remote branches without cloning |
| `timeline` | OTIO import/export/summary/diff |
| `report` | Batch operations, export templates |
| `git import` | Read-only Git history import |
| `key generate` | Ed25519 key generation |
| `hook list/run/edit` | Hook management |

Supporting modules:

| Module | Description |
|--------|-------------|
| `driver_registry.rs` | Builds `DriverRegistry` with all 17 builtin drivers |
| `display.rs` | Terminal output formatting and colors |
| `fuzzy.rs` | Fuzzy matching for branch/tag/patch selection |
| `remote_proto.rs` | HTTP-based remote protocol implementation |
| `ref_utils.rs` | Reference resolution utilities |
| `style.rs` | CLI styling helpers |

---

### suture-tui

**Path:** `crates/suture-tui/`
**Tests:** 31

Terminal UI built with [ratatui](https://github.com/ratatui/ratatui).

**7 tabs:**

| Tab | Description |
|-----|-------------|
| Status | Repository status, staged/untracked files |
| Log | Commit history with graph |
| Branch | Branch management |
| Remote | Remote operations |
| Conflict | Merge conflict resolution |
| Diff | File diffs |
| Help | Key bindings and documentation |

**Conflict resolver:**

- Line-by-line hunk resolution (replaces binary ours/theirs-only choice)
- `1`/`2`/`3` keys: take ours, theirs, or both per hunk
- `j`/`k` keys: navigate between conflict hunks
- `n`/`p` keys: next/previous conflict file
- `a` key: accept all remaining with last used resolution
- Three-panel layout: file list, hunk detail, key bindings footer

---

### suture-lsp

**Path:** `crates/suture-lsp/`
**Tests:** 11

Language Server Protocol implementation via [tower-lsp](https://github.com/ebkalderon/tower-lsp).

Capabilities:
- Hover information for patches
- Diagnostics for merge conflicts
- Workspace-aware repository operations
- Activates for `.suture` directories

---

### suture-daemon

**Path:** `crates/suture-daemon/`
**Tests:** 33

Background daemon for continuous operations.

| Feature | Implementation |
|---------|---------------|
| File watcher | `notify` crate for working tree change detection |
| Shared memory | `memmap2`-based SHM for inter-process state |
| Mount manager | FUSE/WebDAV lifecycle management |
| Auto-sync | Automatic push/pull to configured remotes |
| PID management | PID file management, signal handling (SIGTERM, SIGHUP) |
| Log rotation | `humantime`-based timestamps |

---

### Other Crates

| Crate | Description |
|-------|-------------|
| `suture-protocol` | Binary wire protocol with V2 handshake, Zstd compression, delta encoding |
| `suture-merge` | Standalone merge library (published to crates.io as `suture-merge` v0.2) |
| `suture-s3` | S3-compatible blob storage (AWS SigV4, path/virtual-hosted, MinIO) |
| `suture-ooxml` | Shared OOXML infrastructure for DOCX/XLSX/PPTX drivers |
| `suture-node` | Node.js native addon via napi-rs |
| `suture-e2e` | End-to-end workflow tests (226 tests) |
| `suture-fuzz` | Fuzz testing (6 targets: CAS hash, patch serialization, merge, touch-set) |
| `suture-bench` | Criterion benchmarks (44 functions) |
| `suture-py` | Python bindings via PyO3 |

## Data Model

### Repository

The `Repository` is the top-level object (`suture-core/src/repository/`). It coordinates:

```
Repository
├── BlobStore (CAS)
│   └── .suture/objects/{2-char-prefix}/{62-char-hash}
├── PatchDag (in-memory)
│   └── Nodes: patch_id → [parent_ids], branches: name → tip_patch_id
├── MetadataStore (SQLite)
│   └── .suture/metadata.db
│       ├── patches table (serialized JSON)
│       ├── branches table (name → tip)
│       └── config key-value pairs
├── Working Set
│   └── Currently checked-out files, staged changes
└── Audit Log
    └── .suture/audit.jsonl (BLAKE3 hash chain)
```

### Patch

The fundamental unit of change (`suture-core/src/patch/types.rs`):

| Field | Type | Description |
|-------|------|-------------|
| `id` | `Hash` (BLAKE3) | Content-addressed identifier |
| `parent_ids` | `Vec<Hash>` | Parent patches (1 for linear, 2 for merge) |
| `operation_type` | `OperationType` | `Create`, `Delete`, `Modify`, `Move`, `Merge`, `Batch`, `Identity` |
| `touch_set` | `TouchSet` | Set of file paths this patch modifies |
| `target_path` | `Option<String>` | Target file path |
| `payload` | `Vec<u8>` | Serialized operation data |
| `timestamp` | `i64` | Unix epoch seconds |
| `author` | `String` | Author identifier |
| `message` | `String` | Human-readable description |

The `Batch` operation type groups multiple `FileChange` entries into a single commit (standard CLI commit path).

### FileChange

Within a `Batch` patch:

| Field | Type | Description |
|-------|------|-------------|
| `path` | `String` | Repository-relative file path |
| `content_hash` | `Hash` | BLAKE3 hash of file content (stored in CAS) |
| `operation` | `FileOp` | `Add`, `Modify`, `Delete` |

### Touch Set and Commutativity

Touch sets are the basis for commutativity detection. Two patches **commute** (can be applied in either order) if and only if their touch sets are disjoint. This enables conflict detection without full content comparison.

### Branch

A branch maps a `BranchName` to a tip `PatchId` in the DAG.

### Storage Layout

```
.suture/
├── objects/        # CAS blob storage (BLAKE3-indexed)
│   └── ab/         # 2-char hex prefix (256 buckets)
│       └── cdef... # Remaining 62 hex chars = blob filename
├── metadata.db     # SQLite: patches, branches, config, refs
├── HEAD            # Current branch reference
├── hooks/          # Executable hook scripts
├── keys/           # Ed25519 signing keys
├── config          # Per-repo TOML configuration
├── audit.jsonl     # Tamper-evident audit log
└── worktree        # Marker file for worktree checkouts
```

## Merge Algorithm

Suture implements three-way merge with format-aware conflict detection:

### Step-by-Step

1. **Compute merge base**: `PatchDag::lca()` finds the Lowest Common Ancestor of the two branch tips
2. **Reconstruct file trees**: Apply patches from merge base to each branch tip, obtaining three versions per file: `base`, `ours`, `theirs`
3. **Per-file driver lookup**: Check if a semantic driver exists for the file extension via `DriverRegistry`
4. **Driver present**: Call `driver.merge(base, ours, theirs)` (or `merge_raw()` for binary formats)
5. **No driver**: Fall back to line-based text merge with conflict markers
6. **Conflict handling for binary formats** (DOCX/XLSX/PPTX): Preserve "ours" version, generate `.suture/conflicts/report.md`

### Semantic Merge (Format-Aware)

When a driver is available:

1. **Parse all three versions** (base, ours, theirs) using the format-specific driver
2. **Compute structural diff** using key-path based changes (not line-based)
3. **Classify changes**:
   - **Same**: No change between base and either branch
   - **Ours-only**: Changed in ours only → take ours
   - **Theirs-only**: Changed in theirs only → take theirs
   - **Both-changed-same**: Both changed identically → take either (no conflict)
   - **Both-changed-different**: Both changed differently → **conflict**
4. **Auto-resolve**: Take changed version for ours-only and theirs-only
5. **Detect conflicts**: Both-changed-different entries produce conflicts
6. **Return**: `Some(merged_content)` for clean merge, `None` for conflicts

### Merge Strategies

| Strategy | Flag | Behavior |
|----------|------|----------|
| Semantic | `-s semantic` (default) | Try semantic drivers first, fall back to text merge |
| Ours | `-s ours` | Keep our version for all conflicts |
| Theirs | `-s theirs` | Take their version for all conflicts |
| Manual | `-s manual` | Leave all conflicts for manual resolution |

### Dry Run

`--dry-run` previews the merge result without modifying the working tree. OOXML conflict handling is noted in the output.

## API Reference

### Platform REST API

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/healthz` | No | Health check |
| GET | `/` | No | Web UI index |
| GET | `/static/{*path}` | No | Static assets |
| POST | `/auth/register` | No | Create account |
| POST | `/auth/login` | No | Login, get JWT |
| GET | `/auth/me` | Optional | Current user info |
| POST | `/auth/logout` | Optional | Revoke token |
| GET | `/auth/oauth/start` | No | Start OAuth flow |
| GET | `/auth/google/callback` | No | Google OAuth callback |
| GET | `/auth/github/callback` | No | GitHub OAuth callback |
| POST | `/api/merge` | Optional | Three-way semantic merge |
| GET | `/api/drivers` | No | List supported drivers |
| GET | `/api/usage` | Optional | Current usage/limits |
| GET | `/api/analytics` | Optional | Usage analytics (Pro+) |
| POST | `/api/orgs` | Optional | Create org |
| GET | `/api/orgs` | Optional | List user's orgs |
| POST | `/api/orgs/{org_id}/invite` | Optional | Invite member |
| GET | `/api/orgs/{org_id}/members` | Optional | List members |
| PUT | `/api/orgs/{org_id}/members/{user_id}/role` | Optional | Update role |
| DELETE | `/api/orgs/{org_id}/members/{user_id}` | Optional | Remove member |
| GET | `/api/invitations` | Optional | List invitations |
| POST | `/api/invitations/{invite_id}/accept` | Optional | Accept invitation |
| GET | `/api/plugins` | No | List loaded plugins |
| POST | `/api/plugins/upload` | Enterprise | Upload WASM plugin |
| POST | `/api/plugins/merge` | Optional | Merge with plugin |
| POST | `/billing/checkout` | Optional | Create checkout session |
| POST | `/billing/portal` | Optional | Customer portal |
| GET | `/billing/subscription` | Optional | Get subscription info |
| POST | `/billing/webhook` | No | Stripe webhook |
| GET | `/admin/users` | Admin | List all users |

### Hub REST API

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/healthz` | No | Health check |
| GET/POST | `/handshake` | No | V1 version/capability negotiation |
| GET/POST | `/v2/handshake` | No | V2 with extended capabilities |
| POST | `/v2/push` | No | Push patches and blobs |
| POST | `/v2/pull` | No | Pull patches and blobs |
| GET | `/repos` | No | List repositories |
| POST | `/repos` | Auth | Create repository |
| GET | `/repo/{repo_id}` | Auth | Get repo info |
| DELETE | `/repo/{repo_id}` | Auth | Delete repository |
| POST | `/auth/register` | No | Register user |
| POST | `/auth/login` | No | Login (Ed25519) |
| POST | `/auth/token` | Auth | Create auth token |
| POST | `/auth/verify` | No | Verify auth token |
| GET | `/search` | No | Cross-repo search |
| GET | `/activity` | No | Activity feed |
| POST | `/lfs/batch` | Auth | LFS batch operations |
| POST | `/webhooks` | Auth | Create webhook |
| POST/GET | `/mirror/setup|sync|status` | Auth | Mirror management |
| POST/GET/DELETE | `/replication/peers` | Auth | Raft peer management |

### gRPC Service (Hub)

`SutureHub` service defined in `crates/suture-hub/proto/suture.proto`:

`Handshake`, `ListRepos`, `GetRepoInfo`, `CreateRepo`, `DeleteRepo`, `ListBranches`, `CreateBranch`, `DeleteBranch`, `ListPatches`, `GetBlob`, `Push`, `Pull`, `GetTree`, `Search`

### Wire Protocol (suture-protocol)

Binary protocol with Zstd compression for CLI-to-Hub communication:

- V2 handshake with capability negotiation
- Delta encoding for efficient transfer
- Batch operations for push/pull

## Security Model

### Authentication

**Platform (JWT):**

- HS256 JWT with configurable secret (`SUTURE_JWT_SECRET`)
- 7-day session duration
- Token revocation via `revoked_tokens` table (checked on every authenticated request)
- Revoked tokens auto-cleaned on verification

**Hub (Ed25519):**

- Ed25519 key-based authentication
- Token creation and verification endpoints
- Key generation via `suture key generate`

**OAuth (Google, GitHub):**

- Authorization code flow with PKCE-style state tokens
- CSRF protection: UUID v4 state stored in `oauth_states` table with 10-minute expiry
- State is one-time use (deleted after validation)
- Provider identity stored as `oauth:<provider_id>` marker in password_hash field

### Authorization

**Tier-based rate limiting** (per-minute sliding window):

| Tier | Rate Limit | Merge Limit/mo | Storage | Repos | Price |
|------|-----------|----------------|---------|-------|-------|
| Free | 30 req/min | 100 | 100 MB | 5 | $0 |
| Pro | 300 req/min | 10,000 | 10 GB | Unlimited | $9/seat |
| Enterprise | 3,000 req/min | Unlimited | 100 GB | Unlimited | $29/seat |

Rate limit headers: `X-RateLimit-Limit`, `X-RateLimit-Remaining`, `X-RateLimit-Reset`, `Retry-After`

Anonymous requests: 10 req/min per IP (uses `X-Forwarded-For` header)

### Stripe Integration

- **Webhook HMAC verification**: Stripe-Signature header parsed for `t=<timestamp>,v1=<signature>`. HMAC-SHA256 computed over `timestamp.payload`. Timestamp must be within 300 seconds to prevent replay attacks.
- **Customer management**: Auto-create Stripe customer on first checkout. Customer ID stored in accounts table.
- **Subscription lifecycle**: Handles `checkout.session.completed`, `customer.subscription.updated`, `customer.subscription.deleted`, `invoice.payment_failed`
- **Grace period**: 7-day grace period on payment failure before downgrade
- **Price IDs**: Configured via `STRIPE_PRICE_PRO` and `STRIPE_PRICE_ENTERPRISE` env vars

### Self-Hosted (Hub)

- Ed25519 token-based auth
- Per-IP rate limiting
- Repository-level access control
- Webhook delivery with HMAC-SHA256 signing
- Branch protection rules

### Input Validation

- `RepoPath::new()` rejects `..`, absolute paths, null bytes (path traversal protection)
- `BranchName::new()` validates non-empty, no null bytes
- Email validation: must contain `@`, max 254 characters
- Password: minimum 8 characters
- Org names: 2-39 alphanumeric characters (hyphens and underscores allowed)
- Blob size limits: 50MB max (`max_blob_size`)
- Page size limits: 10K max (`max_page_size`)

### Audit Trail

- Append-only JSONL with BLAKE3 hash chaining
- Optional Ed25519 signatures for non-repudiation
- `suture log --audit` for compliance reporting
- Export formats: JSON, CSV, text

## Error Handling

All crates use `thiserror` for error types:

| Error Type | Source | Description |
|-----------|--------|-------------|
| `RepoError` | suture-core | Repository operations (init, commit, merge) |
| `CasError` | suture-core | Content-addressable storage |
| `DagError` | suture-core | DAG operations (cycle detection, missing patches) |
| `DriverError` | suture-driver | Driver parsing and merge errors |
| `MetaError` | suture-core | SQLite metadata operations |
| `MergeError` | suture-core | Merge results and failures |
| `OrgError` | suture-platform | Organization management errors |

## Dependency Graph

```
suture-common (no deps)
    ↑
suture-core → suture-common
    ↑
suture-driver → suture-core, suture-common
    ↑
suture-merge → suture-driver
suture-ooxml → suture-common
suture-driver-{docx,xlsx,pptx} → suture-driver, suture-ooxml
suture-driver-{json,yaml,toml,csv,xml,...} → suture-driver
suture-protocol → suture-common
suture-cli → suture-core, suture-common, suture-driver, suture-merge, all drivers
suture-hub → suture-common, suture-core, suture-protocol
suture-raft (standalone)
suture-s3 (standalone)
suture-platform → suture-driver (all 17), suture-wasm-plugin
suture-vfs → suture-core
suture-tui → suture-core
suture-lsp → suture-core
suture-daemon → suture-core
suture-wasm-plugin (standalone)
```

Total: 37 crates in workspace (2 excluded: suture-py, desktop-app).
