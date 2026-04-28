# Quickstart: Stop Losing Work to Google Drive

You've been there. Two people edit the same Word doc. Someone saves over someone else's work. Google Drive creates "Copy of proposal_FINAL_v2_REAL.docx." There has to be a better way. There is.

---

## Install (10 seconds)

Head to [github.com/WyattAu/suture/releases](https://github.com/WyattAu/suture/releases), grab the binary for your OS, and drop it somewhere on your PATH. On a Mac with Homebrew:

```bash
brew tap WyattAu/suture-merge-driver
brew install suture-merge-driver
```

On Linux or Windows, download from GitHub Releases and unzip. That's it. No Rust toolchain needed.

Verify it works:

```
$ suture --version
suture 5.0.1
```

---

## Your First Repository (10 seconds)

```bash
cd ~/Documents
suture init
```

```
Initialized empty Suture repository in /Users/you/Documents/.suture/
```

Track your proposal:

```bash
suture add proposal.docx
suture commit "initial draft"
```

```
[main abc1234] initial draft
 1 file changed, 1 insertion (+1), 0 deletions (-0)
```

Done. Suture now knows about every paragraph, table, and heading in that document.

---

## Branch and Edit (15 seconds)

Your coworker Alice needs to update the executive summary while you rework the pricing section. No more "I'll wait for you to finish."

```bash
suture branch alice-edits
suture checkout alice-edits
```

```
Switched to branch 'alice-edits'
```

Alice opens `proposal.docx` in Word and rewrites paragraph 2 (the executive summary). She saves, then:

```bash
suture add proposal.docx
suture commit "updated executive summary"
```

```
[alice-edits def5678] updated executive summary
 1 file changed, 1 insertion (+1), 1 deletion (-1)
```

---

## Merge Without Tears (15 seconds)

Meanwhile, you've been on `main`, editing paragraph 5 (the pricing table). You saved and committed:

```bash
suture checkout main
# (you edited paragraph 5 in Word)
suture add proposal.docx
suture commit "updated pricing table"
```

Now bring Alice's changes in:

```bash
suture merge alice-edits
```

```
Merge made by the 'ort' strategy.
DOCX merge: proposal.docx
  Merged 2 paragraph-level changes (0 conflicts)
  Paragraph 2: updated executive summary  (from alice-edits)
  Paragraph 5: updated pricing table       (from main)
Clean merge. 2 patches applied.
```

Open `proposal.docx`. Alice's new executive summary is there. Your pricing table is there. Nothing got overwritten. No conflict markers. No "Copy of" files.

```bash
suture log --oneline
```

```
*   789abcd Merge branch 'alice-edits'
|\
| * def5678 updated executive summary
* | ghi0123 updated pricing table
|/
* abc1234 initial draft
```

---

## What Just Happened?

Google Drive (and Dropbox, OneDrive, etc.) treat your `.docx` file as a black box of bytes. When two people save changes, the last save wins. Period.

Suture cracks open the DOCX file and reads its *structure* -- paragraphs, headings, tables, images. When Alice changed paragraph 2 and you changed paragraph 5, Suture saw two independent edits to two different parts of the document. It merged them automatically.

This is called **semantic merge**. It works the same way for Excel (cell-level), PowerPoint (slide-level), JSON (field-level), YAML (key-level), and a dozen other formats.

The result: version control for everything, not just code.

---

## Next Steps

- [Semantic Merge Deep Dive](semantic-merge.md) -- how it works under the hood
- [Document Authors Guide](document-authors.md) -- branching strategies for Word, Excel, PowerPoint
- [CLI Reference](cli-reference.md) -- the full command list
