# Suture v2.9.0 — First Public Release

Suture is a patch-based version control system with semantic merge. It understands file formats (JSON, YAML, SQL, etc.) and only flags genuine conflicts — not structural ones.

## Installation

```bash
# From source
cargo install suture-cli

# From crates.io (after publishing)
cargo install suture-cli

# From binary
# Download from GitHub Releases page
```

## Quick Start

```bash
suture init my-project
cd my-project
echo '{"name": "suture"}' > config.json
suture add config.json
suture commit -m "initial commit"
suture log
```

## Semantic Merge

The killer feature. Two developers editing different JSON keys? Merged cleanly. Different YAML anchors? No conflict. Suture parses your files structurally.

## What's Included

### Core (suture-core)
- Patch-based VCS with content-addressable storage
- 14 semantic merge drivers (JSON, YAML, TOML, CSV, XML, Markdown, DOCX, XLSX, PPTX, SQL, PDF, Image, OTIO)
- DAG-based history with branches, tags, merge, rebase, cherry-pick
- Stash, blame, reflog, notes, worktrees
- Ed25519 signing on push
- GC and fsck for repository maintenance

### Hub (suture-hub)
- Self-hosted collaboration server (SQLite backend)
- HTTP API + gRPC (14 RPCs)
- Token-based authentication
- Branch protection and mirrors
- Cursor-based pagination
- Webhooks (push/branch events with HMAC signing)
- S3 blob backend (optional)
- Raft consensus (optional)

### Daemon (suture-daemon)
- Background file watcher with auto-commit
- Auto-sync with remote hub
- Shared memory for nanosecond status queries
- FUSE/WebDAV mount lifecycle management

### VFS (suture-vfs)
- FUSE3 read-write mount (Linux)
- WebDAV server (cross-platform)
- Inode allocation and path translation

### CLI (suture-cli)
- 37 commands covering all VCS operations
- TUI with 7 tabs

### Editor Integration
- Neovim plugin (Lua): 10 commands, gutter signs, float windows
- JetBrains plugin (Kotlin): 10 actions, VCS root detection
- VS Code extension (TypeScript): 14 commands, quick pick, output channel

### Language Bindings
- Node.js (napi-rs native addon)
- Python (PyO3)

### Desktop App
- Tauri v2 with web UI (6 views, dark theme)

## Quality
- 898 tests, 0 failures
- 0 clippy warnings
- 21 proptest property-based test suites (10K+ randomized cases)
- 28 Criterion benchmarks
- 16 semantic drivers
