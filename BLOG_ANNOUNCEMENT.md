# Announcing Suture: A Version Control System That Understands Your File Formats

I've been shipping code with Git for over a decade, and one thing has never stopped bothering me: Git treats every file as opaque lines of text. When two people edit different keys in a JSON config file, Git produces a merge conflict. When someone reorders a YAML file, Git sees every line as changed. When a team collaborates on a spreadsheet, Git can't help at all.

So I built Suture — a version control system written in Rust that understands your file formats.

## The Problem

Here's a scenario you've probably lived through. Your team has a `config.json`:

```json
{
  "host": "localhost",
  "port": 3000,
  "database": "postgres://localhost/mydb",
  "log_level": "info"
}
```

Alice changes the host to point to staging. Bob changes the port to 8080. They're editing different fields — there's no logical conflict. But Git sees overlapping line ranges and spits out conflict markers.

With Suture, the merge is clean:

```bash
suture merge staging
# Merge completed successfully — 1 file merged, 0 conflicts
cat config.json
# {"host": "staging.example.com", "port": 8080, "database": "postgres://localhost/mydb", "log_level": "info"}
```

Suture parsed the JSON, identified that Alice touched `/host` and Bob touched `/port`, and merged them at the field level. No conflict markers. No manual resolution.

The same semantic merge works for YAML, TOML, CSV, XML, Markdown, DOCX, XLSX, and PPTX files. Everything else falls back to line-based diff and merge, just like Git.

## What Suture Is

Suture is a full version control system, not a Git add-on. It has the features you expect:

- **Branching, merging, rebasing**, cherry-picking, bisect, stash, worktrees, tags, blame
- **Interactive rebase** with pick/reword/edit/squash/drop
- **Ed25519 commit signing** and key management
- **Hub server** for push/pull collaboration with branch protection
- **Terminal UI** via `suture tui`
- **Shell completions** for bash, zsh, fish, PowerShell, and Nushell

Under the hood, Suture stores content as BLAKE3-hashed blobs (Zstd compressed) in a content-addressable store, with metadata in SQLite (WAL mode). Changes are modeled as patches in a DAG — two patches conflict only when their logical address sets overlap, not when their line ranges do.

## 9 Built-in Format Drivers

| Format | Merge granularity |
|--------|------------------|
| JSON | Field level (RFC 6901 paths) |
| YAML | Key level |
| TOML | Table and key level |
| CSV | Row and cell level |
| XML | Element and attribute level |
| Markdown | Section/block level |
| DOCX | Paragraph level |
| XLSX | Cell level |
| PPTX | Slide level |

You can also write your own drivers via the `SutureDriver` trait.

## Performance

Criterion benchmarks on the current release:

| Operation | Time |
|-----------|------|
| BLAKE3 hash (1KB) | 1.17 µs |
| CAS put+get (1KB) | 52 µs |
| DAG insertion (1K patches) | 1.8 ms |
| Patch chain apply (100 patches) | 1.4 ms |
| FileTree diff (100 files) | 40 µs |
| FileTree snapshot (10K files) | 25 µs |

## Verification

The core patch algebra is formally verified — I wrote Lean 4 proofs and TLA+ specifications for the commutativity, composition, and conflict detection properties. The proofs cover the theoretical foundation: patch composition preserves touch set unions, conflict detection is sound, and the merge algorithm terminates.

## Honesty Check

Suture is at v0.8.0. It works, it has 438 tests, and I use it daily — but it's young. Some honest limitations:

- **No Windows worktree support** (worktrees use Unix symlinks)
- **No shallow clone on pull** yet (push depth works, pull doesn't)
- **The Hub is single-server** — no Raft replication, no clustering
- **No GUI** — CLI and terminal UI only
- **No Git interoperability** — it's a separate VCS, not a Git frontend

If you need an enterprise-grade, battle-tested VCS, use Git. If you're curious about what version control looks like when it understands your data, try Suture.

## Install

```
cargo install suture-cli
```

Binary releases are available for Linux (x86_64/aarch64) and macOS (x86_64/aarch64):

```bash
curl -fsSL https://raw.githubusercontent.com/WyattAu/suture/main/install.sh | sh
```

The codebase is 20 crates published to crates.io, Apache 2.0 licensed.

## Try It

```bash
suture init
suture config user.name "Your Name"
echo '{"host": "localhost", "port": 3000}' > config.json
suture add . && suture commit "base config"
suture branch staging && suture checkout staging
# ... make changes on each branch, then merge
```

I'd love to hear what you think — especially if you work with config files, CSVs, or Office documents and have felt the pain of line-based merging. Bugs, feature requests, and driver contributions are all welcome.

**GitHub:** [github.com/WyattAu/suture](https://github.com/WyattAu/suture)
