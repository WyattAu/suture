# Suture: Universal Semantic Version Control

[![License: Apache 2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
[![Rust: Stable](https://img.shields.io/badge/Rust-Stable-orange.svg)](https://www.rust-lang.org)
[![Build: Nix](https://img.shields.io/badge/Build-Nix_Flake-blueviolet.svg)](https://nixos.org)

**Suture** is a patch-based version control system that goes beyond text files. Instead of snapshotting file states, Suture models changes as *patches* in a directed acyclic graph (DAG), enabling semantic diffs and merges for structured data formats through pluggable drivers.

Built in Rust with BLAKE3 content addressing and SQLite metadata storage, Suture provides 32 CLI commands covering init, add, commit, branch, merge, rebase, push/pull, stash, blame, notes, tags, and more.

## What Makes It Different

Traditional VCS tools treat non-text files as opaque blobs. Suture's **Semantic Driver SDK** lets you define how specific file formats should be diffed and merged. A JSON driver can produce structural diffs (field-level changes) instead of raw text diffs. A CSV driver can merge row-level changes without line conflicts. The YAML, TOML, and OpenTimelineIO drivers do the same for their formats.

Patches form a DAG (not a linear chain), so merge conflicts are detected via *touch set intersection* -- two patches conflict only when they modify the same logical addresses. This enables deterministic, mathematically-grounded merging.

## Quick Start

```bash
# Clone and build
git clone https://github.com/Suture-VCS/suture.git
cd suture
nix develop    # or: direnv allow

# Create a repository
suture init my-project
cd my-project
suture config user.name "Your Name"

# Basic workflow
echo "hello" > file.txt
suture add file.txt
suture commit "initial file"

# Branch and merge
suture branch feature
suture checkout feature
echo "world" >> file.txt
suture add file.txt
suture commit "add world"
suture checkout main
suture merge feature
```

## Installation

**Nix (recommended):**
```bash
nix develop
cargo run --bin suture -- <command>
```

**Cargo:**
```bash
cargo install --path crates/suture-cli
```

## CLI Reference

| Command | Description |
|---------|-------------|
| `suture init [path]` | Initialize a new repository |
| `suture status` | Show working tree status |
| `suture add <paths>` | Stage files for commit |
| `suture add --all` | Stage all files |
| `suture rm <paths>` | Remove files |
| `suture commit <msg>` | Create a commit |
| `suture branch` | List branches |
| `suture branch <name>` | Create a branch |
| `suture branch -d <name>` | Delete a branch |
| `suture checkout <branch>` | Switch branches |
| `suture log` | Show commit history |
| `suture log --graph` | Show branch topology graph |
| `suture log --oneline` | Compact one-line log |
| `suture shortlog` | Commit summary grouped by author |
| `suture diff` | Show file differences |
| `suture diff --from <ref> --to <ref>` | Diff between refs |
| `suture merge <branch>` | Merge a branch |
| `suture rebase <branch>` | Rebase onto a branch |
| `suture cherry-pick <hash>` | Apply a commit onto HEAD |
| `suture revert <hash>` | Revert a commit |
| `suture blame <path>` | Per-line commit attribution |
| `suture tag <name>` | Create a tag |
| `suture tag -a <name> -m <msg>` | Create an annotated tag |
| `suture tag --list` | List tags |
| `suture notes add <hash> -m <msg>` | Attach a note to a commit |
| `suture notes list <hash>` | List notes for a commit |
| `suture stash` | Stash uncommitted changes |
| `suture stash pop` | Restore latest stash |
| `suture reset <target>` | Reset HEAD to a target |
| `suture push` | Push to remote Hub |
| `suture pull` | Pull from remote Hub |
| `suture fetch` | Fetch without merging |
| `suture clone <url>` | Clone a remote repository |
| `suture config <key=value>` | Get/set config |
| `suture remote add <name> <url>` | Add a remote |
| `suture key generate` | Generate Ed25519 signing key |
| `suture show <ref>` | Show commit details |
| `suture reflog` | Show HEAD movement log |
| `suture drivers` | List semantic drivers |
| `suture version` | Show version |

## Semantic Drivers

Drivers implement the `SutureDriver` trait to provide format-aware diff and merge:

```rust
pub trait SutureDriver {
    fn name(&self) -> &str;
    fn extensions(&self) -> &[&str];
    fn format_diff(&self, old: Option<&str>, new: &str) -> Result<String, DriverError>;
    fn merge(&self, base: &str, ours: &str, theirs: &str) -> Result<Option<String>, DriverError>;
}
```

**Built-in drivers:**

| Driver | Extensions | Capabilities |
|--------|-----------|-------------|
| JSON | `.json` | RFC 6901 path-based diff/merge |
| YAML | `.yaml`, `.yml` | Key-level structural merge |
| TOML | `.toml` | Table/key-aware merge |
| CSV | `.csv` | Row-level merge |
| OTIO | `.otio` | OpenTimelineIO editorial merge |

When `suture diff` or `suture merge` encounters a file with a matching driver, it uses the semantic diff/merge instead of raw line-level operations.

## Architecture

```
suture-core/
  cas/       Content-addressable storage (BLAKE3 blobs)
  dag/       Patch DAG (in-memory, persisted to SQLite)
  engine/    Patch application, tree snapshots, diff, merge
  metadata/  SQLite metadata store (branches, config, working set)
  patch/     Patch types, touch sets, conflict detection
  repository/ High-level Repository API
  signing/   Ed25519 commit signing
```

- **CAS (Content-Addressable Storage):** Files are stored as BLAKE3-hashed blobs in `.suture/objects/`
- **Patch DAG:** Commits are patches with parent references forming a DAG, stored in SQLite
- **Metadata:** Branches, config, working set, and reflog are persisted in SQLite (WAL mode)
- **Hub:** Remote coordination via HTTP/JSON with Ed25519 push signing

## Workspace Crates

| Crate | Description |
|-------|-------------|
| `suture-common` | Shared types (Hash, BranchName, RepoPath) |
| `suture-core` | Core engine (CAS, DAG, patches, repo, merge, stash, rebase, blame, reflog, notes) |
| `suture-cli` | CLI binary (32 commands) |
| `suture-hub` | Hub daemon with SQLite persistence and Ed25519 auth |
| `suture-daemon` | Background service placeholder |
| `suture-driver` | Driver trait, DriverRegistry, semantic types |
| `suture-driver-json` | JSON semantic driver |
| `suture-driver-yaml` | YAML semantic driver |
| `suture-driver-toml` | TOML semantic driver |
| `suture-driver-csv` | CSV semantic driver |
| `suture-driver-otio` | OpenTimelineIO reference driver |
| `suture-bench` | Criterion benchmarks |

## What's Not Implemented Yet

- **Virtual File System (VFS):** NFSv4/ProjFS mounting
- **Distributed consensus:** Raft-based replication
- **Web UI:** Visual branch explorer and diff viewer
- **SSO/RBAC:** Enterprise authentication and authorization
- **Flatbuffers:** Zero-copy metadata serialization (currently using serde_json)
- **Lock-free IPC:** Shared memory status lookups

## License

Apache License, Version 2.0. See [LICENSE](LICENSE) for details.
