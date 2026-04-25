# IDE Integration

Set up Suture as a merge driver in VS Code and JetBrains IDEs so that merge conflicts in structured files (JSON, YAML, TOML, XML, DOCX, XLSX, PPTX, iCal, OTIO, and more) are resolved automatically at the semantic level.

---

## VS Code Integration

### Install the Extension

The Suture VS Code extension lives in `extensions/vscode/`. Install it from the VS Code Marketplace, or build from source:

```bash
cd extensions/vscode
npm install
npm run compile
npx @vscode/vsce package
code --install-extension suture-5.0.0.vsix
```

### Workspace Configuration

Create or update `.vscode/settings.json` in your project root:

```json
{
  "suture.executablePath": "suture",
  "suture.autoConfigure": true,
  "git.mergeDriver": "suture",
  "git.mergeEditor": true
}
```

| Setting | Default | Description |
|---------|---------|-------------|
| `suture.executablePath` | `"suture"` | Path to the `suture` binary. Use an absolute path if it's not on your PATH. |
| `suture.autoConfigure` | `false` | Automatically registers Suture as the merge driver when a `.suture` directory is detected. |

### Register Suture as the Merge Tool

Use the command palette (`Ctrl+Shift+P` / `Cmd+Shift+P`) to run one of:

- **Suture: Configure Git Merge Driver** — registers the driver in `.git/config`
- **Suture: Enable Semantic Merge for JSON** — configures `.gitattributes` for `.json` files only
- **Suture: Enable Semantic Merge for YAML** — configures `.gitattributes` for `.yaml`/`.yml` files only
- **Suture: Enable Semantic Merge for All Formats** — configures `.gitattributes` for all 20+ supported formats

### Workflow

1. Two branches edit different fields in the same structured file (e.g., one changes `database.host`, the other changes `cache.ttl` in `config.json`).
2. You run **Git: Merge Branch** in VS Code (or use the Source Control view).
3. Git detects a line-level conflict and invokes the Suture merge driver.
4. Suture parses the file semantically, applies both sets of changes, and writes the merged result.
5. VS Code shows the file as resolved — no conflict markers, no manual editing.

For binary formats (DOCX, XLSX, PPTX), the extension also sets `merge.suture.recursive` to `binary` automatically so Git passes those files to the driver instead of marking them unmergeable.

---

## JetBrains Integration

Suture has a native JetBrains plugin (`jetbrains-plugin/`) compatible with IntelliJ IDEA, PyCharm, WebStorm, and other JetBrains IDEs built on the 2024.2+ platform.

### Install the Plugin

Build from source or install from the JetBrains Marketplace:

```bash
cd jetbrains-plugin
./gradlew buildPlugin
# Install: Settings > Plugins > Gear Icon > Install Plugin from Disk...
# Select: build/distributions/suture-1.0.0.zip
```

### Configure as External Merge Tool

1. Open **Settings > Version Control > Git**.
2. Set **Path to Git** if Git is not already detected.
3. Expand **Settings > Version Control > Diff & Merge > External Merge Tools**.
4. Click **+** to add a new tool:

| Field | Value |
|-------|-------|
| Name | `Suture` |
| Program | `/usr/local/bin/suture` (or path to your `suture` binary) |
| Arguments | `merge-file --driver auto %base %local %other --label-ours %local.title --label-theirs %other.title -o %merged` |

If the `suture` binary is on your PATH, you can use `suture` as the program name without a full path.

### Three-Way Merge Arguments

The JetBrains merge tool passes these placeholders:

| Placeholder | Git equivalent | Description |
|-------------|---------------|-------------|
| `%base` | `%O` | Common ancestor |
| `%local` | `%A` | Your changes (ours) |
| `%other` | `%B` | Their changes (theirs) |
| `%merged` | (stdout) | Output path for the resolved file |

For the native Suture CLI:

```
merge-file --driver auto %base %local %other -o %merged
```

For the npm `suture-merge-driver` wrapper:

```
merge-file %base %local %other
```

The npm driver writes the result to the `%local` file in-place (matching Git's `%A` convention), so `%merged` should point to the same path or you should copy the result afterward.

### Set Suture as Default Merge Tool

After adding the tool, go to **Settings > Version Control > Diff & Merge** and set:

- **Merge tool**: `Suture`

Now JetBrains will invoke Suture for all merge conflicts, including those triggered during rebase, cherry-pick, and patch apply operations.

---

## Git Interop

For teams that use Git alongside Suture, configure Suture as a Git merge driver so that `git merge`, `git rebase`, and `git cherry-pick` all benefit from semantic conflict resolution.

### Quick Setup

```bash
cargo install suture-cli
suture git driver install
```

This registers the driver and writes `.gitattributes` for all supported formats in one step.

### Manual Setup

If you prefer manual configuration or are distributing setup across a team:

```bash
# Register the merge driver in git config
git config merge.suture.name "Suture semantic merge"
git config merge.suture.driver "suture-merge-driver %O %A %B %P"
git config merge.suture.recursive binary
```

The `suture-merge-driver` npm package provides the thin wrapper that Git invokes. Install it globally:

```bash
npm install -g suture-merge-driver
```

Or use the full CLI directly:

```bash
git config merge.suture.driver "suture merge-file --driver auto %O %A %B %P"
```

### Example .gitattributes

```gitattributes
# Data serialization formats
*.json merge=suture
*.jsonl merge=suture
*.yaml merge=suture
*.yml merge=suture
*.toml merge=suture
*.csv merge=suture
*.tsv merge=suture
*.xml merge=suture
*.xsl merge=suture
*.svg merge=suture

# Documents
*.md merge=suture
*.docx merge=suture
*.docm merge=suture
*.xlsx merge=suture
*.pptx merge=suture

# Media and timelines
*.otio merge=suture
*.ics merge=suture

# Path-restricted patterns
kubernetes/*.yaml merge=suture
configs/*.toml merge=suture

# Exclude generated files from semantic merge
generated/*.json merge=default
```

Commit `.gitattributes` so the entire team uses the same configuration.

---

## Merge Driver Configuration

### Format-Specific Driver Selection

The merge driver detects file format automatically from the extension. You can override this per-pattern in `.gitattributes`:

```bash
# Use the JSON driver for all files in this directory regardless of extension
git config merge.suture-json.name "Suture JSON merge"
git config merge.suture-json.driver "suture merge-file --driver json %O %A %B %P"

# In .gitattributes
secrets/* merge=suture-json
```

Available drivers: `json`, `yaml`, `toml`, `csv`, `xml`, `markdown`, `docx`, `xlsx`, `pptx`, `otio`, `ical`, `auto` (default — detects from extension).

### Conflict Resolution Strategies

Control how Suture handles semantic conflicts (both sides changed the same element) with the `SUTURE_MERGE_STRATEGY` environment variable:

```bash
# Default: semantic merge with line-based fallback for unresolvable conflicts
SUTURE_MERGE_STRATEGY=semantic git merge feature-branch

# Always keep "ours" on conflict; still merge non-conflicting changes from both sides
SUTURE_MERGE_STRATEGY=ours git merge feature-branch

# Always take "theirs" on conflict
SUTURE_MERGE_STRATEGY=theirs git merge feature-branch
```

For a single merge without affecting future merges:

```bash
git -c merge.suture.name="Suture" \
    -c merge.suture.driver="SUTURE_MERGE_STRATEGY=ours suture-merge-driver %O %A %B %P" \
    merge feature-branch
```

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `SUTURE_MERGE_STRATEGY` | `semantic` | Conflict resolution strategy: `semantic`, `ours`, or `theirs` |
| `SUTURE_PATH` | (auto-detected) | Path to the `suture` binary |
| `SUTURE_LOG_LEVEL` | `warn` | Log verbosity: `error`, `warn`, `info`, `debug`, `trace` |
| `SUTURE_CONFLICT_DIR` | `.suture_conflicts` | Directory for conflict reports on binary formats |
| `SUTURE_DRIVER` | `auto` | Override automatic format detection |
| `SUTURE_IGNORE_UNKNOWN_KEYS` | `false` | If `true`, unknown keys during merge are silently preserved rather than flagged |

### Verification

After setup, confirm everything is wired correctly:

```bash
git config --get merge.suture.driver
# suture-merge-driver %O %A %B %P

git check-attr -a -- config.json
# config.json: merge: suture

suture merge-file --driver json --version
# suture-merge-driver 5.0.0
```
