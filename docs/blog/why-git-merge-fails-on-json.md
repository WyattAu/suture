# Why Git Merge Fails on JSON (And How to Never See a Config Conflict Again)

You've seen this error: `CONFLICT (content): Merge conflict in package.json`. You open the file, stare at the `<<<<<<< HEAD` markers, and manually pick the right values. Maybe you've even lost a dependency because you picked the wrong side. Here's why it happens and how to never see it again.

## The Problem: Git Doesn't Understand JSON

Git is brilliant at merging text. It uses a **three-way merge algorithm**: it finds a common ancestor (the base), compares both branches against it, and applies non-overlapping changes. The key word is *text*. Git diffs line by line.

Here's the issue. Imagine this `package.json` on your `main` branch (the base):

```json
{
  "name": "my-app",
  "version": "1.0.0",
  "dependencies": {
    "express": "^4.18.0"
  },
  "scripts": {
    "start": "node index.js",
    "test": "jest"
  }
}
```

Your teammate adds a dependency on `feature/auth`:

```json
{
  "name": "my-app",
  "version": "1.0.0",
  "dependencies": {
    "express": "^4.18.0",
    "jsonwebtoken": "^9.0.0"
  },
  "scripts": {
    "start": "node index.js",
    "test": "jest"
  }
}
```

Meanwhile, you bump the version on `main`:

```json
{
  "name": "my-app",
  "version": "1.1.0",
  "dependencies": {
    "express": "^4.18.0"
  },
  "scripts": {
    "start": "node index.js",
    "test": "jest"
  }
}
```

These changes touch completely different keys: `version` vs `dependencies.jsonwebtoken`. A human reads this and says "obviously merge both." Git reads it and says:

```
CONFLICT (content): Merge conflict in package.json
<<<<<<< HEAD
  "version": "1.1.0",
  "dependencies": {
    "express": "^4.18.0"
  },
=======
  "version": "1.0.0",
  "dependencies": {
    "express": "^4.18.0",
    "jsonwebtoken": "^9.0.0"
  },
>>>>>>> feature/auth
```

Why? Because the addition of `"jsonwebtoken"` shifted every subsequent line down by one. Git's line-based diff sees that the `"version"` line changed on one side and the `"express"` line changed on the other. They overlap in the diff context window. Conflict.

This isn't a bug in Git. It's a fundamental limitation of **line-based merging applied to structured data**. The same problem occurs in YAML (Kubernetes manifests, CI configs), TOML (Rust `Cargo.toml`), XML, and CSV files. Any format where semantic meaning lives in keys and values rather than line positions is vulnerable.

## Before and After: Git vs Suture

Let's run the same merge with Suture:

```
$ suture merge feature/auth
Auto-merging package.json
Clean merge. 2 patches applied.
```

That's it. No conflict markers. No manual resolution. Suture produces:

```json
{
  "name": "my-app",
  "version": "1.1.0",
  "dependencies": {
    "express": "^4.18.0",
    "jsonwebtoken": "^9.0.0"
  },
  "scripts": {
    "start": "node index.js",
    "test": "jest"
  }
}
```

Both changes applied. Both keys correct. Zero human intervention.

## How Semantic Merge Works

Suture replaces Git's line-based diff with a three-step pipeline:

### 1. Parse

Suture reads the file using a format-specific parser. For JSON, it builds a tree of objects, arrays, and values. For YAML, it preserves anchors and aliases. For DOCX, it extracts paragraphs with their styles. The parser understands the structure, not just the text.

### 2. Compare (Key-Path Diffing)

Instead of comparing lines, Suture compares **key paths** (RFC 6901 JSON Pointer notation). The base, ours, and theirs versions are all parsed into trees. Suture walks the trees simultaneously and identifies what changed at each node:

- `$.version` changed from `"1.0.0"` to `"1.1.0"` (ours)
- `$.dependencies.jsonwebtoken` was added (theirs)
- Everything else is identical

Because these changes target different key paths, they don't conflict.

### 3. Merge

Suture applies all non-conflicting changes to the base tree, then serializes the result back to the original format with consistent formatting. The output is clean, well-formatted, and contains exactly the union of both branches' changes.

## Key-Path Diffing vs Line-Based Diffing

| | Line-based (Git) | Semantic (Suture) |
|---|---|---|
| Compares | Lines of text | Key paths and values |
| Understands | Nothing about structure | Object keys, array indices, nesting |
| Formatting changes | Cause conflicts | Ignored during comparison |
| Key reordering | Causes conflicts | No effect on merge result |
| Different keys, same object | Often conflicts | Never conflicts |

## Supported Formats

Suture ships with 17+ semantic drivers:

| Format | Extensions | Merge Granularity |
|--------|-----------|-------------------|
| JSON | `.json` | Field-level (RFC 6901 paths) |
| YAML | `.yaml` `.yml` | Key-level |
| TOML | `.toml` | Table and key-aware |
| CSV | `.csv` | Row-level with header detection |
| XML | `.xml` | Element/attribute-aware |
| Markdown | `.md` | Section-aware |
| HTML | `.html` | DOM-aware |
| SVG | `.svg` | Element-aware |
| DOCX | `.docx` | Paragraph-level |
| XLSX | `.xlsx` | Cell-level |
| PPTX | `.pptx` | Slide-level |
| SQL | `.sql` | DDL schema diff |
| PDF | `.pdf` | Page-level text diff |
| Image | `.png` `.jpg` `.gif` `.webp` | Metadata diff |
| OTIO | `.otio` | OpenTimelineIO editorial merge |
| iCalendar | `.ics` | Event-level merge |
| RSS/Atom | `.rss` `.atom` | Feed and entry-aware |

Files without a driver fall back to line-based merge, identical to Git's behavior.

## How to Install

```bash
# Prebuilt binary (fastest)
curl -fsSL https://github.com/WyattAu/suture/releases/latest/download/suture-linux-x86_64 -o /usr/local/bin/suture && chmod +x /usr/local/bin/suture

# Cargo
cargo install suture-cli

# Homebrew (macOS / Linux)
brew tap WyattAu/suture-merge-driver && brew install suture-merge-driver

# npm
npm install -g suture-merge-driver

# pip
pip install suture-merge-driver
```

## Use as a Git Merge Driver

Add semantic merging to your existing Git repos in 30 seconds:

```bash
suture git driver install
git add .gitattributes .suture/git-merge-driver.sh
git commit -m "Configure suture semantic merge driver"
```

Future merges on JSON, YAML, TOML, DOCX, XLSX, and 17 other file types will use semantic merge automatically. No workflow changes. No new tools to learn. Your team keeps using Git exactly as before — they just stop seeing config file conflicts.

## Try It Yourself

Head to the [interactive merge demo](https://suture.dev/merge) to paste two versions of a JSON, YAML, TOML, or CSV file and watch them merge cleanly. No signup required.

Stop resolving merge conflicts that a computer should handle. Install Suture and get back to writing code.
