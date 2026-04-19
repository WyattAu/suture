---
title: "Semantic Merge: How Suture Makes Structured File Merges Painless"
date: 2026-04-19
author: Suture Team
tags: [semantic-merge, version-control, json, docx, otio]
excerpt: "Git treats DOCX and XLSX as binary blobs. Suture's semantic merge drivers understand file structure and produce clean merges."
---

# Semantic Merge: How Suture Makes DOCX, JSON, and Video Timeline Merges Painless

## The Problem

Git was designed for source code. Every merge is a line-by-line comparison, and every conflict is a disagreement about which lines belong where. For `.rs` and `.py` files, this works well enough. For everything else, it breaks down.

Consider what happens when two people edit a Word document in the same Git repo. Git stores `.docx` files as ZIP archives containing XML. A single character change re-compresses the archive, producing a completely different binary blob:

```
$ git diff report.docx
Binary files a/report.docx and b/report.docx differ
```

That's the entire diff. No context, no structure, no way to reconcile changes. If both branches modified the document, Git gives you a binary conflict — pick one version and discard the other.

JSON and YAML configs aren't much better. Git's line-based merge sees two people editing different keys in the same file and flags a conflict because the changed lines are adjacent:

```json
{
<<<<<<< HEAD
  "database": {"host": "db.example.com", "port": 5432},
=======
  "database": {"host": "localhost", "port": 3306},
>>>>>>> feature/cache
  "cache": {"ttl": 300}
}
```

This isn't a real conflict. One person changed the database host, the other changed the port. But Git doesn't know what a JSON key is — it just sees overlapping line ranges.

Video timelines are even worse. OpenTimelineIO (`.otio`) files describe edits as nested JSON structures with clips, tracks, markers, and transitions. Two editors working on different acts of the same timeline will produce a merge that looks like a bomb went off inside a text editor.

The root cause is the same in every case: Git operates on bytes and lines. It has no model of file structure.

## The Solution: Format-Aware Merge

Suture's semantic drivers understand file structure. Instead of comparing lines, each driver parses the file format and computes diffs at the logical level — keys, elements, rows, slides, cells, clips.

**JSON/YAML/TOML:** Merges happen at the field level. Both sides can change different keys in the same object without conflict. Nested objects and arrays are handled recursively, using RFC 6901 JSON Pointer paths as logical addresses.

**DOCX:** Merges happen at the paragraph and table level. The driver unzips the `.docx` archive, parses the underlying XML, and tracks changes by paragraph index. Two people editing different sections of the same document merge cleanly.

**OTIO:** Merges happen at the clip and track level. The driver understands timeline structure — tracks, clips, markers, effects — so an editor restructuring Act 2 and a colorist grading Act 1 can work simultaneously.

Here's a concrete example. Two developers independently edit `config.json`:

**Base:**
```json
{
  "database": {"host": "localhost", "port": 5432},
  "cache": {"ttl": 60, "max_size": 1000}
}
```

**Alice** changes `database.host` to `"db.example.com"`. **Bob** changes `cache.ttl` to `300`.

With Git, this is a conflict. With Suture:

```json
{
  "database": {"host": "db.example.com", "port": 5432},
  "cache": {"ttl": 300, "max_size": 1000}
}
```

Both changes applied. Valid JSON. No conflict markers.

## How It Works

Suture uses a three-way merge: base, ours, and theirs. The process is:

1. **Parse structure.** The appropriate semantic driver parses all three file versions into an intermediate representation.
2. **Compute semantic diffs.** The driver compares base vs. ours and base vs. theirs, producing a set of changes mapped to logical addresses (e.g., `database/host`, `cache/ttl`).
3. **Check for overlap.** Two patches conflict only when their touch sets intersect. If Alice changed `database/host` and Bob changed `cache/ttl`, the touch sets `{database/host}` and `{cache/ttl}` are disjoint — the patches commute, and both apply cleanly.
4. **Produce merged output.** The driver reconstructs the file from the merged state, serializing back to the original format.

This is formalized as a patch algebra. Per the commutativity theorem in Suture's spec: if T(P1) and T(P2) are the touch sets of two patches, and T(P1) intersect T(P2) is empty, then P1 composed with P2 equals P2 composed with P1. The merge is deterministic and order-independent.

File type detection is automatic — Suture selects the right driver based on file extension. No configuration needed. The 13 supported file types cover JSON, YAML, TOML, CSV, XML, Markdown, DOCX, XLSX, PPTX, PDF, OTIO, SQL, and common image formats (PNG, JPG, GIF, BMP, WebP, TIFF, ICO, AVIF). Files without a matching driver fall back to line-based merge, identical to Git's behavior.

Suture also provides an `--integrity` diff mode that computes Shannon entropy and structural fingerprints of file changes. The motivation is transparency — in a post-XZ-backdoor world, being able to mathematically verify that a binary artifact changed in expected ways isn't optional, it's essential.

## Getting Started

**Install Suture:**

```sh
cargo install --git https://github.com/WyattAu/suture.git --path crates/suture-cli
```

**As a standalone merge tool** (no repo required):

```sh
suture merge-file base.json ours.json theirs.json
```

Auto-detects the driver by file extension. Explicit selection with `--driver`:

```sh
suture merge-file --driver yaml -o merged.yaml base.yaml ours.yaml theirs.yaml
```

**As a Git merge driver** (zero config after one command):

```sh
suture git driver install
```

This writes a merge driver script, configures Git's `merge.suture` section, and updates `.gitattributes` with 20 file patterns. Commit the generated files and every subsequent `git merge` on supported formats will use Suture automatically.

**As a GitHub Action** for CI:

```yaml
name: Semantic Merge
on:
  pull_request:
    types: [opened, synchronize, reopened]
jobs:
  semantic-merge:
    runs-on: ubuntu-latest
    permissions:
      contents: write
      pull-requests: write
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0
      - uses: WyattAu/suture-action@v1
        with:
          github_token: ${{ secrets.GITHUB_TOKEN }}
```

Every PR with conflicts on structured files gets automatically resolved.

## A Real-World Walkthrough

A team shares `config.json` in a Git repo. **Alice** updates the database URL. **Bob** changes the cache TTL. Both branch off `main` and commit independently.

**Alice's branch** (`feature/db-url`):
```json
{
  "database": {"host": "db.prod.example.com", "port": 5432},
  "cache": {"ttl": 60, "max_size": 1000},
  "logging": {"level": "info"}
}
```

**Bob's branch** (`feature/cache-ttl`):
```json
{
  "database": {"host": "localhost", "port": 5432},
  "cache": {"ttl": 300, "max_size": 1000},
  "logging": {"level": "info"}
}
```

**With Git**, merging either branch produces:

```
CONFLICT (content): Merge conflict in config.json
Automatic merge failed; fix conflicts and then commit the result.
```

The file is left with conflict markers and broken JSON syntax. Someone has to manually edit the file, which is error-prone and doesn't scale.

**With Suture** (standalone):

```sh
$ suture merge-file base.json ours.json theirs.json
Merged via JSON driver (semantic merge)
Merge clean
```

Output:

```json
{
  "database": {"host": "db.prod.example.com", "port": 5432},
  "cache": {"ttl": 300, "max_size": 1000},
  "logging": {"level": "info"}
}
```

Both changes preserved. Valid JSON. Zero manual intervention.

**With Suture as a Git driver**, the same result happens automatically during `git merge` — no extra commands needed. The driver detects `config.json`, parses both sides, and writes the merged result before Git even reports the merge status.

This is the core insight: most "conflicts" on structured files aren't conflicts at all. They're artifacts of a merge algorithm that doesn't understand the data it's merging. Suture fixes that.
