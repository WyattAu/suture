---
title: "I Built a Semantic Merge Engine in Rust That Understands Word, Excel, and PowerPoint Files"
date: 2026-04-22
author: Wyatt Au
tags: [rust, semantic-merge, docx, xlsx, pptx, version-control, crates]
excerpt: "Git treats Office documents as opaque binary blobs. Suture unzips them, parses the XML inside, and merges them structurally. Here's how and why."
---

# I Built a Semantic Merge Engine in Rust That Understands Word, Excel, and PowerPoint Files

## The problem nobody talks about

If you have ever put a `.docx`, `.xlsx`, or `.pptx` file in a Git repo, you already know what happens. Two people edit the same document on different branches, someone merges, and Git hands you this:

```
CONFLICT (content): Merge conflict in report.docx
Automatic merge failed; fix conflicts and then commit the result.
```

There is no diff to inspect. No context about what changed. No conflict markers to resolve. Git treats the file as an opaque binary blob because, well, it is one -- a ZIP archive full of compressed XML. A single character change in a paragraph re-compresses differently, producing a completely different byte sequence. Git's entire merge model, built on lines and hunks, collapses.

Your options at this point are not great: pick one side and lose the other's work, manually copy-paste changes between Word windows, or stop tracking Office documents in Git altogether.

This bothered me enough that I spent the last several months building a fix.

## What Suture does

Suture is a semantic merge engine written in Rust. Instead of comparing files line-by-line, it parses them into their logical structure and merges at the semantic level. The crate is called `suture-merge`, and version 0.2 adds three-way semantic merge for DOCX, XLSX, and PPTX files -- which, as far as I can tell, makes it the only Rust crate on crates.io that can do this.

The key insight is that Office Open XML formats are not actually binary. They are ZIP archives containing XML files. A `.docx` file is really a `word/document.xml` (the paragraphs, tables, and formatting), some styles, some metadata, and a few other pieces. Suture unzips the archive, parses the XML, computes diffs at the paragraph and table level, performs a three-way merge, and reconstructs a valid `.docx` file.

The API looks like this:

```rust
use suture_merge::{merge_docx, MergeStatus};

let result = merge_docx(&base, &ours, &theirs)?;
match result.status {
    MergeStatus::Clean => println!("merged cleanly"),
    MergeStatus::Conflict => println!("has conflicts -- inspect result.merged"),
}
```

Three arguments, one return value. `base` is the common ancestor, `ours` is your version, `theirs` is theirs. The function returns a `MergeResult` containing the merged document and a status indicating whether the merge was clean or had conflicts.

## A real scenario

Here is the situation that motivated this work. A team shares a project requirements document (`requirements.docx`) in Git. Alice rewrites the third paragraph to clarify the acceptance criteria. Bob adds a new fourth paragraph with performance requirements. Both branch off `main` and submit pull requests.

With Git, the second person to merge gets a binary conflict. Someone has to open both copies of the document in Word, figure out what changed, and manually reconstruct a combined version. This is tedious, error-prone, and does not scale.

With Suture, both changes are independent -- Alice touched paragraph 3, Bob added paragraph 4 -- so the merge is clean. The driver tracks blocks by index: paragraphs (`<w:p>`) and tables (`<w:tbl>`) are the merge units. Formatting (bold, italic, fonts, styles) is preserved because the merge operates on the raw XML of each block, not on extracted plain text. Tables are treated as atomic units, so a table edit does not interfere with paragraph edits.

The same applies to Excel and PowerPoint. In XLSX, the merge operates on sheets and cells. In PPTX, it operates on slides and their constituent elements. The driver knows the internal structure of each format and merges accordingly.

## Text formats too

Office documents got the headline, but Suture also handles 13 text-based formats: JSON, YAML, TOML, CSV, XML, Markdown, SVG, HTML, iCalendar, and RSS/Atom feeds. The merge logic is the same idea -- parse the structure, compute semantic diffs, check for overlapping changes, and produce a merged result.

For example, two developers editing different keys in the same JSON config:

```rust
use suture_merge::{merge_json, MergeStatus};

let base  = r#"{"host": "localhost", "port": 5432, "debug": false}"#;
let ours  = r#"{"host": "db.prod.example.com", "port": 5432, "debug": false}"#;
let theirs = r#"{"host": "localhost", "port": 5432, "debug": true}"#;

let result = merge_json(base, ours, theirs)?;
assert_eq!(result.status, MergeStatus::Clean);
// result.merged: {"host": "db.prod.example.com", "port": 5432, "debug": true}
```

Git would flag this as a conflict because the changed lines are in the same hunk. Suture sees that `host` and `debug` are independent keys and merges both changes cleanly.

## Feature flags and zero-cost abstractions

You only pay for what you use. Each format is behind a Cargo feature flag:

```toml
[dependencies]
suture-merge = { version = "0.2", features = ["json", "docx"] }
```

The default features are `json`, `yaml`, `toml`, and `csv`. Office document support is opt-in. If you do not enable the `docx` feature, the DOCX driver does not get compiled, and you do not pull in any ZIP handling dependencies. Enable everything with `features = ["all"]`.

## Using it as a Git merge driver

If you want this to work automatically during `git merge` without changing your workflow, Suture can install itself as a Git merge driver:

```sh
suture git driver install
```

This writes a merge driver script, configures Git's `merge.suture` section, and updates `.gitattributes` with the relevant file patterns. After that, every `git merge` on a supported format uses Suture transparently. Your team does not need to learn a new tool or change how they work.

## How it works internally

The merge process follows the same three steps regardless of format:

1. **Parse.** The format-specific driver parses all three versions (base, ours, theirs) into an intermediate representation. For DOCX, this means unzipping the archive, parsing `word/document.xml`, and extracting block-level elements (paragraphs and tables) along with their raw XML and plain text.

2. **Diff and patch.** The driver computes semantic diffs between base vs. ours and base vs. theirs. Each change is associated with a logical address -- for DOCX, that is a block index; for JSON, that is a JSON Pointer path like `/database/host`.

3. **Merge.** Two changes conflict only when their logical addresses overlap. If Alice changed paragraph 3 and Bob changed paragraph 7, the touch sets are disjoint and both patches apply cleanly. The driver then reconstructs the output by serializing the merged state back to the original format.

This is essentially a patch algebra: if the touch sets of two patches are disjoint, they commute, and the merge is deterministic regardless of application order.

## The numbers

- 16 format-aware drivers (13 text + 3 binary document)
- 316 tests, 0 failures
- Apache-2.0 license
- No unsafe code in the merge logic
- Works as a library, a CLI tool, or a Git merge driver

## Try it

```toml
[dependencies]
suture-merge = { version = "0.2", features = ["docx", "xlsx", "pptx"] }
```

- [suture-merge on crates.io](https://crates.io/crates/suture-merge)
- [Suture on GitHub](https://github.com/WyattAu/suture)

If you hit a bug or have a format you would like supported, open an issue. If you find this useful, a star on the repo helps more than you would think.
