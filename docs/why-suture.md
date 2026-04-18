# Why Suture?

## The Problem

Version control was built for source code — files where every line matters and changes are text-level. But most of the world's files aren't source code. They're:

- **Config files** (JSON, YAML, TOML) where two people editing different keys shouldn't conflict
- **Documents** (DOCX, XLSX, PPTX) where paragraph-level changes should merge
- **Data** (CSV, SQL) where row-level changes should combine
- **Media** (video timelines, images) where structural changes should be trackable

Git treats all of these as opaque bytes. A single character change in a Word doc produces a completely different binary file — Git sees 100% conflict and gives up.

```
  The reality of "version controlling" a Word document in Git:

  $ git diff report.docx
  Binary files a/report.docx and b/report.docx differ

  That's it. That's the entire diff.
```

## How Suture Works

Suture doesn't look at bytes — it looks at structure.

```
  Traditional (Git)                      Semantic (Suture)
  ──────────────────────                 ──────────────────────
  Compare line by line                   Parse file format
  │                                      │
  ├─ Lines overlap? → CONFLICT          ├─ Same logical address? → CONFLICT
  │                                      │
  └─ Lines don't overlap? → OK          └─ Different addresses? → MERGE

  Example: two people edit config.json
  ─────────────────────────────────────

  Git sees:                              Suture sees:
  line 2 changed (host)                  key "host" changed
  line 3 changed (port)                  key "port" changed
  Lines 2-3 overlap → CONFLICT          Different keys → CLEAN MERGE
```

Changes in Suture are modeled as **patches** that record the logical addresses they modify. Two patches conflict only when those address sets overlap. Everything else merges automatically.

```
  Patch DAG:

       C1 ─── C2 ─── C5 (merge)
       │      │    ╱
       │      │   ╱
       │      C3 ─ C4
       │      │
       └──────┘

  C2: edit key "host"  →  address: /config/host
  C3: edit key "port"  →  address: /config/port
  C4: edit key "debug" →  address: /config/debug

  C5 merges C2 and C3: addresses don't overlap → clean merge
  C5 merges C2 and C4: addresses don't overlap → clean merge
```

## A Real-World Example

A video editing team works on a commercial:

1. **Jane** (editor) restructures Act 2 — moves clips, adjusts timing
2. **Mike** (colorist) applies a new grade to Act 1
3. **Sarah** (sound designer) replaces the music track

With Git, all three are editing the same binary timeline file. Only one person's changes survive. The other two are lost or require manual replay.

With Suture:

```bash
$ suture log --oneline
a3f21c  sarah: replace music track with licensed cue
b7e42d  mike: apply FilmLook grade to Act 1
9c1a8e  jane: restructure Act 2 — remove 4s, reorder clips
4f2d0b  initial assembly

$ suture diff 9c1a8e..a3f21c timeline.otio
track A1: clip "Score_v1" → "LicensedCue_07"    ← Sarah's change
track V1: clip "Scene3" trimmed 00:00:04:00      ← Jane's change
track V1: clip "Scene5" grade → "FilmLook"        ← Mike's change
```

All three changes are preserved. All three can be reverted independently. The full history is queryable.

## The 16 Supported File Types

| Category | Formats |
|----------|---------|
| Config & data | JSON, YAML, TOML, CSV, XML |
| Documents | Markdown, DOCX, XLSX, PPTX, PDF |
| Databases | SQL (DDL schema diff) |
| Images | PNG, JPG, GIF, BMP, WebP, TIFF, ICO, AVIF (metadata) |
| Video | OTIO (OpenTimelineIO timelines) |

Files without a driver fall back to line-based merge — same as Git.

## Open Source, Rust-Native, No Server Required

- **Open source** (Apache 2.0) — audit the code, contribute, fork
- **Written in Rust** — fast, memory-safe, no runtime dependencies
- **No server required** — works entirely offline as a local VCS
- **Optional server** — Suture Hub for collaboration, but never required
- **Cross-platform** — Linux, macOS, Windows
- **37 CLI commands** — full Git-compatible workflow

## See Also

- [Suture vs. Git](comparing-with-git.md) — honest comparison
- [Suture for Video Editors](video-editors.md)
- [Suture for Document Authors](document-authors.md)
- [Suture for Data Science](data-science.md)
- [Quick Start](quickstart.md)
