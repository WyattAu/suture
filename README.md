# Suture

A version control system that understands your file formats.

[![Rust: Stable](https://img.shields.io/badge/Rust-Stable-orange.svg)](https://www.rust-lang.org)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

Unlike Git, which treats every file as opaque bytes, Suture uses semantic drivers to perform intelligent merges on JSON, YAML, CSV, XML, TOML, Markdown, DOCX, XLSX, and PPTX files.

## Why Suture?

- **No more false merge conflicts on JSON.** Two people edit different keys in a config file — Git produces a conflict. Suture merges them cleanly at the field level.
- **Structural YAML/TOML merge.** DevOps teams working on Kubernetes manifests or Cargo.toml files can edit different sections in parallel without conflicts.
- **Office document merge.** DOCX, XLSX, and PPTX files are parsed via OpenXML so changes to different paragraphs, cells, or slides merge automatically.
- **Patch-based history.** Changes are modeled as patches in a DAG, not whole-file snapshots. Conflicts are detected by logical address overlap, not line overlap.

## Install

**From source (primary):**

```bash
cargo install --git https://github.com/WyattAu/suture suture-cli
```

**Binary releases** (Linux x86_64/aarch64, macOS x86_64/aarch64):

```bash
# Downloads and installs to ~/.local/bin
curl -fsSL https://raw.githubusercontent.com/WyattAu/suture/main/install.sh | sh
```

Or grab the tarball from [GitHub Releases](https://github.com/WyattAu/suture/releases) directly.

## Quick Start

```bash
suture init my-project && cd my-project
echo '{"port": 8080, "host": "localhost"}' > config.json
suture add config.json && suture commit "initial config"

# Branch A: change the port
suture branch deploy && suture checkout deploy
# (edit config.json: port → 9090)
suture add config.json && suture commit "change port"

# Branch B: change the host
suture checkout main
# (edit config.json: host → "0.0.0.0")
suture add config.json && suture commit "change host"

# Merge — no conflict. Suture merges JSON fields structurally.
suture merge deploy
# config.json is now: {"port": 9090, "host": "0.0.0.0"}
```

With Git, the same scenario produces a merge conflict. Suture recognizes that the two edits touch different JSON keys and merges them automatically.

## Semantic Drivers

| Format | Extensions | What it does |
|--------|-----------|-------------|
| JSON | `.json` | Field-level diff and merge via RFC 6901 paths |
| YAML | `.yaml`, `.yml` | Key-level structural merge |
| TOML | `.toml` | Table and key-aware merge |
| CSV | `.csv` | Row-level merge |
| XML | `.xml` | Element-aware structural merge |
| Markdown | `.md` | Section-aware merge |
| DOCX | `.docx` | Word document merge via OpenXML |
| XLSX | `.xlsx` | Spreadsheet merge via OpenXML |
| PPTX | `.pptx` | Presentation merge via OpenXML |
| OTIO | `.otio` | OpenTimelineIO editorial merge |

Files without a matching driver fall back to line-based diff and merge, same as Git.

## CLI Reference

| Command | Description |
|---------|-------------|
| `suture init [path]` | Initialize a new repository |
| `suture add <paths>` | Stage files |
| `suture commit <msg>` | Create a commit |
| `suture status` | Show working tree status |
| `suture diff` | Show changes (semantic when driver matches) |
| `suture log [--graph]` | Show commit history |
| `suture branch <name>` | Create or list branches |
| `suture checkout <branch>` | Switch branches |
| `suture merge <branch>` | Merge a branch |
| `suture rebase [-i] <branch>` | Rebase (interactive supports pick/reword/edit/squash/drop) |
| `suture cherry-pick <hash>` | Apply a commit onto HEAD |
| `s revert <hash>` | Revert a commit |
| `suture stash [pop\|list\|apply]` | Stash and restore changes |
| `suture push / pull` | Remote sync |
| `suture remote add <name> <url>` | Manage remotes |
| `suture tag <name>` | Create tags |
| `suture blame <path>` | Per-line attribution |
| `suture fsck` | Verify repository integrity |
| `suture drivers` | List available semantic drivers |

Run `suture --help` for the full list.

## Architecture

Suture stores content as BLAKE3-hashed blobs (with Zstd compression) and models changes as patches in a directed acyclic graph, persisted to SQLite in WAL mode. Each patch records the logical addresses it modifies; two patches conflict only when those address sets overlap.

The codebase is split into focused crates — core engine, CLI, semantic drivers (one per format), Hub server, TUI, LSP, and more. See [docs/](docs/) for detailed design documentation.

## Comparison

| | Suture | Git | Pijul |
|---|---|---|---|
| **Merge model** | Patch DAG with semantic drivers | Line-based three-way merge | Patch DAG, line-level only |
| **Structured data** | Native support for 10+ formats | Opaque bytes | Opaque bytes |
| **Office documents** | DOCX/XLSX/PPTX merge | Binary blob conflicts | Binary blob conflicts |
| **Conflict detection** | Logical address overlap | Line overlap | Line overlap |
| **Language** | Rust | C | Rust |
| **Maturity** | Early | Production | Early |

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup, coding conventions, and pull request workflow.

## License

Apache License, Version 2.0. See [LICENSE](LICENSE) for details.
