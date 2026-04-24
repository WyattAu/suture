# Suture Architecture

## Overview

Suture is a patch-based version control system with semantic merge capabilities. It understands the *structure* of files (JSON, YAML, CSV, XML, DOCX, XLSX, PPTX, etc.) and can merge them intelligently rather than treating them as opaque text.

```
                         ┌─────────────────────┐
                         │     suture-cli      │
                         │   (58 subcommands)  │
                         └─────────┬───────────┘
                                   │
              ┌────────────────────┼────────────────────┐
              │                    │                    │
     ┌────────▼────────┐  ┌───────▼───────┐  ┌───────▼────────┐
     │   suture-core   │  │  suture-merge │  │  suture-protocol│
     │  (CAS, DAG,     │  │  (standalone  │  │  (wire format, │
     │   patches, repo)│  │  merge lib)   │  │   compression) │
     └───────┬─────────┘  └───────┬───────┘  └───────┬────────┘
             │                    │                   │
     ┌───────▼───────┐  ┌────────▼──────────┐        │
     │ suture-common │  │   suture-driver   │        │
     │ (Hash, types) │  │ (trait + registry)│        │
     └───────────────┘  └───────┬───────────┘        │
                                │                    │
          ┌─────────────────────┼──────────┐         │
          │         │           │          │         │
     ┌────▼──┐ ┌────▼──┐ ┌─────▼──┐ ┌─────▼──┐     │
     │ JSON  │ │ YAML  │ │ OOXML  │ │ ...17  │     │
     │ driver│ │ driver│ │ drivers│ │ drivers│     │
     └───────┘ └───────┘ └────────┘ └────────┘     │
                                                      │
     ┌──────────────────────────────────────────────┐ │
     │               Remote / Sync                  │◄┘
     │  ┌──────────┐  ┌──────────┐  ┌───────────┐  │
     │  │ suture-  │  │ suture-  │  │ suture-   │  │
     │  │   hub    │  │   raft   │  │   s3      │  │
     │  │(HTTP/gRPC│  │(consensus│  │(blob store│  │
     │  │ SQLite)  │  │ cluster) │  │  SigV4)   │  │
     │  └──────────┘  └──────────┘  └───────────┘  │
     └──────────────────────────────────────────────┘

     ┌──────────────────────────────────────────────┐
     │              Client Ecosystem                │
     │  ┌──────────┐  ┌──────────┐  ┌───────────┐  │
     │  │ suture-  │  │ suture-  │  │ suture-   │  │
     │  │   tui    │  │   lsp    │  │   vfs     │  │
     │  │(ratatui) │  │(tower-   │  │(FUSE3 +   │  │
     │  │          │  │ lsp)     │  │ WebDAV)   │  │
     │  └──────────┘  └──────────┘  └───────────┘  │
     └──────────────────────────────────────────────┘
```

## Core Concepts

### Repository

The `Repository` is the top-level object (`suture-core/src/repository/`). It coordinates four subsystems:

- **BlobStore** (CAS) — content-addressed blob storage on disk
- **PatchDag** — in-memory directed acyclic graph of patch history
- **MetadataStore** — SQLite-backed persistent metadata
- **Patch Application Engine** — reconstructs file trees from patch chains

Defined in `suture-core/src/repository/repo_impl.rs`.

### Patch

A `Patch` is the fundamental unit of change (`suture-core/src/patch/types.rs`). Each patch contains:

| Field | Description |
|-------|-------------|
| `id` | BLAKE3 hash of patch content (content-addressed) |
| `parent_ids` | Parent patches (one for linear, two for merges) |
| `operation_type` | `Create`, `Delete`, `Modify`, `Move`, `Merge`, `Batch`, `Identity` |
| `touch_set` | Set of file paths this patch modifies |
| `target_path` | Optional target file path |
| `payload` | Serialized operation data (file content or metadata) |
| `timestamp` | Unix epoch seconds |
| `author` | Author identifier |
| `message` | Human-readable description |

The `Batch` operation type groups multiple `FileChange` entries into a single commit, which is the standard commit path used by the CLI.

### Touch Set and Commutativity

Touch sets are the basis for commutativity detection. Two patches **commute** (can be applied in either order) if and only if their touch sets are disjoint. This enables conflict detection without full content comparison.

### PatchDag

The `PatchDag` (`suture-core/src/dag/graph.rs`) is an in-memory directed acyclic graph. It provides:

- Adding patches with parent edges and cycle detection
- Ancestor/descendant queries
- Lowest Common Ancestor (LCA) computation for merge-base detection
- Branch creation, deletion, and lookup

Branches map to tip patch IDs. The DAG is reconstructed from SQLite metadata on `Repository::open()`.

### Hash

BLAKE3 content-addressable hashing (`suture-common/src/lib.rs`). All identifiers in Suture are 256-bit BLAKE3 hashes (32 bytes, 64 hex characters).

- `Hash::from_data(data)` — compute hash of arbitrary bytes
- `Hash::from_hex(str)` / `to_hex()` — hex encoding/decoding
- `Hash::ZERO` — sentinel value (all zeros)
- CAS uses a 2-char prefix directory scheme (256 buckets) to avoid large directories

### Merge

Three-way merge with format-aware conflict detection:

1. **Text merge** — line-based diff for files without a semantic driver
2. **Semantic merge** — format-aware merge via drivers that understand file structure
3. Returns `MergeStatus::Clean` or `MergeStatus::Conflict`

## Merge Architecture

### The `SutureDriver` Trait

Defined in `suture-driver/src/lib.rs`:

```rust
pub trait SutureDriver: Send + Sync {
    fn name(&self) -> &str;
    fn supported_extensions(&self) -> &[&str];
    fn diff(&self, base: Option<&str>, new: &str) -> Result<Vec<SemanticChange>, DriverError>;
    fn format_diff(&self, base: Option<&str>, new: &str) -> Result<String, DriverError>;
    fn merge(&self, base: &str, ours: &str, theirs: &str) -> Result<Option<String>, DriverError>;
}
```

- `diff()` produces `SemanticChange` entries (`Added`, `Removed`, `Modified`, `Moved`) with path-level granularity
- `merge()` returns `Some(merged)` for clean merges, `None` for conflicts
- `format_diff()` produces human-readable diff output for `suture diff`

### Merge Flow

1. User runs `suture merge feature-branch`
2. CLI computes LCA (merge base) from the PatchDag
3. For each conflicted file, checks if a semantic driver exists for the file extension
4. If a driver exists: calls `driver.merge(base, ours, theirs)`
5. If no driver: falls back to line-based text merge with conflict markers
6. Reports results: "Resolved `config.json` via json driver" or "Merge has N conflict(s)"

### Driver Registry

`DriverRegistry` (`suture-driver/src/registry.rs`) dispatches to the correct driver by file extension. The CLI builds a `builtin_registry()` in `suture-cli/src/driver_registry.rs` that registers all 17 drivers.

### `SemanticChange` Types

```rust
pub enum SemanticChange {
    Added { path: String, value: String },
    Removed { path: String, old_value: String },
    Modified { path: String, old_value: String, new_value: String },
    Moved { old_path: String, new_path: String, value: String },
}
```

## Storage Layout

### Repository Directory (`.suture/`)

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

### CAS (Content Addressable Storage)

Defined in `suture-core/src/cas/`. Blobs are stored using a content-addressed scheme:

- Hash split into 2-char prefix directory + 62-char filename (256 buckets)
- Optional Zstd compression (level 3 default)
- Pack files for efficient storage of small objects
- Deduplication by hash — identical content stored once

### SQLite Metadata

Defined in `suture-core/src/metadata/`. Stores:

- All patches (serialized JSON)
- Branch-to-tip mappings
- Per-repo configuration (`author`, etc.)
- Config key-value pairs

### Audit Log

Defined in `suture-core/src/audit.rs`. Append-only JSONL file with hash chaining:

- Each entry contains BLAKE3 hash of previous entry (tamper-evident chain)
- Fields: sequence number, timestamp, actor, action, details
- Optional Ed25519 signature for non-repudiation
- `verify_chain()` checks entire log integrity

## CLI Architecture

The CLI (`suture-cli`) uses [clap](https://docs.rs/clap) with derive macros for command parsing.

### Command Dispatch

- `src/main.rs` — entry point, clap `#[derive(Parser)]`
- `src/cmd/mod.rs` — re-exports all 58 command modules
- `src/cmd/<name>.rs` — each subcommand in its own file

Key command modules:

| Command | File | Description |
|---------|------|-------------|
| `init` | `cmd/init.rs` | Initialize a new repository |
| `add` | `cmd/add.rs` | Stage files |
| `commit` | `cmd/commit.rs` | Create a patch |
| `merge` | `cmd/merge.rs` | Merge branches with semantic drivers |
| `diff` | `cmd/diff.rs` | Show semantic or line diffs |
| `log` | `cmd/log.rs` | View patch history |
| `push` / `pull` | `cmd/push.rs`, `cmd/pull.rs` | Remote sync via protocol |
| `stash` | `cmd/stash.rs` | Stash and restore changes |
| `rebase` | `cmd/rebase.rs` | Rebase patches onto new base |
| `bisect` | `cmd/bisect.rs` | Binary search for bug-introducing commit |
| `tui` | `cmd/tui.rs` | Launch terminal UI |

### Supporting Modules

| Module | Description |
|--------|-------------|
| `driver_registry.rs` | Builds `DriverRegistry` with all 17 builtin drivers |
| `display.rs` | Terminal output formatting and colors |
| `fuzzy.rs` | Fuzzy matching for branch/tag/patch selection |
| `remote_proto.rs` | HTTP-based remote protocol implementation |
| `ref_utils.rs` | Reference resolution utilities |
| `style.rs` | CLI styling helpers |

## Remote and Sync

### Wire Protocol (`suture-protocol`)

Binary protocol with Zstd compression. Handles:

- V2 handshake with capability negotiation
- Delta encoding for efficient transfer
- Batch operations for push/pull

### CLI Remote Commands

The CLI implements push/pull/fetch via HTTP in `src/remote_proto.rs`:

- `suture remote add <name> <url>` — register a remote
- `suture push <remote> <branch>` — push patches and blobs
- `suture pull <remote>` — fetch and merge remote patches
- `suture fetch <remote>` — download without merging
- `suture clone <url>` — clone a remote repository

## Hub Architecture (`suture-hub`)

The hub is a central server for hosting Suture repositories.

### Transport Layers

| Layer | Technology | Description |
|-------|-----------|-------------|
| HTTP | Axum | REST API, middleware, auth, webhooks |
| gRPC | Tonic + Prost | Bidirectional streaming, defined in `proto/suture.proto` |
| Storage | SQLite | Repository metadata and blob storage |

### gRPC Service (`SutureHub`)

Defined in `crates/suture-hub/proto/suture.proto`:

- `Handshake` — version and capability negotiation
- `ListRepos` / `CreateRepo` / `DeleteRepo` — repository management
- `ListBranches` / `CreateBranch` / `DeleteBranch` — branch management
- `ListPatches` / `GetBlob` — content retrieval
- `Push` / `Pull` — patch and blob synchronization
- `GetTree` — file tree at a given branch
- `Search` — cross-repository search

### Features

- **Auth** — Ed25519 key-based authentication
- **Webhooks** — event-driven notifications (`suture-hub/src/webhooks.rs`)
- **S3 Backend** (feature `s3-backend`) — blob storage via `suture-s3` (AWS SigV4)
- **Raft Clustering** (feature `raft-cluster`) — consensus via `suture-raft` for HA deployments
- **Mirrors** — repository mirroring support
- **Pagination** — for large result sets

## Daemon (`suture-daemon`)

Background daemon for continuous operations:

- **File watcher** — uses `notify` crate to detect working tree changes
- **Shared memory** — `memmap2`-based SHM for inter-process state
- **Mount manager** — manages FUSE mount points
- **Auto-sync** — automatic push/pull to configured remotes
- **Log rotation** — via `humantime`-based timestamps

## Plugin System

### Builtin Plugins

`BuiltinDriverPlugin<D>` wraps any `SutureDriver` implementation for registration in the `PluginRegistry`.

### WASM Plugins (Experimental)

The `wasm-plugins` feature in `suture-driver` enables loading WebAssembly drivers at runtime via [Wasmtime](https://wasmtime.dev/):

- `PluginRegistry::load_wasm_plugin(path)` — load `.wasm` files
- ABI version checking (version 1)
- Plugin descriptor files (`.suture-plugin`) for discovery
- `PluginRegistry::discover_plugins(dir)` — scan a directory for plugin descriptors

The WASM ABI is documented in `crates/suture-driver/src/wasm_abi.md`.

## FUSE and WebDAV (`suture-vfs`)

Exposes a Suture repository as a filesystem:

- **FUSE3 mount** — read/write access via `fuse3` + Tokio runtime
- **WebDAV server** — HTTP-based file access via Axum
- **Path translation** — maps between filesystem paths and repository paths

FUSE integration tests require root privileges and are `#[ignore]`d by default.

## Language Server (`suture-lsp`)

Implements the Language Server Protocol via `tower-lsp`:

- Hover information for patches
- Diagnostics for merge conflicts
- Workspace-aware repository operations

## Platform Bindings

| Binding | Crate | Technology |
|---------|-------|-----------|
| Node.js | `suture-node` | napi-rs (CDLL) |
| Python | `suture-py` | PyO3 (excluded from workspace) |

## Error Handling Strategy

All crates use `thiserror` for error types. Key patterns:

- `RepoError` — repository operations (init, commit, merge, etc.)
- `CasError` — content-addressable storage operations
- `DagError` — DAG operations (cycle detection, missing patches)
- `DriverError` — driver parsing and merge errors
- `MetaError` — SQLite metadata operations
- `MergeError` — merge results and failures

## Build and Dependency Graph

```
suture-common (no deps)
    ↑
suture-core → suture-common
    ↑
suture-driver → suture-core, suture-common
    ↑
suture-merge → suture-driver
suture-cli → suture-core, suture-common, suture-driver, suture-merge, all drivers
suture-hub → suture-common, suture-core, suture-protocol
```

The full dependency graph spans 37 crates with `suture-common` as the sole leaf dependency.
