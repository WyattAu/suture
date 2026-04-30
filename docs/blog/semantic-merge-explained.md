# Semantic Merge Explained: How Structure-Aware Merging Actually Works

Every developer has fought a merge conflict that shouldn't have been a conflict. Two people changed different keys in the same JSON file, or one person reformatted a YAML file while another added a field, and Git declares war. The problem isn't Git — it's that line-based merging doesn't understand file structure. Semantic merge does.

## What Is Semantic Merge?

Semantic merge is a merging strategy that operates on the **parsed structure** of a file rather than its raw text. Where Git sees lines, semantic merge sees objects, keys, arrays, elements, and attributes.

Consider a JSON file:

```json
{"host": "localhost", "port": 3000}
```

A line-based tool sees two lines. A semantic merge tool sees a JSON object with two key-value pairs at paths `$.host` and `$.port`. This distinction is the difference between a clean merge and a conflict.

## Line-Based Merge Limitations

Git's three-way merge algorithm (based on the `diff3` approach) works by:

1. Finding the lowest common ancestor commit (the base).
2. Computing a line-based diff from base to each branch.
3. Applying non-overlapping hunks from both branches.

This works well for prose and code where lines are the natural unit of meaning. It breaks down for structured formats in three common scenarios:

### Reformatting

A teammate runs a code formatter that changes indentation from 2 spaces to 4 spaces. Every line changes. Any other edit to the same file, no matter how small, produces a conflict. Semantic merge parses first and compares structure, so formatting is invisible to the diff.

### Key Reordering

JSON objects, YAML mappings, and XML attributes have no defined order. Two branches that add different keys to the same object will produce a conflict in Git if either branch also happens to reorder existing keys. Semantic merge compares by key path, so order is irrelevant.

### Nested Structures

Deeply nested JSON (common in Kubernetes manifests, AWS CloudFormation templates, and OpenAPI specs) makes line-based diffs fragile. A change to a nested field shifts all subsequent lines, creating phantom conflicts with unrelated changes elsewhere in the file. Semantic merge walks the tree, so depth doesn't matter.

## Semantic Merge Algorithms

### Key-Path Diffing

Suture parses each version of the file into an abstract syntax tree (AST). It then walks the trees from all three versions (base, ours, theirs) simultaneously, comparing nodes at each level using their key paths.

For JSON and YAML, this means comparing object keys. For XML, it means comparing element tags and attributes. For CSV, it means comparing column headers and row indices.

A change at path `$.dependencies.express` on one branch and a change at `$.dependencies.fastify` on another branch are independent — different keys, no conflict.

### Array Strategies

Arrays are the trickiest part of semantic merge because array elements don't have unique identifiers. Suture supports two configurable strategies:

- **Concat**: Elements added to the end of an array by either branch are all kept. This works well for dependency lists, script arrays, and similar append-only patterns.
- **Replace**: If either branch replaces the entire array, the replacement wins. This is appropriate for arrays where order matters, like build steps or middleware chains.

### Conflict Detection

Semantic merge only reports a conflict when both branches modify the **same key path to different values**. For example, if one branch sets `$.port` to `3000` and another sets it to `8080`, that's a genuine conflict that requires human judgment. Everything else merges cleanly.

## Three-Way Merge Theory

The three-way merge model requires three inputs:

| Input | Description |
|-------|-------------|
| **Base** | The common ancestor — the version both branches started from |
| **Ours** | The current branch's version |
| **Theirs** | The incoming branch's version |

The algorithm computes two diffs: base→ours and base→theirs. It then attempts to apply both sets of changes to the base simultaneously. Changes that don't overlap are applied automatically. Changes that touch the same location with different values are flagged as conflicts.

Semantic merge uses the same three inputs but replaces "same location" with "same key path." This is strictly more precise for structured files because key paths capture the actual unit of change.

## Suture's Approach

Suture implements per-format **merge drivers**, each tailored to the semantics of its format:

- **JSON/YAML/TOML drivers** perform key-path diffing on parsed data structures.
- **XML/HTML/SVG drivers** work on the DOM, merging elements and attributes independently.
- **DOCX/XLSX/PPTX drivers** decompose Office Open XML packages into their constituent parts (paragraphs, cells, slides) and merge at that granularity.
- **CSV drivers** detect headers and merge row-by-row, handling column additions and reordering.
- **Markdown drivers** split files into sections by headings and merge section-by-section.

Each driver is configurable. You can set array merge strategies, conflict handling behavior, and formatting preferences per file pattern in your project's Suture configuration.

## When Semantic Merge Can't Help

Semantic merge isn't magic. There are cases where it can't avoid a conflict:

- **Binary files** without a structured parser (compiled binaries, encrypted files) fall back to line-based merge or require manual resolution.
- **Genuinely ambiguous changes** — when both branches modify the same key to different values, no algorithm can decide which is correct. That's a human decision.
- **Lossy formats** — if a round-trip through the parser changes the file (e.g., stripping comments or reordering keys), the merge result may differ from what either branch intended.

For all other cases — different keys in the same object, additions to the same array, formatting changes alongside content changes — semantic merge produces clean, correct results that line-based merge cannot.

## Get Started

Install Suture and add it to your Git workflow:

```bash
cargo install suture-cli
suture git driver install
```

Your team will stop seeing merge conflicts in config files from the very next merge.
