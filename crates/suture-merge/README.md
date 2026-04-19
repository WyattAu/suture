# suture-merge

> Semantic merge for structured files. 3 lines of code.

## What?

Git merges files line-by-line. That works for code, but breaks for structured data like JSON, YAML, and CSV. `suture-merge` understands the *semantics* of your file format and merges accordingly.

## Why?

**The problem with line-based merge:**

```json
// Base
{"name": "Alice", "age": 30}

// Person A changes age
{"name": "Alice", "age": 31}

// Person B adds city
{"name": "Alice", "city": "NYC"}
```

Git sees these as conflicting changes on overlapping lines. But semantically, they're independent — one changes `age`, the other adds `city`.

**With `suture-merge`:**

```rust
use suture_merge::{merge_json, MergeStatus};

let result = merge_json(
    r#"{"name": "Alice", "age": 30}"#,
    r#"{"name": "Alice", "age": 31}"#,
    r#"{"name": "Alice", "city": "NYC"}"#,
)?;

assert_eq!(result.status, MergeStatus::Clean);
// result.merged contains both changes
```

## Supported Formats

| Format | Feature flag | Status |
|--------|-------------|--------|
| JSON | `json` (default) | Supported |
| YAML | `yaml` (default) | Supported |
| TOML | `toml` (default) | Supported |
| CSV | `csv` (default) | Supported |
| XML | `xml` | Supported |
| Markdown | `markdown` | Supported |
| SVG | `svg` | Supported |
| HTML | `html` | Supported |
| iCalendar | `ical` | Supported |
| RSS/Atom | `feed` | Supported |

## Install

```toml
[dependencies]
suture-merge = "0.1"
```

Or with specific formats:

```toml
[dependencies]
suture-merge = { version = "0.1", features = ["json", "yaml"] }
```

Enable everything:

```toml
[dependencies]
suture-merge = { version = "0.1", features = ["all"] }
```

## API

```rust
use suture_merge::{merge_json, merge_yaml, merge_toml, merge_csv,
                   merge_xml, merge_markdown, merge_svg, merge_html,
                   merge_ical, merge_feed, merge_auto, diff, format_diff,
                   MergeResult, MergeStatus, MergeError};

// Format-specific (zero overhead for unused formats)
let result: MergeResult = merge_json(base, ours, theirs)?;
let result: MergeResult = merge_yaml(base, ours, theirs)?;
let result: MergeResult = merge_toml(base, ours, theirs)?;
let result: MergeResult = merge_csv(base, ours, theirs)?;
let result: MergeResult = merge_xml(base, ours, theirs)?;
let result: MergeResult = merge_markdown(base, ours, theirs)?;
let result: MergeResult = merge_svg(base, ours, theirs)?;
let result: MergeResult = merge_html(base, ours, theirs)?;
let result: MergeResult = merge_ical(base, ours, theirs)?;
let result: MergeResult = merge_feed(base, ours, theirs)?;

// Auto-detect from file extension
let result: MergeResult = merge_auto(base, ours, theirs, Some(".json"))?;

// Semantic diff
let changes = diff(base, modified, Some(".json"))?;
let readable = format_diff(base, modified, Some(".json"))?;
```

## Three-Way Merge

Like Git, `suture-merge` uses three-way merge: you provide a **base** (common ancestor), **ours** (our changes), and **theirs** (their changes). The merge succeeds when changes don't conflict.

## MergeResult

Every merge function returns a `MergeResult`:

```rust
pub struct MergeResult {
    pub merged: String,     // The merged content
    pub status: MergeStatus, // Clean or Conflict
}

pub enum MergeStatus {
    Clean,    // No conflicts
    Conflict, // Conflicts detected (merged contains "ours" as best-effort)
}
```

## License

Apache-2.0
