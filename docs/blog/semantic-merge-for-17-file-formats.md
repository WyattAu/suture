# Semantic Merge for 17 File Formats

*Stop losing work to Git merge conflicts on JSON, YAML, Word docs, and Excel spreadsheets.*

---

Every developer knows the feeling. You and a coworker both edit `config.yaml`. You change the database host. They change the server port. Git can't tell that these are different fields — it just sees overlapping line changes and spits out conflict markers. You manually resolve, hoping you didn't drop a comma.

Now imagine the same problem with a Word document. Git treats `.docx` as a binary blob. Two people edit different paragraphs? Git can't merge that at all. "CONFLICT (content): Merge conflict in proposal.docx." Your options: keep one version, keep the other, or manually copy-paste paragraphs in Word.

**Suture fixes this.** It's a semantic merge driver that understands the structure of your files — not just their text.

## How It Works

Git's default merge is line-based. Suture's merge is structure-based:

| File Type | Git Sees | Suture Sees |
|-----------|----------|-------------|
| JSON | Lines of text | Fields, arrays, nested objects |
| YAML | Lines of text | Keys, mappings, anchors |
| DOCX | Binary blob | Paragraphs, tables, headings |
| XLSX | Binary blob | Cells, formulas, sheets |
| CSV | Lines of text | Rows with typed columns |
| SQL | Lines of text | CREATE/ALTER statements |

When two people edit different JSON fields, Suture merges them cleanly. When two people edit different Word paragraphs, Suture merges them cleanly. When two people edit different Excel cells, Suture merges them cleanly.

It's not magic — it's parsing. Suture reads the file format, identifies the structural units (fields, paragraphs, cells), performs a three-way merge at that granularity, and writes the result back.

## The 5-Minute Setup

```bash
brew tap WyattAu/suture-merge-driver
brew install suture-merge-driver

cd your-git-repo
suture git driver install
git add .gitattributes .suture/git-merge-driver.sh
git commit -m "Configure suture semantic merge driver"
```

That's it. No configuration files to edit. No Rust toolchain to install. No daemon to run. The driver script is a thin shell wrapper — Git calls it on conflicts, it calls Suture, Suture does the merge.

## Example: Kubernetes YAML

Three engineers deploy to the same `deployment.yaml`:

- **Alice** changes the container image tag
- **Bob** increases the replica count
- **Carol** adds an environment variable

With Git, this is guaranteed conflict markers. With Suture, all three changes merge cleanly because they touch different YAML keys:

```yaml
spec:
  replicas: 5            # Bob's change
  template:
    spec:
      containers:
      - image: app:v2.1  # Alice's change
        env:
        - name: LOG_LEVEL
          value: "debug"  # Carol's change
```

## Example: Word Document

A marketing team collaborates on `proposal.docx`:

- **Writer A** rewrites the executive summary (paragraphs 1-3)
- **Writer B** updates the pricing table (paragraph 8)
- **Writer C** adds a new case study (paragraph 15)

Suture merges all three changes at the paragraph level. No binary conflict. No "Copy of proposal_FINAL_v2.docx." No lost work.

## Example: Excel Spreadsheet

Finance team edits `budget.xlsx`:

- **Analyst A** updates Q1 revenue formulas
- **Analyst B** adds rows for new product lines
- **Analyst C** changes the summary sheet's chart data range

Suture merges at the cell level. Formulas, formatting, and cross-sheet references are preserved.

## Supported Formats

JSON, JSONL, YAML, TOML, CSV, TSV, XML, XSL, SVG, Markdown, DOCX, DOCM, XLSX, XLSM, PPTX, PPTM, SQL, and OTIO (OpenTimelineIO for video editing). Files without a driver fall back to Git's default merge.

## Under the Hood

Suture is written in Rust (1,245+ tests, zero unsafe in production path). The merge algorithm:

1. **Parse** all three versions (base, ours, theirs) into an AST
2. **Diff** base→ours and base→theirs at the structural level
3. **Merge** non-overlapping changes automatically
4. **Conflict-mark** truly overlapping changes (same field/paragraph/cell)
5. **Serialize** the merged AST back to the original format

For DOCX/XLSX/PPTX, Suture reads the ZIP container, parses the internal XML, merges at the structural level, and writes a valid OOXML file back. The result opens cleanly in Microsoft Office, LibreOffice, and Google Docs.

## Standalone Usage

You don't need Git. Suture works as a standalone merge tool:

```bash
suture merge-file base.json ours.json theirs.json
suture merge-file --driver docx base.docx ours.docx theirs.docx -o merged.docx
```

## Performance

On a release build, Suture merges a 100-file JSON repository in under a second. Startup is 3ms. A single-file commit takes 14ms. It's fast enough to be invisible in your workflow.

## Install

```bash
# macOS
brew tap WyattAu/suture-merge-driver
brew install suture-merge-driver

# Linux / Windows
# Download from https://github.com/WyattAu/suture/releases

# npm (Node.js wrapper)
npm install -g suture-merge-driver

# Python
pip install suture-merge-driver

# Cargo (Rust)
cargo install suture-cli
```

## Open Source

Suture is Apache 2.0 licensed. The source is at [github.com/WyattAu/suture](https://github.com/WyattAu/suture). Contributions welcome — especially new format drivers.

## Links

- **GitHub:** [github.com/WyattAu/suture](https://github.com/WyattAu/suture)
- **Documentation:** [wyattau.github.io/suture](https://wyattau.github.io/suture/)
- **Homebrew Tap:** [github.com/WyattAu/homebrew-suture-merge-driver](https://github.com/WyattAu/homebrew-suture-merge-driver)
- **npm:** [npmjs.com/package/suture-merge-driver](https://www.npmjs.com/package/suture-merge-driver)
- **crates.io:** [crates.io/crates/suture-cli](https://crates.io/crates/suture-cli)
