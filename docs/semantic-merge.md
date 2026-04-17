# Semantic Merge

## What It Is

Suture's semantic merge understands the internal structure of your files. Instead of comparing files line-by-line (like Git), Suture uses format-aware drivers that parse file structure and detect changes at the logical level -- keys, elements, rows, slides, cells.

Two patches conflict only when they modify the same logical address. Changing different JSON keys, different CSV rows, or different slides in a PPTX never conflicts.

## How It Works

1. A **driver** parses the file into a structured intermediate representation
2. Changes are mapped to **logical addresses** (e.g., `database/host`, `sheet1/A3`)
3. The patch algebra checks for **touch set overlap** -- only overlapping addresses cause conflicts
4. When merging, the driver reconstructs the file from both sides' changes

If no driver matches a file's extension, Suture falls back to line-based merge (same behavior as Git).

## Supported Formats

| Format | Extensions | Driver | What It Understands |
|--------|-----------|--------|-------------------|
| JSON | `.json` | `suture-driver-json` | Keys, nested objects, arrays (RFC 6901 paths) |
| YAML | `.yaml`, `.yml` | `suture-driver-yaml` | Anchors, aliases, mappings, sequences |
| TOML | `.toml` | `suture-driver-toml` | Tables, inline tables, arrays |
| CSV | `.csv` | `suture-driver-csv` | Rows, columns, headers |
| XML | `.xml` | `suture-driver-xml` | Elements, attributes, namespaces |
| Markdown | `.md` | `suture-driver-markdown` | Headings, code blocks, lists, tables, links |
| SQL | `.sql` | `suture-driver-sql` | Tables, columns, indexes, constraints (DDL) |
| DOCX | `.docx` | `suture-driver-docx` | Paragraphs, sections |
| XLSX | `.xlsx` | `suture-driver-xlsx` | Cells, sheets |
| PPTX | `.pptx` | `suture-driver-pptx` | Slides |
| PDF | `.pdf` | `suture-driver-pdf` | Text content, pages |
| Image | `.png` `.jpg` `.gif` `.bmp` `.webp` `.tiff` `.ico` `.avif` | `suture-driver-image` | Dimensions, color type, format |
| OTIO | `.otio` | `suture-driver-otio` | Clips, tracks, timeline structure |

## Example: JSON Config Merge

Two developers edit `config.json` independently:

**Base (committed on `main`):**
```json
{
  "database": {
    "host": "localhost",
    "port": 5432
  },
  "logging": {
    "level": "info"
  }
}
```

**Alice** (on branch `feature/logging`) changes `logging.level`:
```json
{
  "database": {
    "host": "localhost",
    "port": 5432
  },
  "logging": {
    "level": "debug"
  }
}
```

**Bob** (on `main`) changes `database.port`:
```json
{
  "database": {
    "host": "localhost",
    "port": 3306
  },
  "logging": {
    "level": "info"
  }
}
```

**`suture merge feature/logging`** produces:
```json
{
  "database": {
    "host": "localhost",
    "port": 3306
  },
  "logging": {
    "level": "debug"
  }
}
```

Both changes applied. No conflict.

## Contrast with Git

Git treats files as opaque text. When both sides modify the same region (even different keys on adjacent lines), Git inserts conflict markers:

```
<<<<<<< HEAD
    "port": 3306
=======
    "level": "debug"
>>>>>>> feature/logging
```

Suture knows these are different JSON keys (`database/port` vs `logging/level`) and merges them automatically.

## Standalone File Merge

You can use Suture's semantic merge outside of a repository:

```bash
suture merge-file base.json ours.json theirs.json
suture merge-file --driver yaml -o merged.yaml base.yaml ours.yaml theirs.yaml
```

The `--driver` flag selects a specific driver. If omitted, Suture auto-detects by file extension.

## Writing a Custom Driver

Implement the `SutureDriver` trait to add support for a new format:

1. Create a crate: `crates/suture-driver-<format>/`
2. Depend on `suture-driver` and implement `SutureDriver`
3. Implement `diff()` to produce `SemanticChange` values between two file versions
4. Implement `format_diff()` for human-readable output
5. Implement `merge()` for three-way semantic merge (return `None` on genuine conflict)
6. Return supported extensions from `supported_extensions()`

See `crates/suture-driver-example/` for a minimal working driver.

### Element ID Convention

Use slash-delimited paths that reflect the document hierarchy:

| Format | Example Element IDs |
|--------|-------------------|
| JSON | `database/host`, `logging/level` |
| XLSX | `workbook/sheet/rows/3/cols/A` |
| DOCX | `doc/body/paragraphs/5/runs/2` |
| OTIO | `timeline/track/clip`, `timeline/stack` |

### Commutativity

Two patches commute when their touch sets are disjoint. The driver only needs to compute correct touch sets -- Suture's patch algebra handles the rest.

## Fallback

Files without a matching driver use line-based diff and merge, identical to Git's behavior. This covers source code (`.rs`, `.py`, `.js`, etc.) and any other text format.
