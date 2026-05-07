# Announcing Suture v1.0.0-rc.1: Semantic Merge That Actually Works

I've been shipping code with Git for over a decade, and one thing has never stopped bothering me: Git treats every file as opaque lines of text. When two people edit different keys in a JSON config file, Git produces a merge conflict. When someone reorders a YAML file, Git sees every line as changed.

So I built Suture — a version control system written in Rust that understands your file formats. Today I'm releasing v1.0.0-rc.1, the "honest v1.0": every feature works, nothing is fake, and the test suite proves it.

## The Killer Feature: Semantic Merge

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
```

Suture parsed the JSON, identified that Alice touched `/host` and Bob touched `/port`, and merged them at the field level. No conflict markers. No manual resolution.

The same works for YAML, TOML, CSV, XML, Markdown, DOCX, XLSX, and PPTX files. Everything else falls back to line-based diff and merge, just like Git.

**Try it yourself in 60 seconds:** [docs/demo.md](docs/demo.md)

## Works as a Git Merge Driver

You don't have to switch VCS to use Suture's semantic merge. Configure it as a Git merge driver and it handles structured files automatically while Git handles everything else:

```bash
suture driver install --git
git config merge.suture.driver "suture driver merge %O %A %B %P %S"
git config merge.suture.name "Suture semantic merge"
```

## What Suture Is

A full version control system with branching, merging, rebasing (interactive too), cherry-picking, bisect, stash, worktrees, tags, blame, Ed25519 commit signing, a hub server for push/pull, and a terminal UI.

Under the hood: BLAKE3-hashed blobs (Zstd compressed), content-addressable store with SQLite metadata, and a patch DAG where two patches conflict only when their logical address sets overlap.

## The Numbers

- **602 tests, 0 failures** — no skipped, no ignored
- **Clippy-clean** with all pedantic lints
- **9 built-in format drivers** for JSON, YAML, TOML, CSV, XML, Markdown, DOCX, XLSX, PPTX
- **20 crates** published to crates.io, Apache 2.0 licensed
- Core patch algebra verified via extensive property-based testing (proptest)

## Honesty Check

Some things that aren't there yet:

- **No Windows worktree support** (uses Unix symlinks)
- **No shallow clone on pull** (push depth works, pull doesn't)
- **Hub is single-server** — no replication, no clustering
- **No GUI** — CLI and terminal UI only

## Install

```bash
cargo install suture-cli
```

Binary releases for Linux (x86_64/aarch64) and macOS (x86_64/aarch64):

```bash
curl -fsSL https://raw.githubusercontent.com/WyattAu/suture/main/install.sh | sh
```

If you work with config files, CSVs, or Office documents and have felt the pain of line-based merging, give Suture a try. Bugs, feature requests, and driver contributions are all welcome.

**GitHub:** [github.com/WyattAu/suture](https://github.com/WyattAu/suture)
