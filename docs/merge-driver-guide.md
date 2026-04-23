# Suture Merge Driver Guide

A comprehensive guide for using suture-merge as a Git merge driver to automatically resolve conflicts in structured files.

---

## Quick Start

Install the driver and configure Git in under 30 seconds:

```bash
# Install (pick one)
npm install -g suture-merge-driver
pip install suture-merge-driver
cargo install suture-merge

# Configure Git
git config merge.suture.name "Suture semantic merge"
git config merge.suture.driver "suture-merge-driver %O %A %B %P"
echo "*.json merge=suture" >> .gitattributes
echo "*.yaml merge=suture" >> .gitattributes
echo "*.yml merge=suture" >> .gitattributes
```

Commit `.gitattributes` and every future `git merge` on those file types will use semantic merge.

If you have the full Suture CLI installed, the one-command alternative is:

```bash
suture git driver install
```

This registers the driver and writes `.gitattributes` entries for all 20+ supported formats.

---

## Per-Filetype Configuration

You control which file types use semantic merge through `.gitattributes`. Add patterns for the formats you want:

```gitattributes
# Data serialization
*.json merge=suture
*.yaml merge=suture
*.yml merge=suture
*.toml merge=suture
*.csv merge=suture
*.xml merge=suture

# Documents
*.md merge=suture
*.markdown merge=suture
*.docx merge=suture
*.xlsx merge=suture
*.pptx merge=suture

# Media and timelines
*.otio merge=suture
*.ics merge=suture
*.rss merge=suture
*.atom merge=suture
```

### Path-based patterns

You can restrict semantic merge to specific directories:

```gitattributes
# Only Kubernetes manifests
kubernetes/*.yaml merge=suture
kubernetes/*.yml merge=suture

# Only config files
configs/*.json merge=suture
configs/*.toml merge=suture

# Everything in a project except generated files
*.json merge=suture
generated/*.json merge=default
```

### Binary file support

For binary formats (DOCX, XLSX, PPTX), add an additional config line:

```bash
git config merge.suture.recursive binary
```

Without this, Git treats binary files as unmergeable and skips the driver entirely. Setting `recursive` to `binary` tells Git to pass binary files to the merge driver instead of marking them as conflicts outright.

---

## How It Handles Conflicts

The merge driver receives three files from Git:

- `%O` -- the base version (common ancestor)
- `%A` -- ours (your changes; Git reads the result from this file)
- `%B` -- theirs (their changes)
- `%P` -- the original file path (used for format detection)

### Clean merge

When both sides change non-overlapping parts of the file, the driver applies all changes and writes the merged result to `%A`. Git sees exit code 0 and accepts the result. No conflict markers, no manual resolution.

Example: Alice changes `database.host` while Bob changes `cache.ttl` in the same JSON file. The driver merges both changes and produces valid JSON.

### Semantic conflict

When both sides change the same logical element (e.g., the same JSON key, the same YAML key, the same CSV row), the driver reports a conflict. For text-based formats (JSON, YAML, TOML, CSV, XML, Markdown), the driver falls back to line-based merge with standard Git conflict markers on the conflicting section. For binary formats (DOCX, XLSX, PPTX), the driver preserves the "ours" version and generates a `.suture_conflicts/report.md` file with details about what conflicted.

### Partial conflict

When some changes overlap and others don't, the driver applies all non-conflicting changes and only reports conflicts on the overlapping elements. The rest of the file remains clean and valid.

---

## Fallback Behavior

### Unsupported file types

Files not listed in `.gitattributes` are completely unaffected. Git uses its standard line-based merge as if Suture were not installed. There is zero overhead for unsupported files.

### Unknown file extensions

If a file matches a `.gitattributes` pattern but has an extension the driver doesn't recognize, the driver exits with a non-zero code and Git falls back to its default merge behavior. The merge continues normally -- it just won't benefit from semantic resolution.

### Driver errors

If the driver crashes or encounters an unparseable file (malformed JSON, corrupted YAML, etc.), it exits with a non-zero code. Git treats this as a merge failure and falls back to standard conflict markers, just as it would without any merge driver configured.

---

## Merge Strategies

The suture-merge driver supports three strategies, controlled by the `SUTURE_MERGE_STRATEGY` environment variable:

### semantic (default)

Attempts semantic merge first. For non-overlapping changes, produces a clean merge. For overlapping changes, falls back to line-based conflict markers (text formats) or preserves "ours" (binary formats).

### ours

Always resolves conflicts by keeping "ours" (your version). The other side's changes to conflicting elements are discarded. Non-conflicting changes from both sides still apply.

```bash
SUTURE_MERGE_STRATEGY=ours git merge feature-branch
```

### theirs

Always resolves conflicts by taking "theirs" (their version). Your changes to conflicting elements are discarded. Non-conflicting changes from both sides still apply.

```bash
SUTURE_MERGE_STRATEGY=theirs git merge feature-branch
```

---

## Performance

### Overhead

Semantic merge adds minimal overhead compared to Git's line-based merge. Benchmarks on a Linux x86_64 machine (release build):

| Operation                  | Time      |
|---------------------------|-----------|
| Merge 10-field JSON        | ~9 us     |
| Merge 100-field JSON       | ~126 us   |
| Merge 100-field JSON (conflict) | ~98 us  |

For typical config files (under 100 fields), the overhead is under 200 microseconds -- imperceptible in any workflow.

### Large files

The driver handles large files without issues. CSV files with thousands of rows, JSON files with hundreds of nested keys, and TOML files with many tables are all merged at the structural level. Performance scales with the number of elements, not the file size in bytes.

### Binary documents (DOCX, XLSX, PPTX)

Binary Office documents require ZIP extraction and XML parsing, which adds more overhead than text formats. A typical 50-page DOCX merges in under 100 milliseconds. Large spreadsheets (thousands of cells) may take a few hundred milliseconds.

### Memory usage

The driver loads all three file versions into memory for parsing. For typical config files, this is a few kilobytes. For large binary documents, memory usage scales with the uncompressed document size.

---

## Troubleshooting

### "suture-merge-driver: command not found"

The driver binary is not on your PATH. Verify the installation:

```bash
which suture-merge-driver
```

If using npm, the global bin directory may not be in your PATH:

```bash
npm config get prefix
# Add the bin directory to PATH:
export PATH="$(npm config get prefix)/bin:$PATH"
```

If using pip, ensure the package installed correctly:

```bash
pip show suture-merge-driver
```

### Driver not being invoked

Check that `.gitattributes` is committed and Git recognizes it:

```bash
git check-attr -a -- config.json
```

Output should show `merge: suture`. If it shows nothing, the `.gitattributes` file is either not committed or the pattern doesn't match.

Also verify the driver is registered:

```bash
git config --get merge.suture.driver
```

This should print the driver command string. If empty, re-run the configuration step.

### Conflicts still appearing on structured files

The driver only runs when Git detects a text-level conflict. If Git can merge the lines without conflict (even if the result is semantically wrong), it won't invoke the driver. This is standard Git behavior and not a bug.

For binary files (DOCX, XLSX, PPTX), ensure you have set:

```bash
git config merge.suture.recursive binary
```

Without this setting, Git marks binary files as unmergeable before the driver ever runs.

### Malformed files causing merge failure

If a file is not valid for its format (malformed JSON, corrupted YAML), the driver exits with an error and Git falls back to line-based merge. Fix the file format before merging, or resolve the conflict manually.

---

## Integration with CI/CD

### GitHub Actions

Use the official Suture action to automatically resolve conflicts in CI:

```yaml
name: Auto-merge
on:
  pull_request:
    types: [opened, synchronize, reopened]

jobs:
  auto-merge:
    runs-on: ubuntu-latest
    permissions:
      contents: write
      pull-requests: write
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Install suture-merge-driver
        run: npm install -g suture-merge-driver

      - name: Configure merge driver
        run: |
          git config merge.suture.name "Suture semantic merge"
          git config merge.suture.driver "suture-merge-driver %O %A %B %P"
          echo "*.json merge=suture" >> .gitattributes
          echo "*.yaml merge=suture" >> .gitattributes
          echo "*.yml merge=suture" >> .gitattributes
          echo "*.toml merge=suture" >> .gitattributes

      - name: Attempt merge
        run: |
          git config user.name "github-actions[bot]"
          git config user.email "github-actions[bot]@users.noreply.github.com"
          git merge origin/main --no-edit || true

      - name: Push resolution
        run: |
          git diff --quiet && echo "No conflicts to resolve" && exit 0
          git diff --cached --quiet && echo "No staged changes" && exit 0
          git commit -am "Auto-resolve structured merge conflicts"
          git push
```

You can also use the composite action directly:

```yaml
- uses: WyattAu/suture/.github/actions/suture-action@v5.0.0
  with:
    formats: 'json,yaml,toml'
    fail-on-conflict: 'false'
```

### GitLab CI

```yaml
auto-merge:
  image: node:20
  script:
    - npm install -g suture-merge-driver
    - git config merge.suture.name "Suture semantic merge"
    - git config merge.suture.driver "suture-merge-driver %O %A %B %P"
    - echo "*.json merge=suture" >> .gitattributes
    - echo "*.yaml merge=suture" >> .gitattributes
    - git fetch origin $CI_MERGE_REQUEST_TARGET_BRANCH_NAME
    - git config user.name "gitlab-ci"
    - git config user.email "gitlab-ci@example.com"
    - git merge origin/$CI_MERGE_REQUEST_TARGET_BRANCH_NAME --no-edit || true
    - |
      if ! git diff --quiet --cached; then
        git commit -am "Auto-resolve structured merge conflicts"
        git push origin HEAD
      fi
  rules:
    - if: $CI_PIPELINE_SOURCE == "merge_request_event"
```

---

## Enterprise Deployment

### Team-wide configuration with git templates

To ensure every developer on the team uses the merge driver without manual setup, use Git's template directory:

```bash
# On a shared filesystem or in a bootstrap script:
TEMPLATE_DIR=/etc/git-template
mkdir -p "$TEMPLATE_DIR"

# Write the merge driver config
git config -f "$TEMPLATE_DIR/config" merge.suture.name "Suture semantic merge"
git config -f "$TEMPLATE_DIR/config" merge.suture.driver "suture-merge-driver %O %A %B %P"
git config -f "$TEMPLATE_DIR/config" merge.suture.recursive binary

# Write .gitattributes template
cat > "$TEMPLATE_DIR/info/attributes" << 'EOF'
*.json merge=suture
*.yaml merge=suture
*.yml merge=suture
*.toml merge=suture
*.csv merge=suture
*.xml merge=suture
*.md merge=suture
*.docx merge=suture
*.xlsx merge=suture
*.pptx merge=suture
EOF

# Set as the default template directory
git config --system init.templateDir "$TEMPLATE_DIR"
```

Now every `git init` or `git clone` on the machine will inherit the merge driver configuration automatically. Existing repos can be updated by copying `.gitattributes` into the repo root and committing it.

### System-level installation

Install the driver to a system-wide location:

```bash
# npm
npm install -g suture-merge-driver

# pip (system-wide)
pip3 install suture-merge-driver

# cargo (system-wide)
cargo install suture-merge
```

Ensure the binary is on the default PATH for all users. For npm, this may require adding the global bin directory to `/etc/profile.d/suture.sh`:

```bash
echo 'export PATH="$(npm config get prefix)/bin:$PATH"' > /etc/profile.d/suture.sh
```

### Verification

After deployment, verify the configuration on any machine:

```bash
git config --get merge.suture.driver
# Should print: suture-merge-driver %O %A %B %P

git config --get merge.suture.recursive
# Should print: binary

git check-attr -a -- config.json
# Should print: config.json: merge: suture
```

### Rollout checklist

1. Install the driver binary on all developer machines and CI runners.
2. Configure the git template directory with merge driver settings and `.gitattributes`.
3. Verify the binary is on PATH for all users and CI environments.
4. Add `.gitattributes` to existing repositories and commit it.
5. Run `git check-attr` to confirm the driver is registered.
6. Test with a known-conflict scenario (two branches changing different keys in the same JSON file).
7. Monitor for any driver errors in the first week of rollout.
