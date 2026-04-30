# Git Merge Driver — Comprehensive Guide

## What is a Git Merge Driver?

Git's default merge strategy operates on **lines**. When two branches edit the same file, Git performs a three-way merge using the common ancestor, and marks conflicts where both sides changed overlapping lines. This works well for prose, but fails for structured data — a JSON file with two people changing different keys on adjacent lines will show a conflict even though the changes are logically independent.

A **Git merge driver** replaces Git's line-based merge for specific file types. When Git encounters a conflict on a file listed in `.gitattributes`, it invokes your custom driver instead of inserting conflict markers. The driver receives three files — the base (common ancestor), ours (your branch), and theirs (incoming branch) — and must write the merged result back to the ours file. Exit code 0 means clean; non-zero means conflict.

## Why Suture?

Suture understands the **structure** of your files. Instead of comparing lines, it parses JSON into objects, YAML into mappings, TOML into tables, and merges at the logical level:

- **JSON**: Field-level merge — two branches changing different keys never conflict
- **YAML**: Key-level merge — nested mappings merge recursively
- **TOML**: Table and key-aware — `[section]` headers guide the merge
- **CSV**: Row-level merge with header detection
- **XML**: Element and attribute-aware
- **Markdown**: Section-aware merge
- **DOCX/XLSX/PPTX**: Paragraph, cell, and slide-level merge (binary formats)

### Before / After

Given a `config.json` where two people edit simultaneously:

**Base:**
```json
{
  "database": {"host": "localhost", "port": 5432},
  "server": {"port": 3000, "workers": 4}
}
```

**Branch A** changes `database.host` to `db.example.com`.
**Branch B** changes `server.port` to `8080`.

**Without Suture** — Git produces a conflict:
```
<<<<<<< HEAD
  "database": {"host": "localhost", "port": 5432},
  "server": {"port": 8080, "workers": 4}
=======
  "database": {"host": "db.example.com", "port": 5432},
  "server": {"port": 3000, "workers": 4}
>>>>>>> feature/db-config
```

**With Suture** — clean merge, no conflicts:
```json
{
  "database": {"host": "db.example.com", "port": 5432},
  "server": {"port": 8080, "workers": 4}
}
```

Both changes are preserved automatically.

---

## Installation

### One-liner (recommended)

```bash
curl -fsSL https://raw.githubusercontent.com/WyattAu/suture/main/scripts/install-merge-driver.sh | bash
```

This detects your OS, installs Suture if needed (via cargo, brew, npm, pip, or binary download), configures Git merge drivers for all supported formats, and creates `.gitattributes`.

### Cargo

```bash
cargo install suture-cli
suture git driver install
```

### Homebrew

```bash
brew tap WyattAu/suture-merge-driver
brew install suture-merge-driver
```

### npm

```bash
npm install -g suture-merge-driver
```

### PyPI

```bash
pip install suture-merge-driver
```

### Binary download

Download from [GitHub Releases](https://github.com/WyattAu/suture/releases) and add to PATH.

---

## Configuration

### Global config (all repositories)

The one-liner installer sets this up automatically. To configure manually:

```bash
git config --global merge.json.name "Suture JSON merge driver"
git config --global merge.json.driver "suture merge-file --driver json %O %A %B -o %A"

git config --global merge.yaml.name "Suture YAML merge driver"
git config --global merge.yaml.driver "suture merge-file --driver yaml %O %A %B -o %A"

git config --global merge.toml.name "Suture TOML merge driver"
git config --global merge.toml.driver "suture merge-file --driver toml %O %A %B -o %A"

git config --global merge.xml.name "Suture XML merge driver"
git config --global merge.xml.driver "suture merge-file --driver xml %O %A %B -o %A"

git config --global merge.csv.name "Suture CSV merge driver"
git config --global merge.csv.driver "suture merge-file --driver csv %O %A %B -o %A"

git config --global merge.md.name "Suture Markdown merge driver"
git config --global merge.md.driver "suture merge-file --driver markdown %O %A %B -o %A"

git config --global merge.docx.name "Suture DOCX merge driver"
git config --global merge.docx.driver "suture merge-file --driver docx %O %A %B -o %A"
git config --global merge.docx.recursive "binary"

git config --global merge.xlsx.name "Suture XLSX merge driver"
git config --global merge.xlsx.driver "suture merge-file --driver xlsx %O %A %B -o %A"
git config --global merge.xlsx.recursive "binary"

git config --global merge.pptx.name "Suture PPTX merge driver"
git config --global merge.pptx.driver "suture merge-file --driver pptx %O %A %B -o %A"
git config --global merge.pptx.recursive "binary"
```

Then create a global `.gitattributes`:

```bash
cat >> ~/.gitattributes << 'EOF'
*.json merge=json
*.jsonl merge=json
*.yaml merge=yaml
*.yml merge=yaml
*.toml merge=toml
*.xml merge=xml
*.csv merge=csv
*.tsv merge=csv
*.md merge=md
*.markdown merge=md
*.docx merge=docx
*.xlsx merge=xlsx
*.pptx merge=pptx
EOF
```

### Per-repo config (single repository)

```bash
cd /path/to/your/repo

git config merge.json.name "Suture JSON merge driver"
git config merge.json.driver "suture merge-file --driver json %O %A %B -o %A"
# ... repeat for other formats

cat > .gitattributes << 'EOF'
*.json merge=json
*.yaml merge=yaml
*.toml merge=toml
EOF

git add .gitattributes
git commit -m "Configure suture merge driver"
```

Or use the built-in command:

```bash
suture git driver install
git add .gitattributes .suture/git-merge-driver.sh
git commit -m "Configure suture merge driver"
```

### Using `suture git driver` (built-in)

Suture's CLI includes a built-in driver installer that handles all configuration:

```bash
suture git driver install    # Install for current repo
suture git driver uninstall  # Remove driver from current repo
suture git driver list       # Show current driver status
```

This creates a shell wrapper at `.suture/git-merge-driver.sh` that calls `suture merge-file --driver auto`, registers it as `merge.suture`, and writes `.gitattributes` with 20 file patterns.

### `.gitattributes` reference

The `.gitattributes` file tells Git which merge driver to use for each file pattern:

```gitattributes
# Pattern                  Driver name
*.json merge=json          # Use the "json" merge driver
*.yaml merge=yaml          # Use the "yaml" merge driver
*.docx merge=docx          # Use the "docx" merge driver (binary)

# Path-based patterns
kubernetes/*.yaml merge=yaml    # Only Kubernetes manifests
configs/*.json merge=json       # Only config files

# Exclude specific paths
generated/*.json merge=default  # Skip semantic merge for generated files
```

Each `merge=<name>` entry references a `[merge "<name>"]` section in your git config. The driver name must match exactly.

### Custom driver arguments

You can pass additional flags to `suture merge-file`:

```bash
# Use auto-detection instead of explicit driver
git config merge.json.driver "suture merge-file --driver auto %O %A %B -o %A"

# Custom conflict labels
git config merge.json.driver "suture merge-file --driver json --label-ours HEAD --label-theirs feature %O %A %B -o %A"
```

---

## How It Works

When `git merge` encounters a conflict on a file matching a `.gitattributes` pattern, Git invokes the configured driver:

```
suture merge-file --driver json %O %A %B -o %A
```

Git substitutes the placeholders:

| Placeholder | Meaning | Description |
|-------------|---------|-------------|
| `%O` | Base | Common ancestor version of the file |
| `%A` | Ours | Current branch's version (result written here) |
| `%B` | Theirs | Incoming branch's version |
| `%P` | Path | Original file path (not used by merge-file) |

The merge-file command:
1. Reads all three files (binary-safe for DOCX/XLSX/PPTX)
2. Detects or uses the specified semantic driver
3. Performs structural merge at the appropriate granularity
4. Writes the merged result to the output file (`-o %A`)
5. Exits 0 on clean merge, non-zero on conflict

### Exit codes

| Code | Meaning | Git's response |
|------|---------|----------------|
| 0 | Clean merge | Accepts the merged result |
| 1 | Conflict | Falls back to standard conflict markers |
| 2+ | Error | Fails the merge |

### Standalone usage

You can also use `suture merge-file` directly outside of Git:

```bash
suture merge-file base.json ours.json theirs.json
suture merge-file --driver yaml -o merged.yaml base.yaml ours.yaml theirs.yaml
suture merge-file --driver docx base.docx ours.docx theirs.docx -o merged.docx
suture merge-file --label-ours HEAD --label-theirs feature base.json ours.json theirs.json
```

#### `suture merge-file` reference

```
suture merge-file [OPTIONS] <BASE> <OURS> <THEIRS>

Arguments:
  <BASE>    Common ancestor file path
  <OURS>    Current branch file path
  <THEIRS>  Incoming branch file path

Options:
  --driver <name>        Semantic driver (json, yaml, toml, csv, xml, markdown, docx, xlsx, pptx, auto)
                         Auto-detected from file extension if omitted.
  -o, --output <path>    Write merged result to a file (default: stdout)
  --label-ours <label>   Label for ours side in conflict markers (default: ours)
  --label-theirs <label> Label for theirs side in conflict markers (default: theirs)
```

When no semantic driver matches, `merge-file` falls back to line-based three-way merge with standard conflict markers.

---

## Supported Formats

| Format | Extensions | Merge Granularity | Binary? |
|--------|-----------|-------------------|---------|
| JSON | `.json`, `.jsonl` | Field-level (RFC 6901 paths) | No |
| YAML | `.yaml`, `.yml` | Key-level (recursive) | No |
| TOML | `.toml` | Table and key-aware | No |
| CSV | `.csv`, `.tsv` | Row-level with header detection | No |
| XML | `.xml`, `.xsl`, `.svg` | Element/attribute-aware | No |
| Markdown | `.md`, `.markdown` | Section-aware | No |
| Word | `.docx`, `.docm` | Paragraph-level (preserves formatting) | Yes |
| Excel | `.xlsx`, `.xlsm` | Cell-level (preserves formulas) | Yes |
| PowerPoint | `.pptx`, `.pptm` | Slide-level (preserves formatting) | Yes |
| OTIO | `.otio` | Clip-level (video timeline) | Yes |
| SQL | `.sql` | DDL schema diff | No |

Files without a matching driver fall back to Git's default line-based merge. There is zero overhead for unsupported files.

---

## Troubleshooting

### Driver not being invoked

Check that `.gitattributes` is committed and Git recognizes it:

```bash
git check-attr -a -- config.json
```

Output should show `merge: json`. If it shows nothing, either the `.gitattributes` file is not committed or the pattern doesn't match the file path.

Verify the driver is registered:

```bash
git config --get merge.json.driver
# Should print: suture merge-file --driver json %O %A %B -o %A
```

### Merge still showing conflicts

1. The driver only runs when Git detects a text-level conflict. If Git merges the lines without conflict, the driver is not invoked.
2. Ensure the file matches a `.gitattributes` pattern (use `git check-attr`).
3. Verify the driver command works manually:

```bash
suture merge-file --driver json %O %A %B -o %A
```

Replace the placeholders with actual file paths to test.

### Binary files (DOCX, XLSX, PPTX) showing as conflicts

Ensure `recursive` is set to `binary`:

```bash
git config --get merge.docx.recursive
# Should print: binary

# If not set:
git config merge.docx.recursive binary
```

Without this, Git treats binary files as unmergeable and skips the driver entirely.

### Performance on large files

- Config files (< 100 fields): under 200 microseconds
- CSV with thousands of rows: merges at row-level, scales with row count
- DOCX (50 pages): under 100 milliseconds
- XLSX (thousands of cells): a few hundred milliseconds

Memory usage scales with file size. All three versions are loaded into memory for parsing.

### `suture: command not found` at merge time

The driver is configured globally but `suture` is not on PATH in all environments (e.g., SSH sessions, CI runners). Verify:

```bash
which suture
echo $PATH
```

Fix by adding suture to a standard PATH location:

```bash
sudo ln -s "$(which suture)" /usr/local/bin/suture
```

Or use the full path in the driver config:

```bash
git config merge.json.driver "/usr/local/bin/suture merge-file --driver json %O %A %B -o %A"
```

### Malformed files causing merge failure

If a file is not valid for its format (malformed JSON, corrupted YAML), the driver exits non-zero and Git falls back to standard conflict markers. Fix the file format or resolve manually.

---

## Uninstallation

### Using the installer script

```bash
./scripts/install-merge-driver.sh --uninstall
```

### Using the built-in command

```bash
suture git driver uninstall
git add .gitattributes && git commit -m "Remove suture merge driver"
```

### Manual uninstallation

```bash
# Remove per-format drivers
for driver in json yaml toml xml csv md docx xlsx pptx; do
    git config --global --unset "merge.${driver}.name" 2>/dev/null
    git config --global --unset "merge.${driver}.driver" 2>/dev/null
    git config --global --unset "merge.${driver}.recursive" 2>/dev/null
done

# Remove single-driver config (if installed via suture git driver install)
git config --global --remove-section merge.suture 2>/dev/null

# Remove .gitattributes entries
# Edit .gitattributes and ~/.gitattributes to remove lines containing merge=json, merge=yaml, etc.
```

### Verify removal

```bash
git config --get merge.json.driver
# Should print nothing

git check-attr -a -- config.json
# Should show nothing (or merge: default)
```
