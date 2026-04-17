# Suture

A version control system that understands your file formats.

[![CI](https://github.com/WyattAu/suture/actions/workflows/ci.yml/badge.svg)](https://github.com/WyattAu/suture/actions)
[![Rust: Stable](https://img.shields.io/badge/Rust-Stable-orange.svg)](https://www.rust-lang.org)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

Unlike Git, which treats every file as opaque bytes, Suture uses semantic drivers to perform intelligent merges on JSON, YAML, CSV, XML, TOML, Markdown, DOCX, XLSX, PPTX, SQL, PDF, and image files.

```text
  Git merge on JSON:                    Suture merge on JSON:
  ┌─────────────────────┐              ┌─────────────────────┐
  │ <<<<<<< HEAD        │              │ {                   │
  │   "host": "prod",   │              │   "host": "prod",   │  ← from theirs
  │   "port": 8080      │              │   "port": 3000,     │  ← from ours  
  │ =======             │              │   "debug": true     │  ← from theirs
  │   "host": "staging" │              │ }                   │
  │   "port": 8080      │              └─────────────────────┘
  │ >>>>>>> staging     │              No conflict. Both changes applied.
  │   "debug": true     │
  └─────────────────────┘
  Conflict markers in your file.
```

## Who is this for?

- **DevOps teams** collaborating on Kubernetes YAML, Docker Compose, CI/CD configs, SQL schemas
- **Data teams** editing JSON/YAML/CSV pipelines and database migrations
- **Documentation teams** working with DOCX, XLSX, PPTX where Git's line merge is catastrophic
- **Config-as-code teams** managing TOML, XML, properties files across environments
- **Media teams** using NLEs (DaVinci Resolve, Premiere) via FUSE or WebDAV mount

## Install

**From source (requires Rust 1.85+):**
```bash
cargo install --path crates/suture-cli
```

This produces the `suture` binary. See [rustup.rs](https://rustup.rs) to install Rust.

**Build from repo:**
```bash
git clone https://github.com/WyattAu/suture.git
cd suture
cargo build --release --bin suture
# Binary at target/release/suture
```

## Quick Start

```bash
# Init a repo
suture init
suture config user.name "Your Name"

# Create a JSON config and commit it
echo '{"host": "localhost", "port": 3000}' > config.json
suture add .
suture commit "base config"

# Branch, edit a different key, commit
suture branch staging
suture checkout staging
echo '{"host": "staging.example.com", "port": 3000}' > config.json
suture add .
suture commit "point to staging"

# Switch back, change a different key
suture checkout main
echo '{"host": "localhost", "port": 8080}' > config.json
suture add .
suture commit "change port"

# Merge — both changes combined, zero conflicts
suture merge staging
cat config.json
# {"host": "staging.example.com", "port": 8080}
```

Suture understood the JSON structure and merged both sides — `host` from `staging`, `port` from `main` — without conflict markers. The same semantic merge works for YAML, TOML, CSV, XML, Markdown, DOCX, XLSX, PPTX, SQL, and PDF files.

## Features

### Semantic Merge (16 format drivers)

| Format | Extensions | What it does |
|--------|-----------|-------------|
| JSON | `.json` | Field-level diff and merge via RFC 6901 paths |
| YAML | `.yaml`, `.yml` | Key-level structural merge |
| TOML | `.toml` | Table and key-aware merge |
| CSV | `.csv` | Row-level merge with header detection |
| XML | `.xml` | Element/attribute-aware structural merge |
| Markdown | `.md` | Section-aware merge |
| DOCX | `.docx` | Paragraph-level Word document merge |
| XLSX | `.xlsx` | Cell-level spreadsheet merge |
| PPTX | `.pptx` | Slide-level presentation merge |
| SQL | `.sql` | DDL schema diff (CREATE/ALTER/DROP TABLE) |
| PDF | `.pdf` | Page-level text diff and merge |
| Image | `.png` `.jpg` `.gif` `.bmp` `.webp` `.tiff` `.ico` `.avif` | Metadata diff (dimensions, color type) |
| OTIO | `.otio` | OpenTimelineIO editorial merge |

Files without a matching driver fall back to line-based diff and merge, same as Git.

### VFS Mount (FUSE + WebDAV)

Mount a Suture repo as a regular directory. File saves create patches automatically.

```bash
# Linux: FUSE mount (read-write)
suture vfs mount /path/to/repo /mnt/repo

# Cross-platform: WebDAV mount
# macOS: Finder → Connect to Server → http://localhost:8080
# Windows: Map Network Drive → http://localhost:8080
```

### Hub (Self-hosted collaboration server)

```bash
suture-hub --db suture.db
# Web UI at http://localhost:50051
# API at http://localhost:50051/api/v2
```

Features: repository browsing, file tree viewer, user auth, push/pull, branch protection, mirrors, replication, search.

### Daemon (Background service)

```bash
suture daemon start /path/to/repo
# Watches for file changes, auto-commits, auto-syncs to remote
# SHM at /tmp/suture-shm-<pid> for nanosecond status queries
```

### TUI (Terminal UI)

```bash
suture tui
# 7 tabs: Status, Log, Staging, Diff, Branches, Remote, Help
# Log graph, checkout confirmation, merge conflict view
```

### LSP (Language Server)

Provides hover information (blame data) and diagnostics to editors that support the Language Server Protocol.

### CLI (37 commands)

Full Git-compatible workflow: init, add, commit, status, diff, log, branch, checkout, merge, rebase (interactive), cherry-pick, revert, stash, push, pull, remote, tag, blame, fsck, bisect, notes, worktree, gc, key, sign, and more.

Run `suture --help` for the full list.

### Editor Integration

- **Neovim**: [suture.nvim](editors/neovim/suture.nvim/) — gutter signs, 10 commands, float windows
- **Node.js**: `@suture/core` — native bindings via napi-rs with TypeScript declarations

### Git Merge Driver

Use Suture as a [Git merge driver](docs/git_merge_driver.md) for semantic merging inside existing Git workflows:

```bash
git config merge.suture.name "suture"
git config merge.suture.driver "suture merge-file --driver %s %O %A %B -o %A"
```

## Architecture

Suture stores content as BLAKE3-hashed blobs (with Zstd compression) and models changes as patches in a directed acyclic graph, persisted to SQLite in WAL mode. Each patch records the logical addresses it modifies; two patches conflict only when those address sets overlap.

The codebase is 26 focused crates — core engine, CLI, semantic drivers (one per format), Hub server, daemon, VFS, TUI, LSP, and more. See [docs/](docs/) for detailed design documentation.

## Comparison

| | Suture | Git | Pijul |
|---|---|---|---|
| **Merge model** | Patch DAG with semantic drivers | Line-based three-way merge | Patch DAG, line-level only |
| **Structured data** | Native support for 13+ formats | Opaque bytes | Opaque bytes |
| **Office documents** | DOCX/XLSX/PPTX/PDF merge | Binary blob conflicts | Binary blob conflicts |
| **VFS mount** | FUSE + WebDAV | None | None |
| **Conflict detection** | Logical address overlap | Line overlap | Line overlap |
| **Language** | Rust | C | Rust |
| **Maturity** | Early | Production | Early |

See [docs/comparison.md](docs/comparison.md) for a detailed feature comparison.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup, coding conventions, and pull request workflow.

## License

Apache License, Version 2.0. See [LICENSE](LICENSE) for details.
