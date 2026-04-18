# Suture for Document Authors

## The Problem

Collaborating on Word documents, Excel spreadsheets, and PowerPoint decks is broken:

- **Track Changes** in Word works for one person at a time. Open it on two machines and you get conflicting copies.
- **Google Docs** loses formatting — tables break, images shift, styles don't survive round-trips.
- **Emailing versions** (`Report_v3_FINAL_Jane.docx`) makes it impossible to see what changed or merge work.
- **Git** treats `.docx` and `.xlsx` as opaque binary blobs. A one-character text change in a Word doc produces a completely different ZIP file — Git sees 100% conflict.

## How Suture Helps

Suture opens the document structure and merges at the logical level:

| Format | Merge granularity |
|--------|-------------------|
| DOCX | Paragraph-level — two people editing different paragraphs never conflict |
| XLSX | Cell-level — changes to different cells merge cleanly |
| PPTX | Slide-level — edits on different slides combine automatically |

## Example: Two People Editing the Same DOCX

Alice edits the executive summary. Bob edits the financial projections. They commit to different branches.

```bash
# Alice's branch
$ suture diff main..alice report.docx

 paragraph 3 (Executive Summary)
-  "Revenue grew modestly in Q3."
+  "Revenue grew 34% in Q3, driven by enterprise contracts."

1 paragraph changed, 0 conflicts

# Bob's branch
$ suture diff main..bob report.docx

 paragraph 12 (Financial Projections)
-  "Projected ARR: $2.1M"
+  "Projected ARR: $2.8M"

1 paragraph changed, 0 conflicts

# Merge both
$ suture checkout main
$ suture merge alice
$ suture merge bob
# Both changes applied. No conflict markers. No lost formatting.
```

## 5-Minute Setup

### 1. Install Suture

```bash
cargo install suture-cli
```

### 2. Initialize a project

```bash
mkdir shared-docs && cd shared-docs
suture init
suture config user.name "Alice"
```

### 3. Add your documents

```bash
cp /path/to/report.docx .
cp /path/to/budget.xlsx .
suture add . && suture commit "initial documents"
```

### 4. Collaborate

```bash
# Alice edits report.docx, commits
suture add . && suture commit "update executive summary"

# Bob pulls, edits budget.xlsx, commits
suture pull
suture add . && suture commit "Q4 budget revisions"

# Or use branches for larger changes
suture branch redesign
suture checkout redesign
# Make major edits to report.docx
suture add . && suture commit "redesign layout and charts"
suture checkout main
suture merge redesign
```

### 5. Use the FUSE mount for seamless editing

```bash
suture vfs mount . /mnt/docs
# Open report.docx from /mnt/docs in Word
# When you save, Suture creates a patch automatically
# Run `suture status` to see what changed
```

## Comparison

| | Suture | Git LFS | Google Docs | Track Changes |
|---|---|---|---|---|
| Merge DOCX semantically | Yes — paragraph-level | No — binary blob | N/A (single doc) | No — sequential only |
| Merge XLSX semantically | Yes — cell-level | No — binary blob | N/A | No |
| Multiple editors simultaneously | Branch + merge workflow | Branch + merge (broken for docs) | Yes (but conflicts overwrite) | No |
| Preserves formatting | Yes | Yes (as blob) | Partially | Yes |
| Works offline | Yes | Yes | No | Yes |
| See what changed | `suture diff` at paragraph/cell level | File-level only | Version history (limited) | Accept/reject changes |
| No server required | Yes | Yes | No (Google account) | Yes |

## Use Cases

**Proposal writing** — Branch for each client variant, merge shared sections back.

**Budget spreadsheets** — Two departments update their own sheets in the same workbook. Suture merges cell-level changes.

**Slide decks** — Marketing updates slides 1–5, sales updates slides 8–12. Both changes merge cleanly.

**Legal documents** — Track every clause change with full attribution. Revert a specific paragraph without touching the rest.

## See Also

- [Why Suture?](why-suture.md) — how semantic understanding works
- [Suture vs. Git](comparing-with-git.md) — honest comparison
- [Quick Start](quickstart.md) — general setup guide
