# Suture: Universal Semantic Version Control

[![License: Apache 2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
[![Rust: Stable](https://img.shields.io/badge/Rust-Stable-orange.svg)](https://www.rust-lang.org)
[![Build: Nix](https://img.shields.io/badge/Build-Nix_Flake-blueviolet.svg)](https://nixos.org)

**Suture** is a patch-based version control system built for any file type — not just code. Instead of snapshotting file states, Suture models changes as *patches* in a directed acyclic graph (DAG), enabling **semantic diffs and merges** for structured data formats through pluggable drivers.

Built in Rust with BLAKE3 content addressing and SQLite metadata, Suture provides 37 CLI commands covering the full workflow: init, add, commit, branch, merge, rebase (interactive and non-interactive), push/pull, bisect, hooks, stash, blame, tags, and more.

## What Makes It Different

Traditional VCS tools treat non-text files as opaque blobs. Suture's **Semantic Driver SDK** defines how specific file formats should be diffed and merged:

- **JSON** — field-level structural diffs (RFC 6901 JSON Patch)
- **YAML** — key-level structural merge
- **TOML** — table/key-aware merge
- **CSV** — row-level merge
- **XML** — element-aware structural merge
- **DOCX** — Word document merge (via OpenXML)
- **XLSX** — Excel spreadsheet merge (via OpenXML)
- **PPTX** — PowerPoint merge (via OpenXML)
- **OTIO** — OpenTimelineIO editorial merge

Patches form a DAG (not a linear chain), so merge conflicts are detected via *touch set intersection* — two patches conflict only when they modify the same logical addresses. This enables deterministic, mathematically-grounded merging.

## Installation

### Binary Releases (recommended)

Pre-built binaries for Linux (x86_64/aarch64), macOS (x86_64/aarch64), and Windows (x86_64) are available on [GitHub Releases](https://github.com/WyattAu/suture/releases):

```bash
# Linux / macOS
curl -sL https://github.com/WyattAu/suture/releases/latest/download/suture-$(uname -m)-unknown-linux-gnu -o /usr/local/bin/suture
chmod +x /usr/local/bin/suture
```

### From Source

**Nix (reproducible build):**
```bash
git clone https://github.com/WyattAu/suture.git
cd suture
nix develop
cargo build --release
```

**Cargo:**
```bash
cargo install --git https://github.com/WyattAu/suture.git
```

## Quick Start

```bash
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

# Semantic merge in action
# Edit config.json on both branches, then merge —
# Suture merges JSON fields structurally instead of line-by-line
```

## Features

### Hook System

Git-compatible hooks that run at key workflow points:

```bash
# Create a pre-commit hook (lint check)
cat > .suture/hooks/pre-commit << 'EOF'
#!/bin/sh
echo "Running linter..." >&2
exit 0
EOF
chmod +x .suture/hooks/pre-commit
```

Supported hooks: `pre-commit`, `post-commit`, `pre-push`, `post-push`, `pre-merge`, `post-merge`, `pre-rebase`, `post-rebase`, `pre-cherry-pick`, `pre-revert`. Supports `hook.d/` directories for multiple ordered scripts. Environment variables (`SUTURE_BRANCH`, `SUTURE_HEAD`, `SUTURE_AUTHOR`, etc.) are passed to every hook.

### Interactive Rebase

Full git-compatible interactive rebase with pick/reword/edit/squash/drop:

```bash
suture rebase -i main
# Opens $EDITOR with TODO file:
#
# pick abc12345 first commit
# pick def67890 second commit
# squash ghi90123 third commit
# drop jkl45678 bad commit
#
# Edit, save, quit — rebase executes the plan
```

### Bisect

Binary search for bug-introducing commits:

```bash
# Manual bisect
suture bisect start <good-commit> <bad-commit>
suture bisect good          # or: suture bisect bad
suture bisect reset

# Automated bisect with a test command
suture bisect run <good> <bad> -- ./run_tests.sh
# Reports the first bad commit automatically
```

### Semantic Merge

When merging files with matching drivers, Suture performs structural merges instead of line-level diffs. This eliminates false conflicts in JSON configs, YAML manifests, CSV data, and Office documents.

### Per-Repo Configuration

```bash
suture config user.name "Alice"
suture config core.hooksPath "./scripts/hooks"
```

Config lookup priority: `.suture/config` → SQLite config → `~/.config/suture/config.toml` → defaults.

## CLI Reference

### Repository

| Command | Description |
|---------|-------------|
| `suture init [path]` | Initialize a new repository |
| `suture status` | Show working tree status |
| `suture show <ref>` | Show commit details |
| `suture reflog` | Show HEAD movement log |
| `suture fsck` | Verify repository integrity |
| `suture gc` | Garbage collect unreachable objects |
| `suture config <key=value>` | Get/set configuration |

### Staging & Commits

| Command | Description |
|---------|-------------|
| `suture add <paths>` | Stage files for commit |
| `suture add --all` | Stage all modified/deleted files |
| `suture rm <paths>` | Remove files from working tree |
| `suture rm --cached <paths>` | Unstage files (keep on disk) |
| `suture commit <msg>` | Create a commit |
| `suture commit --all <msg>` | Stage all and commit |
| `suture stash` | Stash uncommitted changes |
| `suture stash pop` | Restore latest stash |
| `suture stash apply <n>` | Apply a specific stash |
| `suture stash list` | List stashes |
| `suture stash drop <n>` | Delete a specific stash |

### Branching & History

| Command | Description |
|---------|-------------|
| `suture branch` | List branches |
| `suture branch <name>` | Create a branch |
| `suture branch <name> -t <target>` | Create branch from target |
| `suture branch -d <name>` | Delete a branch |
| `suture checkout <branch>` | Switch branches |
| `suture checkout -b <branch>` | Create and switch |
| `suture log` | Show commit history |
| `suture log --graph` | Show ASCII DAG topology |
| `suture log --oneline` | Compact one-line log |
| `suture log --author <name>` | Filter by author |
| `suture log --grep <pattern>` | Filter by message |
| `suture log --since <date>` | Commits newer than date |
| `suture log --all` | All branches |
| `suture shortlog` | Commit summary by author |
| `suture blame <path>` | Per-line attribution |
| `suture diff` | Show differences |
| `suture diff --from <ref> --to <ref>` | Diff between refs |
| `suture diff --cached` | Staged vs HEAD |

### Merging & Rebasing

| Command | Description |
|---------|-------------|
| `suture merge <branch>` | Merge a branch into HEAD |
| `suture rebase <branch>` | Rebase onto a branch |
| `suture rebase -i <branch>` | Interactive rebase (pick/reword/edit/squash/drop) |
| `suture rebase --abort` | Abort in-progress rebase |
| `suture cherry-pick <hash>` | Apply a commit onto HEAD |
| `suture revert <hash>` | Revert a commit |
| `suture squash <n>` | Squash last N commits |
| `suture reset <target>` | Reset HEAD to a target |

### Remote Operations

| Command | Description |
|---------|-------------|
| `suture push [remote]` | Push to remote Hub |
| `suture pull [remote]` | Pull and merge from remote |
| `suture pull --rebase [remote]` | Pull with rebase (linear history) |
| `suture fetch [remote]` | Fetch without merging |
| `suture clone <url> [dir]` | Clone a remote repository |
| `suture clone --depth <n> <url>` | Shallow clone |
| `suture remote add <name> <url>` | Add a remote |
| `suture remote list` | List remotes |
| `suture remote remove <name>` | Remove a remote |

### Bisect

| Command | Description |
|---------|-------------|
| `suture bisect start <good> <bad>` | Start bisect session |
| `suture bisect good` | Mark current commit as good |
| `suture bisect bad` | Mark current commit as bad |
| `suture bisect run <good> <bad> -- <cmd>` | Automated binary search |
| `suture bisect reset` | Cancel bisect session |

### Tags & Notes

| Command | Description |
|---------|-------------|
| `suture tag <name>` | Create a tag |
| `suture tag -a <name> -m <msg>` | Create annotated tag |
| `suture tag --list` | List tags |
| `suture tag -d <name>` | Delete a tag |
| `suture notes add <hash> -m <msg>` | Attach a note |
| `suture notes list <hash>` | List notes for a commit |

### Signing & Utilities

| Command | Description |
|---------|-------------|
| `suture key generate [name]` | Generate Ed25519 signing key |
| `suture key list` | List signing keys |
| `suture key public [name]` | Show public key |
| `suture drivers` | List semantic drivers |
| `suture completions <shell>` | Generate shell completions |
| `suture mv <src> <dst>` | Move/rename a file |
| `suture tui` | Launch terminal UI |
| `suture version` | Show version |

## Semantic Drivers

Drivers implement the `SutureDriver` trait for format-aware diff and merge:

```rust
pub trait SutureDriver {
    fn name(&self) -> &str;
    fn extensions(&self) -> &[&str];
    fn format_diff(&self, old: Option<&str>, new: &str) -> Result<String, DriverError>;
    fn merge(&self, base: &str, ours: &str, theirs: &str) -> Result<Option<String>, DriverError>;
}
```

| Driver | Extensions | Capabilities |
|--------|-----------|-------------|
| JSON | `.json` | RFC 6901 path-based diff/merge |
| YAML | `.yaml`, `.yml` | Key-level structural merge |
| TOML | `.toml` | Table/key-aware merge |
| CSV | `.csv` | Row-level merge |
| XML | `.xml` | Element-aware structural merge |
| DOCX | `.docx` | Word document merge (via OpenXML) |
| XLSX | `.xlsx` | Spreadsheet merge (via OpenXML) |
| PPTX | `.pptx` | Presentation merge (via OpenXML) |
| OTIO | `.otio` | OpenTimelineIO editorial merge |

When `suture diff` or `suture merge` encounters a file with a matching driver, it uses the semantic diff/merge instead of raw line-level operations. Office document drivers (DOCX, XLSX, PPTX) are powered by the `suture-ooxml` crate.

## Architecture

```
suture-common/    Shared types (Hash, BranchName, RepoPath, FileStatus)
suture-core/      CAS + DAG + patches + repo + merge + stash + rebase + blame + hooks
suture-cli/       CLI binary (37 commands, clap, async)
suture-hub/       HTTP/JSON Hub server with Ed25519 auth
suture-driver/    Driver trait + DriverRegistry
suture-driver-*/  Format-specific drivers (JSON, YAML, TOML, CSV, XML, DOCX, XLSX, PPTX, OTIO)
suture-ooxml/     Office Open XML parsing library
suture-tui/       Terminal UI (ratatui)
suture-lsp/       Language Server Protocol server
suture-git-bridge/ Git interoperability layer
suture-protocol/  Network protocol definitions (protobuf)
suture-bench/     Criterion benchmarking suite
suture-fuzz/      Proptest-based fuzzing harnesses
suture-e2e/       End-to-end integration tests
```

- **CAS:** BLAKE3-hashed blobs in `.suture/objects/` with Zstd compression
- **Patch DAG:** Patches with parent references, persisted to SQLite (WAL mode)
- **Metadata:** Branches, config, working set, reflog, hooks — all in SQLite
- **Hooks:** Executable scripts in `.suture/hooks/` (git-compatible)
- **Hub:** HTTP/JSON remote coordination with Ed25519 push signing and rustls TLS

## Repository Layout

```
my-project/
  .suture/
    objects/        CAS blob storage (BLAKE3-addressed)
    metadata.db     SQLite metadata (branches, patches, config, working set)
    hooks/          Hook scripts (pre-commit, post-push, etc.)
    config          Per-repo TOML configuration
```

## License

Apache License, Version 2.0. See [LICENSE](LICENSE) for details.
